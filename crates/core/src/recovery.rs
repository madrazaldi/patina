//! What survives a connection failure or a restart: a local position per item
//! (checkpoints) and the play queue with its position.
//!
//! Both are written to the platform data directory and scoped by account and
//! server, so another profile or server never sees them. Plex stays the source
//! of truth for watch state; a checkpoint only matters when the server did not
//! hear about the last minutes of playback (the connection dropped), which is
//! why one is ignored once the server has a newer view of the item.

use std::path::PathBuf;

use anyhow::Result;
use patina_plex::Metadata;
use serde::{Deserialize, Serialize};

use crate::state::Session;
use crate::{config, offline, storage};

/// The account and server something belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    pub account: String,
    pub server: String,
}

impl Scope {
    pub fn of(session: &Session) -> Self {
        Self { account: offline::account(session), server: session.server_id.clone() }
    }
}

fn checkpoints_path() -> PathBuf {
    config::data_dir().join("playback-checkpoints.json")
}

fn queues_path() -> PathBuf {
    config::data_dir().join("saved-queues.json")
}

const MONTH: i64 = 30 * 24 * 3600;
/// A local checkpoint is only used to start playback again for this long; after that
/// the server's position (or an explicit resume of a saved queue) decides.
const RECOVERY_WINDOW: i64 = 6 * 3600;
const MAX_CHECKPOINTS: usize = 200;
const MAX_QUEUES: usize = 16;
/// Longest queue kept; around the playing item when there are more.
pub const MAX_QUEUE_ITEMS: usize = 300;

// Checkpoints ------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub account: String,
    pub server: String,
    pub rating_key: String,
    pub position: i64,
    pub duration: i64,
    pub updated: i64,
}

impl Checkpoint {
    fn is(&self, scope: &Scope, rating_key: &str) -> bool {
        self.account == scope.account && self.server == scope.server && self.rating_key == rating_key
    }
}

/// Remember where an item is being played.
pub fn save_checkpoint(scope: &Scope, rating_key: &str, position: i64, duration: i64) -> Result<()> {
    let now = offline::now();
    storage::update_json::<Vec<Checkpoint>, _>(&checkpoints_path(), |list| {
        list.retain(|c| !c.is(scope, rating_key) && now - c.updated < MONTH);
        list.push(Checkpoint {
            account: scope.account.clone(),
            server: scope.server.clone(),
            rating_key: rating_key.into(),
            position: position.max(0),
            duration,
            updated: now,
        });
        if list.len() > MAX_CHECKPOINTS {
            let drop = list.len() - MAX_CHECKPOINTS;
            list.drain(..drop);
        }
    })
}

/// Forget an item's checkpoint (it was finished).
pub fn clear_checkpoint(scope: &Scope, rating_key: &str) -> Result<()> {
    storage::update_json::<Vec<Checkpoint>, _>(&checkpoints_path(), |list| list.retain(|c| !c.is(scope, rating_key)))
}

pub fn checkpoint(scope: &Scope, rating_key: &str) -> Option<Checkpoint> {
    storage::read_json::<Vec<Checkpoint>>(&checkpoints_path()).ok()?.into_iter().find(|c| c.is(scope, rating_key))
}

/// The position to resume `item` from, if a local checkpoint is newer than what the server knows.
pub fn resume_position(scope: &Scope, item: &Metadata) -> Option<i64> {
    let cp = checkpoint(scope, item.rating_key.as_deref()?)?;
    usable(&cp, item.last_viewed_at, offline::now())
}

fn usable(cp: &Checkpoint, last_viewed_at: Option<i64>, now: i64) -> Option<i64> {
    let finished = cp.duration > 0 && cp.position as f64 >= cp.duration as f64 * 0.95;
    // The server saw the item after the checkpoint (here or on another device): it knows better.
    let server_newer = last_viewed_at.is_some_and(|seen| seen > cp.updated);
    (cp.position > 0 && !finished && !server_newer && now - cp.updated < RECOVERY_WINDOW).then_some(cp.position)
}

// Saved queue ------------------------------------------------------------------

/// The queue that was playing, to offer again after a restart.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedQueue {
    pub account: String,
    pub server: String,
    pub saved_at: i64,
    /// `video` or `audio`.
    pub kind: String,
    /// Slim copies of the queue items, in play order.
    pub items: Vec<Metadata>,
    pub current: usize,
    pub position_ms: i64,
    pub quality: Option<String>,
}

impl SavedQueue {
    fn is(&self, scope: &Scope) -> bool {
        self.account == scope.account && self.server == scope.server
    }
}

/// What a front end shows in its "Resume" offer.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedQueueView {
    pub current: Metadata,
    pub position_ms: i64,
    pub index: usize,
    pub count: usize,
    pub saved_at: i64,
}

impl SavedQueue {
    pub fn view(&self) -> Option<SavedQueueView> {
        Some(SavedQueueView {
            current: self.items.get(self.current)?.clone(),
            position_ms: self.position_ms,
            index: self.current,
            count: self.items.len(),
            saved_at: self.saved_at,
        })
    }
}

/// An item reduced to what is needed to list and re-queue it.
pub fn slim(m: &Metadata) -> Metadata {
    Metadata {
        rating_key: m.rating_key.clone(),
        key: m.key.clone(),
        kind: m.kind.clone(),
        title: m.title.clone(),
        year: m.year,
        index: m.index,
        parent_index: m.parent_index,
        duration: m.duration,
        thumb: m.thumb.clone(),
        parent_thumb: m.parent_thumb.clone(),
        grandparent_thumb: m.grandparent_thumb.clone(),
        parent_title: m.parent_title.clone(),
        grandparent_title: m.grandparent_title.clone(),
        parent_rating_key: m.parent_rating_key.clone(),
        grandparent_rating_key: m.grandparent_rating_key.clone(),
        library_section_id: m.library_section_id,
        last_viewed_at: m.last_viewed_at,
        ..Default::default()
    }
}

/// At most [`MAX_QUEUE_ITEMS`] items keeping `current` in view; returns the window and the new index of `current`.
pub fn window(len: usize, current: usize) -> (std::ops::Range<usize>, usize) {
    if len <= MAX_QUEUE_ITEMS {
        return (0..len, current);
    }
    let start = current.saturating_sub(MAX_QUEUE_ITEMS / 10).min(len - MAX_QUEUE_ITEMS);
    (start..start + MAX_QUEUE_ITEMS, current - start)
}

pub fn save_queue(scope: &Scope, queue: SavedQueue) -> Result<()> {
    let now = offline::now();
    storage::update_json::<Vec<SavedQueue>, _>(&queues_path(), |list| {
        list.retain(|q| !q.is(scope) && now - q.saved_at < MONTH);
        list.push(queue);
        if list.len() > MAX_QUEUES {
            let drop = list.len() - MAX_QUEUES;
            list.drain(..drop);
        }
    })
}

pub fn load_queue(scope: &Scope) -> Option<SavedQueue> {
    let now = offline::now();
    storage::read_json::<Vec<SavedQueue>>(&queues_path())
        .ok()?
        .into_iter()
        .find(|q| q.is(scope) && now - q.saved_at < MONTH && q.current < q.items.len())
}

pub fn clear_queue(scope: &Scope) -> Result<()> {
    storage::update_json::<Vec<SavedQueue>, _>(&queues_path(), |list| list.retain(|q| !q.is(scope)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cp(position: i64, duration: i64, updated: i64) -> Checkpoint {
        Checkpoint { account: "a".into(), server: "s".into(), rating_key: "1".into(), position, duration, updated }
    }

    #[test]
    fn a_checkpoint_is_used_until_the_server_has_seen_the_item_later() {
        let c = cp(120_000, 600_000, 1000);
        assert_eq!(usable(&c, None, 1100), Some(120_000));
        assert_eq!(usable(&c, Some(900), 1100), Some(120_000), "the server last saw it before the checkpoint");
        assert_eq!(usable(&c, Some(1000), 1100), Some(120_000));
        assert_eq!(usable(&c, Some(1001), 1100), None, "watched since, maybe elsewhere");
    }

    #[test]
    fn finished_or_stale_checkpoints_are_ignored() {
        assert_eq!(usable(&cp(0, 600_000, 1000), None, 1100), None);
        assert_eq!(usable(&cp(590_000, 600_000, 1000), None, 1100), None);
        assert_eq!(usable(&cp(5000, 600_000, 1000), None, 1000 + RECOVERY_WINDOW + 1), None);
    }

    #[test]
    fn long_queues_keep_the_playing_item_in_the_window() {
        assert_eq!(window(10, 4), (0..10, 4));
        let (range, current) = window(1000, 500);
        assert_eq!(range.len(), MAX_QUEUE_ITEMS);
        assert_eq!(range.start + current, 500);
        let (range, current) = window(1000, 999);
        assert_eq!(range.end, 1000);
        assert_eq!(range.start + current, 999);
        assert_eq!(window(1000, 0).0.start, 0);
    }

    #[test]
    fn slim_items_keep_what_is_needed_to_list_and_requeue() {
        let full = Metadata {
            rating_key: Some("5".into()),
            key: Some("/library/metadata/5".into()),
            kind: Some("episode".into()),
            title: Some("Pilot".into()),
            grandparent_title: Some("Show".into()),
            summary: Some("long".into()),
            index: Some(1),
            parent_index: Some(1),
            ..Default::default()
        };
        let s = slim(&full);
        assert_eq!(s.full_title(), full.full_title());
        assert_eq!(s.summary, None);
        assert!(s.is_playable());
    }
}
