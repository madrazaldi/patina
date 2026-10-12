//! Live updates from the server's notification stream (SSE).
//!
//! Library changes and playback on other devices bump the revision so views
//! refetch; background activities (scans, analysis) are mirrored for the
//! sidebar and the Activity page; session changes nudge the Activity page.
//! The listener reconnects on its own and stops when the session changes.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::StreamExt;
use serde_json::Value;

use crate::state::{AppState, Changes, CoreEvent, ServerActivity};

/// Start listening to the current server, replacing any previous listener.
pub fn watch(app: AppState) {
    let generation = app.live_generation.fetch_add(1, Ordering::SeqCst) + 1;
    app.clone().spawn(async move {
        let current = || app.live_generation.load(Ordering::SeqCst) == generation;
        let pending = Arc::new(Mutex::new(Pending::default()));
        let mut backoff = Duration::from_secs(2);
        while current() {
            let Ok(server) = app.server() else { return };
            if let Ok(list) = server.activities().await {
                if !current() { return; }
                app.set_connected(true);
                app.set_activities(list.iter().filter_map(activity_view).collect());
            }
            match server.notifications(&[]).await {
                Ok(stream) => {
                    if !current() { return; }
                    app.set_connected(true);
                    backoff = Duration::from_secs(2);
                    let mut stream = std::pin::pin!(stream);
                    let mut last_sessions = Instant::now() - Duration::from_secs(5);
                    while let Some(n) = stream.next().await {
                        if !current() {
                            return;
                        }
                        let Ok(n) = n else { break };
                        handle(&app, &n.event, &n.data, &pending, &mut last_sessions);
                    }
                }
                Err(e) => tracing::debug!("notifications unavailable: {e}"),
            }
            if !current() {
                return;
            }
            let reachable = server.capabilities().await.is_ok();
            if !current() { return; }
            app.set_connected(reachable);
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(60));
        }
    });
}

/// Stop listening (signing out).
pub fn stop(app: &AppState) {
    app.live_generation.fetch_add(1, Ordering::SeqCst);
    app.set_activities(Vec::new());
}

/// Changes collected while a refetch is scheduled.
#[derive(Default)]
struct Pending {
    scheduled: bool,
    changes: Changes,
}

fn handle(app: &AppState, event: &str, data: &Value, pending: &Arc<Mutex<Pending>>, last_sessions: &mut Instant) {
    match event {
        "timeline" => {
            // Items added, updated or deleted in a library. State 5 is "done", 9 "deleted".
            for e in entries(data, "TimelineEntry") {
                let state = e.get("state").and_then(Value::as_i64);
                let library = e.get("identifier").and_then(Value::as_str) == Some("com.plexapp.plugins.library");
                let changed = library && (matches!(state, Some(5 | 9)) || e.get("metadataState").and_then(Value::as_str) == Some("deleted"));
                if changed {
                    bump_soon(app, pending, text(e, "sectionID").as_deref(), text(e, "itemID").as_deref());
                }
            }
        }
        "activity" => {
            for n in entries(data, "ActivityNotification") {
                let Some(activity) = n.get("Activity").and_then(|a| serde_json::from_value(a.clone()).ok()) else { continue };
                let Some(view) = activity_view(&activity) else { continue };
                let ended = n.get("event").and_then(Value::as_str) == Some("ended");
                let library = view.kind.starts_with("library.") || view.kind.starts_with("media.");
                let section = view.section_id.clone();
                app.update_activities(|list| {
                    list.retain(|a| a.uuid != view.uuid);
                    if !ended {
                        list.push(view);
                    }
                });
                if ended && library {
                    bump_soon(app, pending, section.as_deref(), None);
                    let app = app.clone();
                    app.clone().spawn(async move {
                        let _ = crate::session::reload_sections(&app).await;
                    });
                }
            }
        }
        "playing" | "transcodeSession.start" | "transcodeSession.end" | "transcodeSession.update" => {
            if last_sessions.elapsed() >= Duration::from_secs(1) {
                *last_sessions = Instant::now();
                app.emit(CoreEvent::SessionsChanged);
            }
            // Something stopped playing (here or on another device): watch state may have changed.
            for n in entries(data, "PlaySessionStateNotification") {
                if n.get("state").and_then(Value::as_str) == Some("stopped") {
                    bump_soon(app, pending, text(n, "librarySectionID").as_deref(), text(n, "ratingKey").as_deref());
                }
            }
        }
        _ => {}
    }
}

/// A string or number field as a string.
fn text(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Coalesce bursts of changes (a scan adds many items) into one refetch that
/// names every section and item touched. A change with no section means "anywhere".
fn bump_soon(app: &AppState, pending: &Arc<Mutex<Pending>>, section: Option<&str>, item: Option<&str>) {
    {
        let mut p = pending.lock().unwrap();
        p.changes.add(section, item);
        if std::mem::replace(&mut p.scheduled, true) {
            return;
        }
    }
    let app = app.clone();
    let pending = pending.clone();
    app.clone().spawn(async move {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let changes = {
            let mut p = pending.lock().unwrap();
            p.scheduled = false;
            std::mem::take(&mut p.changes)
        };
        app.bump_with(changes);
    });
}

/// Notification payloads arrive as `{Key: [...]}` or `{Key: {...}}`, sometimes
/// wrapped in a `NotificationContainer`.
fn entries<'a>(data: &'a Value, key: &str) -> Vec<&'a Value> {
    let data = data.get("NotificationContainer").unwrap_or(data);
    match data.get(key) {
        Some(Value::Array(list)) => list.iter().collect(),
        Some(one @ Value::Object(_)) => vec![one],
        _ => Vec::new(),
    }
}

fn activity_view(a: &patina_plex::Activity) -> Option<ServerActivity> {
    let section = a.context.as_ref().and_then(|c| c.get("librarySectionID"));
    Some(ServerActivity {
        uuid: a.uuid.clone()?,
        kind: a.kind.clone().unwrap_or_default(),
        title: a.title.clone().unwrap_or_default(),
        subtitle: a.subtitle.clone().filter(|t| !t.is_empty()),
        progress: a.progress.unwrap_or(0),
        cancellable: a.cancellable,
        section_id: section.map(|id| id.as_str().map(str::to_string).unwrap_or_else(|| id.to_string())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn changes_merge_sections_and_items() {
        let mut c = Changes::default();
        c.add(Some("1"), Some("10"));
        c.add(Some("1"), Some("11"));
        c.add(Some("2"), None);
        assert!(!c.all);
        assert_eq!(c.sections, ["1", "2"]);
        assert_eq!(c.items, ["10", "11"]);
        c.add(None, Some("12"));
        assert!(c.all, "a change without a section could be anywhere");
    }

    #[test]
    fn text_reads_strings_and_numbers() {
        let v = json!({"a": "1", "b": 2, "c": "", "d": null});
        assert_eq!(text(&v, "a").as_deref(), Some("1"));
        assert_eq!(text(&v, "b").as_deref(), Some("2"));
        assert_eq!(text(&v, "c"), None);
        assert_eq!(text(&v, "d"), None);
        assert_eq!(text(&v, "e"), None);
    }
}
