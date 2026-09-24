//! Watchlist, playlists and watch history.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use patina_plex::{HistoryQuery, Metadata};
use serde::Serialize;

use crate::state::AppState;

/// The plex.tv watchlist, up to 1000 items.
pub async fn watchlist(app: &AppState) -> Result<Vec<Metadata>> {
    // Discover caps the page size at an undocumented value that has shrunk
    // before (even 50 was refused at times), so page and step down when refused.
    let tv = app.plex_tv()?;
    let mut sizes = [50u32, 20, 10].into_iter().peekable();
    let mut size = sizes.next().unwrap_or(10);
    let mut items = Vec::new();
    loop {
        let page = match tv.watchlist(items.len() as u32, size).await {
            Ok(page) => page,
            Err(e) if format!("{e:#}").contains("container-size") && sizes.peek().is_some() => {
                size = sizes.next().unwrap_or(size);
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        let total = page.total_size.map(|t| t as usize);
        let got = page.metadata.len();
        items.extend(page.metadata);
        if got < size as usize || total.is_some_and(|t| items.len() >= t) || items.len() >= 1000 {
            break;
        }
    }
    Ok(items)
}

pub async fn watchlisted(app: &AppState, discover_id: &str) -> Result<bool> {
    let state = app.plex_tv()?.user_state(discover_id).await?;
    Ok(state.is_some_and(|s| s.watchlisted_at.is_some()))
}

#[derive(Serialize)]
pub struct PlaylistView {
    pub playlist: Metadata,
    pub items: Vec<Metadata>,
}

pub async fn playlist(app: &AppState, id: &str) -> Result<Option<PlaylistView>> {
    let server = app.server()?;
    let Some(playlist) = server.playlist(id).await? else { return Ok(None) };
    let items = server.playlist_items(id, 0, 2000).await?.metadata;
    Ok(Some(PlaylistView { playlist, items }))
}

/// Add an item to an existing playlist, or to a new one called `title` of `kind`.
pub async fn add_to_playlist(app: &AppState, rating_key: &str, playlist: Option<&str>, title: Option<&str>, kind: &str) -> Result<()> {
    let server = app.server()?;
    let uri = server.item_uri(&[rating_key]);
    match (playlist, title) {
        (Some(id), _) => server.add_to_playlist(id, &uri).await?,
        (None, Some(title)) => {
            server.create_playlist(title, kind, &uri, false).await?;
        }
        (None, None) => return Err(anyhow!("No playlist given.")),
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub items: Vec<Metadata>,
    pub has_more: bool,
    /// Account names by id (owners only).
    pub accounts: HashMap<i64, String>,
}

pub const HISTORY_PAGE: u32 = 60;

/// Page `page` of the server's watch history, newest first.
pub async fn history(app: &AppState, page: u32) -> Result<HistoryPage> {
    let server = app.server()?;
    let owned = app.session().is_some_and(|s| s.owned);
    let mc = server.history(&HistoryQuery { start: page * HISTORY_PAGE, size: HISTORY_PAGE, ..Default::default() }).await?;
    let accounts = if owned {
        server.accounts().await.unwrap_or_default().into_iter().filter_map(|a| Some((a.id?, a.name?))).collect()
    } else {
        HashMap::new()
    };
    let has_more = mc.has_more() || mc.metadata.len() == HISTORY_PAGE as usize;
    Ok(HistoryPage { items: mc.metadata, has_more, accounts })
}

/// Delete a history entry by its `historyKey`.
pub async fn delete_history(app: &AppState, key: &str) -> Result<()> {
    Ok(app.server()?.delete_history(last_segment(key)).await?)
}

/// The id at the end of a key like `/status/sessions/history/12`.
pub(crate) fn last_segment(key: &str) -> &str {
    key.rsplit('/').next().unwrap_or(key)
}
