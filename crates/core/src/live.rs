//! Live updates from the server's notification stream (SSE).
//!
//! Library changes and playback on other devices bump the revision so views
//! refetch; background activities (scans, analysis) are mirrored for the
//! sidebar and the Activity page; session changes nudge the Activity page.
//! The listener reconnects on its own and stops when the session changes.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures::StreamExt;
use serde_json::Value;

use crate::state::{AppState, CoreEvent, ServerActivity};

/// Start listening to the current server, replacing any previous listener.
pub fn watch(app: AppState) {
    let generation = app.live_generation.fetch_add(1, Ordering::SeqCst) + 1;
    app.clone().spawn(async move {
        let current = || app.live_generation.load(Ordering::SeqCst) == generation;
        let pending = Arc::new(AtomicBool::new(false));
        let mut backoff = Duration::from_secs(2);
        while current() {
            let Ok(server) = app.server() else { return };
            if let Ok(list) = server.activities().await {
                app.set_activities(list.iter().filter_map(activity_view).collect());
            }
            match server.notifications(&[]).await {
                Ok(stream) => {
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

fn handle(app: &AppState, event: &str, data: &Value, pending: &Arc<AtomicBool>, last_sessions: &mut Instant) {
    match event {
        "timeline" => {
            // Items added, updated or deleted in a library. State 5 is "done", 9 "deleted".
            let changed = entries(data, "TimelineEntry").iter().any(|e| {
                let state = e.get("state").and_then(Value::as_i64);
                let library = e.get("identifier").and_then(Value::as_str) == Some("com.plexapp.plugins.library");
                library && (matches!(state, Some(5 | 9)) || e.get("metadataState").and_then(Value::as_str) == Some("deleted"))
            });
            if changed {
                bump_soon(app, pending);
            }
        }
        "activity" => {
            for n in entries(data, "ActivityNotification") {
                let Some(activity) = n.get("Activity").and_then(|a| serde_json::from_value(a.clone()).ok()) else { continue };
                let Some(view) = activity_view(&activity) else { continue };
                let ended = n.get("event").and_then(Value::as_str) == Some("ended");
                let library = view.kind.starts_with("library.") || view.kind.starts_with("media.");
                app.update_activities(|list| {
                    list.retain(|a| a.uuid != view.uuid);
                    if !ended {
                        list.push(view);
                    }
                });
                if ended && library {
                    bump_soon(app, pending);
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
            let stopped = entries(data, "PlaySessionStateNotification")
                .iter()
                .any(|n| n.get("state").and_then(Value::as_str) == Some("stopped"));
            if stopped {
                bump_soon(app, pending);
            }
        }
        _ => {}
    }
}

/// Coalesce bursts of changes (a scan adds many items) into one refetch.
fn bump_soon(app: &AppState, pending: &Arc<AtomicBool>) {
    if pending.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let pending = pending.clone();
    app.clone().spawn(async move {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        pending.store(false, Ordering::SeqCst);
        app.bump();
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
