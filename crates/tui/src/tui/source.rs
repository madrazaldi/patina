//! What a column lists, and how it loads.
//!
//! A new screen is a [`Source`] variant plus its arm in [`fetch`]; a root
//! entry for it is one line in `App::root_items` (via [`crate::item::Item::nav`]).

use patina_core::AppState;
use patina_core::ops::lists;
use patina_plex::Params;

use crate::fetch::{continue_watching, recently_added, watchlist_items};
use crate::item::Item;

const PAGE_SIZE: usize = 500;

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// The root navigation column.
    Root,
    /// Items handed over when the column opens (search results, hub contents).
    Static,
    /// A server listing; big ones load in pages.
    Path(String),
    Continue,
    Recent,
    Watchlist,
    Sessions,
    History,
}

impl Source {
    /// The server path listed, used for paging, refreshes and smart playlists.
    pub fn path(&self) -> Option<String> {
        match self {
            Source::Path(p) => Some(p.clone()),
            _ => None,
        }
    }

    /// Whether there is anything to (re)load.
    pub fn loads(&self) -> bool {
        !matches!(self, Source::Root | Source::Static)
    }
}

/// Where a synthetic entry (nav entry, hub, person, tag) leads.
#[derive(Debug, Clone)]
pub struct Link {
    pub source: Source,
    /// Shown until the source loads, or for good when it is [`Source::Static`].
    pub items: Vec<Item>,
}

pub fn is_paged(path: &str) -> bool {
    (path.starts_with("/library/sections/") && path != "/library/sections")
        || path.starts_with("/library/collections/")
        || (path.starts_with("/playlists/") && path.ends_with("/items"))
}

/// Load a source. Paged listings report each page through `page(items,
/// append, total)` when `progressive`, before the full result is returned.
pub async fn fetch(core: &AppState, source: &Source, progressive: bool, page: &mut (dyn FnMut(Vec<Item>, bool, Option<usize>) + Send)) -> anyhow::Result<Vec<Item>> {
    let server = core.server()?;
    match source {
        Source::Root | Source::Static => Ok(Vec::new()),
        Source::Continue => continue_watching(&server).await,
        Source::Recent => recently_added(&server).await,
        Source::Sessions => Ok(server.sessions().await?.into_iter().map(Item::from).collect()),
        Source::History => Ok(lists::history(core, 0).await?.items.into_iter().map(Item::from).collect()),
        Source::Watchlist => watchlist_items(core, &server).await,
        Source::Path(p) if is_paged(p) => paged(&server, p, progressive, page).await,
        Source::Path(p) => Ok(Item::listing(server.fetch(p, &Params::new()).await?)),
    }
}

async fn paged(
    server: &patina_plex::PlexServer,
    path: &str,
    progressive: bool,
    page: &mut (dyn FnMut(Vec<Item>, bool, Option<usize>) + Send),
) -> anyhow::Result<Vec<Item>> {
    let mut all: Vec<Item> = Vec::new();
    let mut start = 0;
    loop {
        let params = Params::new()
            .set("X-Plex-Container-Start", start)
            .set("X-Plex-Container-Size", PAGE_SIZE)
            .set("includeGuids", 1);
        let mc = server.fetch(path, &params).await?;
        let total = mc.total_size.or(mc.size).map(|n| n.max(0) as usize);
        let items = Item::listing(mc);
        let n = items.len();
        start += n;
        let finished = n == 0 || total.map_or(n < PAGE_SIZE, |t| start >= t);
        if progressive && !finished {
            page(items.clone(), start > n, total);
        }
        all.extend(items);
        if finished {
            return Ok(all);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paged_paths() {
        assert!(is_paged("/library/sections/1/all"));
        assert!(is_paged("/library/sections/1/all?genre=3"));
        assert!(!is_paged("/library/sections"));
        assert!(is_paged("/playlists/9/items"));
        assert!(!is_paged("/playlists"));
        assert!(!is_paged("/library/metadata/4/children"));
    }
}
