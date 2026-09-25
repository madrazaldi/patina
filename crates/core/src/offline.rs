//! Local resume points and an account/server-scoped outbox for offline watches.
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use patina_plex::{MetadataOptions, TimelineState, TimelineUpdate};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    config,
    downloads::Download,
    state::{AppState, Session},
    storage,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Progress {
    pub id: String,
    pub account: String,
    pub server: String,
    pub rating_key: String,
    pub position: i64,
    pub duration: i64,
    pub watched: bool,
    pub updated: i64,
    pub revision: String,
    pub pending: bool,
}

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}
pub fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
pub fn account(session: &Session) -> String {
    session.user.as_ref().and_then(|u| u.uuid.clone()).unwrap_or_else(|| digest(session.server.token()))
}
fn path() -> PathBuf {
    config::config_dir().join("offline-progress.json")
}

pub fn position(download: &Download) -> i64 {
    storage::read_json::<Vec<Progress>>(&path())
        .unwrap_or_default()
        .iter()
        .find(|p| p.id == download.id)
        .map(|p| if p.watched { 0 } else { p.position })
        .unwrap_or(download.item.view_offset.unwrap_or(0))
}

pub fn record(download: &Download, position: i64, duration: i64, completed: bool, threshold: f64) -> Result<()> {
    if duration <= 0 {
        return Ok(());
    }
    let watched = completed || position as f64 / duration as f64 >= threshold;
    let progress = Progress {
        id: download.id.clone(),
        account: download.account.clone(),
        server: download.server_id.clone(),
        rating_key: download.item.rating_key().to_string(),
        position: if watched { 0 } else { position.max(0) },
        duration,
        watched,
        updated: now(),
        revision: uuid::Uuid::new_v4().to_string(),
        pending: !download.account.is_empty(),
    };
    storage::update_json::<Vec<Progress>, _>(&path(), |list| {
        list.retain(|p| p.id != progress.id);
        list.push(progress);
    })
}

fn belongs(p: &Progress, server: &str, account: &str) -> bool {
    p.pending && p.server == server && p.account == account
}
fn acknowledge(list: &mut [Progress], sent: &Progress) {
    // A playback checkpoint written while the request was in flight stays pending.
    if let Some(p) = list.iter_mut().find(|p| p.id == sent.id && p.revision == sent.revision) {
        p.pending = false;
    }
}

pub async fn sync(app: &AppState) -> Result<()> {
    let Some(session) = app.session() else {
        return Ok(());
    };
    // Wait until local playback ends so partial checkpoints don't race its final watch state.
    if app.now_playing().is_some_and(|n| n.local) {
        return Ok(());
    }
    let Some(_lease) = storage::try_lock(&config::config_dir().join("offline-sync"))? else {
        return Ok(());
    };
    let account = account(&session);
    let list: Vec<Progress> = storage::read_json(&path())?;
    let mut first_error = None;
    for p in list.into_iter().filter(|p| belongs(p, &session.server_id, &account)) {
        if !app.session().is_some_and(|s| s.server == session.server) {
            break;
        }
        let item = match session.server.metadata(&p.rating_key, MetadataOptions::default()).await {
            Ok(Some(item)) => item,
            Ok(None) | Err(patina_plex::Error::Status { status: 404 | 410, .. }) => {
                // Deleted titles keep their local resume point, but must not block the outbox.
                storage::update_json::<Vec<Progress>, _>(&path(), |list| acknowledge(list, &p))?;
                continue;
            }
            Err(e) => {
                first_error.get_or_insert(e);
                continue;
            }
        };
        // A newer online watch wins; replay must never rewind another device's newer progress.
        let server_is_newer = item.last_viewed_at.unwrap_or(0) > p.updated;
        let server_position = item.view_offset.unwrap_or(0);
        let server_watched = item.is_watched();
        if !server_is_newer {
            let result = if p.watched {
                session.server.scrobble(&p.rating_key).await
            } else {
                session
                    .server
                    .timeline(&TimelineUpdate {
                        rating_key: p.rating_key.clone(),
                        key: item.key.unwrap_or_else(|| format!("/library/metadata/{}", p.rating_key)),
                        state: TimelineState::Stopped,
                        time_ms: p.position,
                        duration_ms: p.duration,
                        play_queue_item_id: None,
                        session: None,
                    })
                    .await
            };
            if let Err(e) = result {
                first_error.get_or_insert(e);
                continue;
            }
        }
        storage::update_json::<Vec<Progress>, _>(&path(), |list| {
            acknowledge(list, &p);
            if server_is_newer && let Some(local) = list.iter_mut().find(|local| local.id == p.id && local.revision == p.revision) {
                local.position = server_position;
                local.watched = server_watched;
            }
        })?;
        app.bump();
    }
    match first_error {
        Some(e) => Err(e.into()),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn progress() -> Progress {
        Progress {
            id: "one".into(),
            account: "alice".into(),
            server: "home".into(),
            rating_key: "1".into(),
            position: 10,
            duration: 100,
            watched: false,
            updated: 1,
            revision: "old".into(),
            pending: true,
        }
    }
    #[test]
    fn never_replays_to_a_different_user_or_server() {
        let p = progress();
        assert!(belongs(&p, "home", "alice"));
        assert!(!belongs(&p, "home", "bob"));
        assert!(!belongs(&p, "away", "alice"));
    }
    #[test]
    fn acknowledging_a_request_does_not_erase_newer_progress() {
        let sent = progress();
        let mut list = vec![sent.clone()];
        list[0].revision = "new".into();
        acknowledge(&mut list, &sent);
        assert!(list[0].pending);
        let newest = list[0].clone();
        acknowledge(&mut list, &newest);
        assert!(!list[0].pending);
    }
}
