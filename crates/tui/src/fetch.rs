//! Listings shared by the UI and the CLI.

use futures::StreamExt;
use patina_core::AppState;
use patina_core::ops::lists;
use patina_plex::{Metadata, PlexServer};

use crate::item::Item;

/// Continue Watching; servers without the hub endpoint list it among the home hubs.
pub async fn continue_watching(server: &PlexServer) -> anyhow::Result<Vec<Item>> {
    from_home_hubs(server.continue_watching(50).await, server, |id| id.contains("continue") || id.contains("ondeck")).await
}

/// Recently added across libraries, from the home hubs where the flat list is unavailable.
pub async fn recently_added(server: &PlexServer) -> anyhow::Result<Vec<Item>> {
    from_home_hubs(server.recently_added(100).await, server, |id| id.contains("recentlyadded")).await
}

async fn from_home_hubs(
    direct: patina_plex::Result<Vec<Metadata>>,
    server: &PlexServer,
    wanted: impl Fn(&str) -> bool,
) -> anyhow::Result<Vec<Item>> {
    let direct = match direct {
        Ok(items) if !items.is_empty() => return Ok(items.into_iter().map(Item::from).collect()),
        other => other,
    };
    let hubs = match server.home_hubs(50).await {
        Ok(hubs) => hubs,
        Err(e) => return direct.map(|_| Vec::new()).map_err(|_| e.into()),
    };
    Ok(hubs
        .into_iter()
        .filter(|h| wanted(&h.hub_identifier.as_deref().unwrap_or("").to_ascii_lowercase()))
        .flat_map(|h| h.metadata)
        .map(Item::from)
        .collect())
}

/// The watchlist, with titles that exist on this server replaced by the
/// local (playable) item.
pub async fn watchlist_items(core: &AppState, server: &PlexServer) -> anyhow::Result<Vec<Item>> {
    let remote = lists::watchlist(core).await?;
    Ok(futures::stream::iter(remote)
        .map(|r| async move {
            if let Some(g) = r.guid.as_deref()
                && let Ok(found) = server.find_by_guid(g).await
                && let Some(local) = found.into_iter().next()
            {
                return Item::from(local);
            }
            Item::discover(r)
        })
        .buffered(8)
        .collect()
        .await)
}

/// Server-wide search, flattened across result hubs.
pub async fn search(server: &PlexServer, query: &str, limit: u32) -> anyhow::Result<Vec<Item>> {
    let hub = server.search_hubs(query, limit, None).await?;
    Ok(Item::listing(patina_plex::MediaContainer { hub, ..Default::default() }))
}
