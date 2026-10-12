//! Undo for reversible actions: watch-state changes (single and bulk), removing a
//! playlist entry and removing something from Continue Watching.
//!
//! Each action goes through a function here, which records what it needs to put
//! things back *before* changing anything, does the change, and announces it with a
//! toast carrying the undo entry's id (see [`crate::state::Toast::undo`]). Front ends
//! then offer "Undo" on that toast and run [`undo`] with the id, or with `None` for
//! the most recent entry (Cmd/Ctrl+Z, the TUI's `Z`).
//!
//! The stack belongs to one account and server: entries of another scope are dropped
//! when the session changes ([`Stack::retain_scope`]) and refused if one slips through.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use futures::{StreamExt, TryStreamExt};
use patina_plex::{Metadata, PlexServer};

use crate::servers;
use crate::state::{AppState, Changes, Session};

/// How many undoable actions are remembered.
const DEPTH: usize = 25;
/// Calls made at once when restoring many items.
const CONCURRENCY: usize = 6;

/// The account and server an undo entry belongs to.
pub fn scope_of(session: &Session) -> String {
    format!("{}:{}", crate::offline::account(session), session.server_id)
}

fn current_scope(app: &AppState) -> Option<String> {
    app.session().map(|s| scope_of(&s))
}

/// What an item's watch state was.
#[derive(Debug, Clone, PartialEq)]
pub struct WatchState {
    pub rating_key: String,
    pub watched: bool,
    /// Resume position in milliseconds.
    pub offset: i64,
}

impl WatchState {
    fn of(item: &Metadata) -> Self {
        Self {
            rating_key: item.rating_key().to_string(),
            watched: item.view_count.unwrap_or(0) > 0,
            offset: item.view_offset.unwrap_or(0).max(0),
        }
    }
}

/// How to reverse one change.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Put these items back to their earlier watch state.
    Watch { server: Option<String>, restore: Vec<WatchState> },
    /// Put a removed entry back into a playlist, after the entry it followed.
    PlaylistEntry { playlist: String, rating_key: String, after: Option<i64> },
    /// Bring back a resume position (what removing from Continue Watching and clearing progress take away).
    Progress { server: Option<String>, rating_key: String, offset: i64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: u64,
    pub scope: String,
    /// What the action was, as in the toast ("Marked Heat watched").
    pub label: String,
    pub action: Action,
}

/// The undo history, newest last.
#[derive(Debug, Default)]
pub struct Stack {
    entries: Vec<Entry>,
    next: u64,
}

impl Stack {
    /// Remember an action; returns its id. Entries of other scopes are dropped first.
    pub fn push(&mut self, scope: &str, label: String, action: Action) -> u64 {
        self.retain_scope(scope);
        self.next += 1;
        self.entries.push(Entry { id: self.next, scope: scope.to_string(), label, action });
        if self.entries.len() > DEPTH {
            self.entries.remove(0);
        }
        self.next
    }

    /// Drop everything that does not belong to `scope`.
    pub fn retain_scope(&mut self, scope: &str) {
        self.entries.retain(|e| e.scope == scope);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// The newest entry of `scope`.
    pub fn last(&self, scope: &str) -> Option<&Entry> {
        self.entries.iter().rev().find(|e| e.scope == scope)
    }

    /// Remove and return entry `id`, or the newest one of `scope`.
    pub fn take(&mut self, scope: &str, id: Option<u64>) -> Option<Entry> {
        self.retain_scope(scope);
        let at = match id {
            Some(id) => self.entries.iter().position(|e| e.id == id)?,
            None => self.entries.len().checked_sub(1)?,
        };
        Some(self.entries.remove(at))
    }

    /// Put an entry that could not be undone back where it was (the newest slot).
    pub fn restore(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Forget every entry (the session ended).
pub fn clear(app: &AppState) {
    app.undo.lock().unwrap().clear();
}

/// Drop entries that belong to another account or server.
pub fn session_changed(app: &AppState, session: Option<&Session>) {
    let mut stack = app.undo.lock().unwrap();
    match session {
        Some(s) => stack.retain_scope(&scope_of(s)),
        None => stack.clear(),
    }
}

/// The id and description of what [`undo`] would reverse.
pub fn last(app: &AppState) -> Option<(u64, String)> {
    let scope = current_scope(app)?;
    app.undo.lock().unwrap().last(&scope).map(|e| (e.id, e.label.clone()))
}

/// Remember an action begun in `scope` (taken before its first request). Nothing is
/// recorded if the account or server changed meanwhile: the action's rating keys and
/// "current server" would then be read against the wrong one.
fn record(app: &AppState, scope: Option<&str>, label: String, action: Action) -> Option<u64> {
    let scope = scope?;
    if current_scope(app).as_deref() != Some(scope) {
        return None;
    }
    Some(app.undo.lock().unwrap().push(scope, label, action))
}

/// Toast `text`, offering undo when the action was recorded.
fn announce(app: &AppState, text: String, undo: Option<u64>) {
    match undo {
        Some(id) => app.toast_undo(text, id),
        None => app.toast(text),
    }
}

// ---- watch state ---------------------------------------------------------

/// Every playable thing below the given items, with its current watch state, for the
/// ones whose state `watched` would change.
fn changing(items: impl IntoIterator<Item = Metadata>, watched: bool) -> Vec<WatchState> {
    items
        .into_iter()
        .map(|m| WatchState::of(&m))
        .filter(|s| if watched { !s.watched } else { s.watched || s.offset > 0 })
        .collect()
}

/// The watch state of `keys` and, for shows, seasons, artists and albums, of everything inside.
async fn leaf_states(server: &PlexServer, keys: &[String]) -> Result<(Vec<Metadata>, Vec<Metadata>)> {
    let mut found = Vec::new();
    for chunk in keys.chunks(40) {
        let refs: Vec<&str> = chunk.iter().map(String::as_str).collect();
        found.extend(server.metadata_many(&refs).await?);
    }
    let mut leaves = Vec::new();
    let mut nested = Vec::new();
    for item in &found {
        if matches!(item.kind(), "show" | "season" | "artist" | "album") {
            nested.push(item.rating_key().to_string());
        } else {
            leaves.push(item.clone());
        }
    }
    let inside = futures::stream::iter(nested)
        .map(|rk| async move { server.all_leaves(&rk).await.map(|mc| mc.metadata) })
        .buffered(CONCURRENCY)
        .try_collect::<Vec<_>>()
        .await?;
    leaves.extend(inside.into_iter().flatten());
    Ok((found, leaves))
}

fn watch_label(found: &[Metadata], keys: usize, watched: bool) -> String {
    let state = if watched { "watched" } else { "unwatched" };
    match found {
        [one] if keys == 1 => format!("Marked {} {state}", one.title()),
        _ if keys == 1 => format!("Marked as {state}"),
        _ => format!("Marked {keys} items {state}"),
    }
}

/// Mark items (or whole shows, seasons, albums) watched or unwatched on `server`
/// (`None` for the current one), remembering how to put them back.
pub async fn set_watched(app: &AppState, server_id: Option<&str>, keys: &[String], watched: bool) -> Result<()> {
    let scope = current_scope(app);
    let foreign = server_id.filter(|id| !servers::is_current(app, Some(id))).map(str::to_string);
    let server = servers::server_for(app, server_id).await?;
    // The earlier state is best effort: failing to read it must not stop the change.
    let before = leaf_states(&server, keys).await.ok();
    for key in keys {
        if watched { server.scrobble(key).await? } else { server.unscrobble(key).await? }
    }
    let undo = before.and_then(|(found, leaves)| {
        let restore = changing(leaves, watched);
        (!restore.is_empty()).then(|| (watch_label(&found, keys.len(), watched), Action::Watch { server: foreign, restore }))
    });
    let text = undo.as_ref().map(|(l, _)| l.clone()).unwrap_or_else(|| {
        if keys.len() == 1 { format!("Marked as {}", if watched { "watched" } else { "unwatched" }) } else { format!("Marked {} items {}", keys.len(), if watched { "watched" } else { "unwatched" }) }
    });
    let id = undo.and_then(|(label, action)| record(app, scope.as_deref(), label, action));
    announce(app, text, id);
    app.bump_with(Changes::everything());
    Ok(())
}

/// Forget the resume position without changing the watched state.
pub async fn clear_progress(app: &AppState, rating_key: &str) -> Result<()> {
    let scope = current_scope(app);
    let server = app.server()?;
    let before = server.metadata(rating_key, Default::default()).await.ok().flatten();
    server.set_progress(rating_key, 0, "stopped").await?;
    let kept = before.as_ref().map(|m| m.view_offset.unwrap_or(0)).filter(|o| *o > 0);
    let id = kept.and_then(|offset| {
        let label = format!("Cleared progress of {}", before.as_ref().map(|m| m.title()).unwrap_or("the item"));
        record(app, scope.as_deref(), label, Action::Progress { server: None, rating_key: rating_key.to_string(), offset })
    });
    announce(app, "Progress cleared".into(), id);
    app.bump_with(Changes::everything());
    Ok(())
}

/// Hide an item from Continue Watching. Undo is offered when it had a resume position
/// to bring back.
pub async fn remove_from_continue_watching(app: &AppState, rating_key: &str) -> Result<()> {
    let scope = current_scope(app);
    let server = app.server()?;
    let before = server.metadata(rating_key, Default::default()).await.ok().flatten();
    server.remove_from_continue_watching(rating_key).await?;
    let offset = before.as_ref().map(|m| m.view_offset.unwrap_or(0)).unwrap_or(0);
    let id = (offset > 0).then(|| {
        let label = format!("Removed {} from Continue Watching", before.as_ref().map(|m| m.title()).unwrap_or("the item"));
        record(app, scope.as_deref(), label, Action::Progress { server: None, rating_key: rating_key.to_string(), offset })
    });
    announce(app, "Removed from Continue Watching".into(), id.flatten());
    app.bump_with(Changes::everything());
    Ok(())
}

// ---- playlists -----------------------------------------------------------

/// A playlist entry as far as position matters.
#[derive(Debug, Clone, PartialEq)]
pub struct Slot {
    pub entry_id: i64,
    pub rating_key: String,
    pub title: String,
}

async fn playlist_slots(server: &PlexServer, id: &str) -> Result<Vec<Slot>> {
    const PAGE: u32 = 500;
    let mut out = Vec::new();
    loop {
        let mc = server.playlist_items(id, out.len() as u32, PAGE).await?;
        let got = mc.metadata.len();
        let total = mc.total_size.map(|t| t.max(0) as usize);
        out.extend(mc.metadata.iter().filter_map(|m| {
            Some(Slot { entry_id: m.playlist_item_id?, rating_key: m.rating_key().to_string(), title: m.title().to_string() })
        }));
        if got < PAGE as usize || total.is_some_and(|t| out.len() >= t) || out.len() >= 50_000 {
            break;
        }
    }
    Ok(out)
}

/// The slot of `entry_id` and the entry id it follows.
pub fn locate(slots: &[Slot], entry_id: i64) -> Option<(&Slot, Option<i64>)> {
    let at = slots.iter().position(|s| s.entry_id == entry_id)?;
    Some((&slots[at], at.checked_sub(1).map(|p| slots[p].entry_id)))
}

/// Remove one entry from a playlist, remembering where it was.
pub async fn remove_from_playlist(app: &AppState, playlist: &str, entry_id: i64) -> Result<()> {
    let scope = current_scope(app);
    let server = app.server()?;
    let slots = playlist_slots(&server, playlist).await.ok();
    let found = slots.as_deref().and_then(|s| locate(s, entry_id)).map(|(slot, after)| (slot.clone(), after));
    server.remove_from_playlist(playlist, entry_id).await?;
    let (text, id) = match found {
        Some((slot, after)) => {
            let label = format!("Removed {} from the playlist", slot.title);
            let id = record(app, scope.as_deref(), label.clone(), Action::PlaylistEntry { playlist: playlist.to_string(), rating_key: slot.rating_key, after });
            (label, id)
        }
        None => ("Removed from the playlist".to_string(), None),
    };
    announce(app, text, id);
    app.bump_with(Changes::everything());
    Ok(())
}

/// Where to put a restored entry, given the playlist as it is now (with the restored
/// entry last): after `after` if that entry still exists, at the top when it was first,
/// else it stays at the end.
pub fn place(slots: &[Slot], after: Option<i64>) -> Option<Option<i64>> {
    match after {
        None => Some(None),
        Some(a) if slots.iter().rev().skip(1).any(|s| s.entry_id == a) => Some(Some(a)),
        Some(_) => None,
    }
}

async fn restore_playlist_entry(server: &PlexServer, playlist: &str, rating_key: &str, after: Option<i64>) -> Result<()> {
    server.add_to_playlist(playlist, &server.item_uri(&[rating_key])).await?;
    let slots = playlist_slots(server, playlist).await?;
    let added = slots.last().ok_or_else(|| anyhow!("The playlist is empty."))?;
    if added.rating_key == rating_key
        && let Some(target) = place(&slots, after)
    {
        server.move_playlist_item(playlist, added.entry_id, target).await?;
    }
    Ok(())
}

// ---- undoing -------------------------------------------------------------

/// Restore calls for watch states: scrobble or unscrobble, then the resume position.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    Scrobble(String),
    Unscrobble(String),
    Progress(String, i64),
}

pub fn calls_for(restore: &[WatchState]) -> Vec<Call> {
    let mut out = Vec::new();
    for s in restore {
        out.push(if s.watched { Call::Scrobble(s.rating_key.clone()) } else { Call::Unscrobble(s.rating_key.clone()) });
        if !s.watched && s.offset > 0 {
            out.push(Call::Progress(s.rating_key.clone(), s.offset));
        }
    }
    out
}

async fn run_calls(server: &PlexServer, calls: Vec<Call>) -> Result<()> {
    // One item's calls stay in order; different items go in parallel.
    let mut per_item: Vec<(String, Vec<Call>)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for c in calls {
        let key = match &c {
            Call::Scrobble(k) | Call::Unscrobble(k) | Call::Progress(k, _) => k.clone(),
        };
        let at = *index.entry(key.clone()).or_insert_with(|| {
            per_item.push((key, Vec::new()));
            per_item.len() - 1
        });
        per_item[at].1.push(c);
    }
    futures::stream::iter(per_item)
        .map(|(_, calls)| async move {
            for c in calls {
                match c {
                    Call::Scrobble(k) => server.scrobble(&k).await?,
                    Call::Unscrobble(k) => server.unscrobble(&k).await?,
                    Call::Progress(k, ms) => server.set_progress(&k, ms, "stopped").await?,
                }
            }
            Ok::<_, patina_plex::Error>(())
        })
        .buffer_unordered(CONCURRENCY)
        .try_collect::<Vec<_>>()
        .await?;
    Ok(())
}

/// Reverse an action. `id` names an entry (an Undo button), `None` the most recent one.
/// Returns what was undone; the front ends are told with a toast and a revision.
pub async fn undo(app: &AppState, id: Option<u64>) -> Result<String> {
    let scope = current_scope(app).ok_or_else(|| anyhow!("Not connected to a server."))?;
    let entry = app.undo.lock().unwrap().take(&scope, id).ok_or_else(|| anyhow!("Nothing to undo."))?;
    let result = match &entry.action {
        Action::Watch { server, restore } => match servers::server_for(app, server.as_deref()).await {
            Ok(s) => run_calls(&s, calls_for(restore)).await,
            Err(e) => Err(e),
        },
        Action::PlaylistEntry { playlist, rating_key, after } => match app.server() {
            Ok(s) => restore_playlist_entry(&s, playlist, rating_key, *after).await,
            Err(e) => Err(e),
        },
        Action::Progress { server, rating_key, offset } => match servers::server_for(app, server.as_deref()).await {
            Ok(s) => s.set_progress(rating_key, *offset, "stopped").await.map_err(Into::into),
            Err(e) => Err(e),
        },
    };
    match result {
        Ok(()) => {
            app.toast(format!("Undid: {}", entry.label));
            app.bump_with(Changes::everything());
            Ok(entry.label)
        }
        Err(e) => {
            // Keep it so a retry is possible once the server is reachable.
            let scope_still = current_scope(app).as_deref() == Some(entry.scope.as_str());
            if scope_still {
                app.undo.lock().unwrap().restore(entry);
            }
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(rk: &str, views: i64, offset: i64) -> Metadata {
        serde_json::from_value(json!({ "ratingKey": rk, "title": rk, "viewCount": views, "viewOffset": offset })).unwrap()
    }

    fn push(stack: &mut Stack, scope: &str, label: &str) -> u64 {
        stack.push(scope, label.into(), Action::Progress { server: None, rating_key: "1".into(), offset: 5 })
    }

    #[test]
    fn undo_takes_the_newest_entry_first() {
        let mut s = Stack::default();
        let a = push(&mut s, "x", "a");
        let b = push(&mut s, "x", "b");
        assert_eq!(s.last("x").map(|e| e.id), Some(b));
        assert_eq!(s.take("x", None).map(|e| e.id), Some(b));
        assert_eq!(s.take("x", Some(a)).map(|e| e.label), Some("a".to_string()));
        assert!(s.take("x", None).is_none());
    }

    #[test]
    fn a_specific_entry_can_be_taken_out_of_order() {
        let mut s = Stack::default();
        let a = push(&mut s, "x", "a");
        let b = push(&mut s, "x", "b");
        assert_eq!(s.take("x", Some(a)).map(|e| e.id), Some(a));
        assert_eq!(s.take("x", Some(a)), None, "already undone");
        assert_eq!(s.len(), 1);
        assert_eq!(s.last("x").map(|e| e.id), Some(b));
    }

    #[test]
    fn entries_never_cross_account_or_server_scopes() {
        let mut s = Stack::default();
        let a = push(&mut s, "alice:server1", "a");
        assert!(s.last("alice:server2").is_none());
        assert!(s.take("bob:server1", Some(a)).is_none());
        // Taking from another scope also forgot the old one.
        assert!(s.is_empty());
        push(&mut s, "alice:server1", "a");
        push(&mut s, "alice:server2", "b");
        assert_eq!(s.len(), 1, "pushing in a new scope drops the old scope");
        s.retain_scope("alice:server1");
        assert!(s.is_empty());
    }

    #[test]
    fn history_is_bounded() {
        let mut s = Stack::default();
        for i in 0..DEPTH + 10 {
            push(&mut s, "x", &i.to_string());
        }
        assert_eq!(s.len(), DEPTH);
        assert_eq!(s.last("x").map(|e| e.label.clone()), Some((DEPTH + 9).to_string()));
        assert_eq!(s.entries[0].label, "10");
    }

    #[test]
    fn a_failed_undo_goes_back_on_the_stack() {
        let mut s = Stack::default();
        let a = push(&mut s, "x", "a");
        let e = s.take("x", None).unwrap();
        s.restore(e);
        assert_eq!(s.last("x").map(|e| e.id), Some(a));
    }

    #[test]
    fn only_items_whose_state_changes_are_remembered() {
        let items = || vec![item("unwatched", 0, 0), item("partial", 0, 30_000), item("watched", 2, 0)];
        let marked_watched = changing(items(), true);
        assert_eq!(marked_watched.iter().map(|s| s.rating_key.as_str()).collect::<Vec<_>>(), ["unwatched", "partial"]);
        // The resume position of the partly watched one is kept so undo can put it back.
        assert_eq!(marked_watched[1].offset, 30_000);
        let marked_unwatched = changing(items(), false);
        assert_eq!(marked_unwatched.iter().map(|s| s.rating_key.as_str()).collect::<Vec<_>>(), ["partial", "watched"]);
        assert!(changing(vec![item("a", 0, 0)], false).is_empty(), "nothing changes, nothing to undo");
    }

    #[test]
    fn restoring_replays_state_then_position() {
        let restore = vec![
            WatchState { rating_key: "1".into(), watched: false, offset: 30_000 },
            WatchState { rating_key: "2".into(), watched: true, offset: 0 },
            WatchState { rating_key: "3".into(), watched: false, offset: 0 },
        ];
        assert_eq!(
            calls_for(&restore),
            [
                Call::Unscrobble("1".into()),
                Call::Progress("1".into(), 30_000),
                Call::Scrobble("2".into()),
                Call::Unscrobble("3".into()),
            ]
        );
    }

    fn slots(ids: &[i64]) -> Vec<Slot> {
        ids.iter().map(|i| Slot { entry_id: *i, rating_key: format!("rk{i}"), title: format!("t{i}") }).collect()
    }

    #[test]
    fn locating_an_entry_finds_its_predecessor() {
        let s = slots(&[10, 20, 30]);
        assert_eq!(locate(&s, 20).map(|(slot, after)| (slot.entry_id, after)), Some((20, Some(10))));
        assert_eq!(locate(&s, 10).map(|(_, after)| after), Some(None), "first entry has no predecessor");
        assert!(locate(&s, 99).is_none());
    }

    #[test]
    fn a_restored_entry_goes_back_after_its_predecessor() {
        // The restored entry (40) is last; it used to follow 10.
        let now = slots(&[10, 30, 40]);
        assert_eq!(place(&now, Some(10)), Some(Some(10)));
        assert_eq!(place(&now, None), Some(None), "it was first");
        assert_eq!(place(&now, Some(99)), None, "the predecessor is gone, so it stays at the end");
        // The entry itself is not its own predecessor.
        assert_eq!(place(&slots(&[40]), Some(40)), None);
    }

    #[test]
    fn labels_name_a_single_item_and_count_many() {
        let one = vec![item("Heat", 0, 0)];
        assert_eq!(watch_label(&one, 1, true), "Marked Heat watched");
        assert_eq!(watch_label(&[], 1, false), "Marked as unwatched");
        assert_eq!(watch_label(&one, 7, true), "Marked 7 items watched");
    }
}
