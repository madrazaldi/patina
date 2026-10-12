//! The other servers of the signed-in account: connecting to them on demand and
//! keeping those connections, so items found on a second server keep being
//! browsed, played and reported against that server (never the current one).

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use patina_plex::{Metadata, PlexServer, connect_resource};

use crate::state::AppState;

/// How long a connection to another server is reused before it is raced again.
const REUSE: Duration = Duration::from_secs(15 * 60);

type Key = (String, String);
static CONNECTIONS: LazyLock<Mutex<HashMap<Key, (Instant, PlexServer)>>> = LazyLock::new(Default::default);

/// Whether `id` names the session's own server (or nothing at all).
pub fn is_current(app: &AppState, id: Option<&str>) -> bool {
    match (id, app.session()) {
        (None, _) | (Some(""), _) => true,
        (Some(id), Some(s)) => id == s.server_id || s.server.machine_identifier() == Some(id),
        (Some(_), None) => false,
    }
}

/// The server with machine identifier `id`: the session's own, or another server of
/// the account, connected to on first use. `None` means the current server.
pub async fn server_for(app: &AppState, id: Option<&str>) -> Result<PlexServer> {
    if is_current(app, id) {
        return app.server();
    }
    let id = id.unwrap_or_default();
    let session = app.session().ok_or_else(|| anyhow!("Not connected to a server."))?;
    let account = crate::offline::account(&session);
    let key = (account, id.to_string());
    if let Some((at, server)) = CONNECTIONS.lock().unwrap().get(&key).cloned()
        && at.elapsed() < REUSE
    {
        return Ok(server);
    }
    let tv = session.tv.clone().ok_or_else(|| anyhow!("Other servers need a plex.tv sign-in."))?;
    let resource = session.servers.iter().find(|r| r.id() == id).cloned().ok_or_else(|| anyhow!("That server is no longer on this account."))?;
    let server = connect_resource(&tv, &resource).await.with_context(|| format!("connecting to {}", resource.name()))?;
    CONNECTIONS.lock().unwrap().insert(key, (Instant::now(), server.clone()));
    Ok(server)
}

/// The server an item lives on: its own tag, else the current one.
pub async fn server_of(app: &AppState, item: &Metadata) -> Result<PlexServer> {
    server_for(app, item.server_id()).await
}

/// Forget a connection that stopped working so the next use connects again.
pub fn forget(app: &AppState, id: &str) {
    if let Some(session) = app.session() {
        CONNECTIONS.lock().unwrap().remove(&(crate::offline::account(&session), id.to_string()));
    }
}

/// Tag items with the server they came from, leaving the current server's items alone.
pub fn tag(app: &AppState, items: &mut [Metadata], server: Option<&str>) {
    let server = server.filter(|id| !is_current(app, Some(id)));
    for item in items {
        tag_one(item, server);
    }
}

/// Tag an item and the items of its related hubs (shown on its page).
fn tag_one(item: &mut Metadata, server: Option<&str>) {
    item.set_server_id(server);
    if let Some(related) = item.related.as_mut() {
        for hub in &mut related.hub {
            for m in &mut hub.metadata {
                m.set_server_id(server);
            }
        }
    }
}

/// A server's display name: the account's resource name, or the session's own.
pub fn name_of(app: &AppState, id: Option<&str>) -> String {
    let Some(session) = app.session() else { return String::new() };
    if is_current(app, id) {
        return session.server_name;
    }
    session.servers.iter().find(|r| Some(r.id()) == id).map(|r| r.name().to_string()).unwrap_or_default()
}

/// Artwork of an item on another server, resized by that server. The webview's own
/// cache keeps it, so it is not written to the on-disk artwork cache.
pub async fn image(app: &AppState, id: &str, source: &str, w: u32, h: u32) -> Option<(Vec<u8>, String)> {
    if source.is_empty() {
        return None;
    }
    let server = server_for(app, Some(id)).await.ok()?;
    let (bytes, ctype) = server.image(source, w, h).await.ok()?;
    if bytes.is_empty() {
        return None;
    }
    let ctype = ctype.filter(|c| c.starts_with("image/")).unwrap_or_else(|| "image/jpeg".to_string());
    Some((bytes, ctype))
}
