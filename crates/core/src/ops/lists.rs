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
    /// How many entries the server says the playlist has; more than
    /// `items.len()` only when the safety limit cut loading short.
    pub total: usize,
}

/// Entries requested per page of a playlist.
pub const PLAYLIST_PAGE: u32 = 1000;
/// A runaway guard: no front end loads more entries than this.
pub const PLAYLIST_LIMIT: usize = 100_000;

pub async fn playlist(app: &AppState, id: &str) -> Result<Option<PlaylistView>> {
    let server = app.server()?;
    let Some(playlist) = server.playlist(id).await? else { return Ok(None) };
    let (items, total) = playlist_entries(&server, id).await?;
    Ok(Some(PlaylistView { playlist, items, total }))
}

/// Every entry of a playlist (paging through the server's limit) and the
/// total the server reports.
pub async fn playlist_entries(server: &patina_plex::PlexServer, id: &str) -> Result<(Vec<Metadata>, usize)> {
    collect_pages(PLAYLIST_PAGE, PLAYLIST_LIMIT, |start, size| async move {
        let mc = server.playlist_items(id, start, size).await?;
        let total = mc.total_size.or(mc.size).map(|n| n.max(0) as usize);
        Ok((mc.metadata, total))
    })
    .await
}

/// Fetch consecutive pages until the reported total, a short page, or
/// `limit` entries. `fetch(start, size)` returns a page and the total the
/// server reports (when it does). The returned total is what the server
/// said, or what was loaded when it said nothing.
pub(crate) async fn collect_pages<T, F, Fut>(size: u32, limit: usize, mut fetch: F) -> Result<(Vec<T>, usize)>
where
    F: FnMut(u32, u32) -> Fut,
    Fut: std::future::Future<Output = Result<(Vec<T>, Option<usize>)>>,
{
    let mut items: Vec<T> = Vec::new();
    let mut total: Option<usize> = None;
    loop {
        let (page, reported) = fetch(items.len() as u32, size).await?;
        total = reported.or(total);
        let got = page.len();
        items.extend(page);
        // Without a total a short page means the end; with one keep going
        // until it is reached, since servers may cap pages below `size`.
        let done = got == 0 || total.map_or(got < size as usize, |t| items.len() >= t) || items.len() >= limit;
        if done {
            items.truncate(limit);
            let total = total.unwrap_or(items.len());
            return Ok((items, total));
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    type Page = Result<(Vec<usize>, Option<usize>)>;

    /// A fake paged source of `n` numbers that serves at most `cap` per request.
    fn source(n: usize, cap: usize, report_total: bool) -> impl FnMut(u32, u32) -> std::future::Ready<Page> {
        move |start, size| {
            let (start, take) = (start as usize, (size as usize).min(cap));
            let page: Vec<usize> = (start..n.min(start + take)).collect();
            std::future::ready(Ok((page, report_total.then_some(n))))
        }
    }

    #[tokio::test]
    async fn collects_every_page() {
        let (items, total) = collect_pages(1000, 100_000, source(2500, 1000, true)).await.unwrap();
        assert_eq!((items.len(), total), (2500, 2500));
        assert!(items.iter().enumerate().all(|(i, v)| i == *v));
    }

    #[tokio::test]
    async fn exact_multiples_and_empty() {
        let (items, total) = collect_pages(1000, 100_000, source(2000, 1000, true)).await.unwrap();
        assert_eq!((items.len(), total), (2000, 2000));
        let (items, total) = collect_pages(1000, 100_000, source(2000, 1000, false)).await.unwrap();
        assert_eq!((items.len(), total), (2000, 2000));
        let (items, total) = collect_pages(1000, 100_000, source(0, 1000, true)).await.unwrap();
        assert_eq!((items.len(), total), (0, 0));
    }

    #[tokio::test]
    async fn server_page_cap_below_requested_size() {
        // The server hands out 300 at a time even though 1000 were requested.
        let (items, total) = collect_pages(1000, 100_000, source(1000, 300, true)).await.unwrap();
        assert_eq!((items.len(), total), (1000, 1000));
    }

    #[tokio::test]
    async fn limit_truncates_but_keeps_the_true_total() {
        let (items, total) = collect_pages(1000, 2500, source(10_000, 1000, true)).await.unwrap();
        assert_eq!(items.len(), 2500);
        assert_eq!(total, 10_000, "a cut-off load must not look complete");
    }

    #[tokio::test]
    async fn errors_bubble_up() {
        let r = collect_pages::<usize, _, _>(10, 100, |start, _| async move {
            if start == 0 { Ok(((0..10).collect(), Some(30))) } else { Err(anyhow!("boom")) }
        })
        .await;
        assert!(r.is_err());
    }
}
