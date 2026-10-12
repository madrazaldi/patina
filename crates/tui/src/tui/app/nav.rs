use super::*;
use patina_core::session;
use crate::item::resolve_key;
use crate::tui::pager::{PAGE, Pager};
use crate::tui::source::{self, LibraryView, is_paged};

impl App {
    // ---- loading ---------------------------------------------------------

    /// Start (or restart) loading a column's listing.
    pub(super) fn load(&mut self, idx: usize) {
        let Some(col) = self.columns.get(idx) else { return };
        if !col.source.loads() {
            return;
        }
        if col.source.local() {
            let items = self.local_items(&col.source);
            let col = &mut self.columns[idx];
            col.set_items(items, &mut self.fuzzy);
            col.loaded = true;
            col.loading = false;
            return;
        }
        let col = &mut self.columns[idx];
        col.generation += 1;
        col.error = None;
        if col.pageable() {
            return self.load_pages(idx);
        }
        col.pager = None;
        col.loading = true;
        let (id, generation, source) = (col.id, col.generation, col.source.clone());
        let progressive = col.items.is_empty();
        let (core, tx) = (self.core.clone(), self.tx.clone());
        // Replaces (and aborts) the listing this column was already loading.
        col.task.spawn(async move {
            let page_tx = tx.clone();
            let mut page = move |items, append, total| {
                let _ = page_tx.send(Msg::Listing { id, generation, items, append, total, done: false });
            };
            let _ = tx.send(match source::fetch(&core, &source, progressive, &mut page).await {
                Ok(items) => Msg::Listing { id, generation, items, append: false, total: None, done: true },
                Err(e) => Msg::ListingFailed { id, generation, error: err(e) },
            });
        });
    }

    // ---- big listings, a window at a time --------------------------------

    /// (Re)load a listing that is shown through a [`Pager`]: the first page
    /// tells how many entries there are; after that only pages near the
    /// cursor are fetched, as it moves.
    fn load_pages(&mut self, idx: usize) {
        let col = &mut self.columns[idx];
        col.loading = true;
        if let Some(pager) = &mut col.pager {
            // Keep showing what is there until fresh pages replace it.
            pager.invalidate();
            return self.fetch_pages(idx);
        }
        let (id, generation, path) = (col.id, col.generation, col.path.clone().unwrap_or_default());
        let (core, tx) = (self.core.clone(), self.tx.clone());
        col.task.spawn(async move {
            let result = match core.server() {
                Ok(server) => source::fetch_page(&server, &path, 0, PAGE).await.map_err(err),
                Err(e) => Err(err(e)),
            };
            let _ = tx.send(match result {
                Ok((items, total)) => Msg::Page { id, generation, page: 0, items, total },
                Err(error) => Msg::ListingFailed { id, generation, error },
            });
        });
    }

    /// Request the pages the cursor of column `idx` needs and drop the ones it left.
    fn fetch_pages(&mut self, idx: usize) {
        let height = self.list_height.max(10);
        let (core, tx) = (self.core.clone(), self.tx.clone());
        let col = &mut self.columns[idx];
        let (id, generation, cursor) = (col.id, col.generation, col.cursor);
        let Some(path) = col.path.clone() else { return };
        let Some(pager) = &mut col.pager else { return };
        let wanted = pager.wanted(cursor, height);
        pager.trim(&wanted, cursor);
        let size = pager.size();
        for page in pager.missing(&wanted) {
            let (core, tx, path) = (core.clone(), tx.clone(), path.clone());
            pager.fetch(page, async move {
                let result = match core.server() {
                    Ok(server) => source::fetch_page(&server, &path, page * size, size).await.map_err(err),
                    Err(e) => Err(err(e)),
                };
                let _ = tx.send(match result {
                    Ok((items, total)) => Msg::Page { id, generation, page, items, total },
                    Err(error) => Msg::PageFailed { id, generation, page, error },
                });
            });
        }
        col.loading = pager.loading() > 0;
    }

    /// Keep the focused column's window of pages current after the cursor moved.
    pub(super) fn page_in(&mut self) {
        if let Some(idx) = self.columns.len().checked_sub(1)
            && self.columns[idx].pager.is_some()
        {
            self.fetch_pages(idx);
        }
    }

    pub(super) fn on_page(&mut self, id: u64, generation: u64, page: usize, items: Vec<Item>, total: Option<usize>) {
        let Some(idx) = self.column_index(id) else { return };
        let col = &mut self.columns[idx];
        if col.generation != generation || col.want_full {
            return;
        }
        match &mut col.pager {
            Some(pager) => pager.insert(page, items, total.unwrap_or(pager.total)),
            None => {
                // The first page: learn the size of the listing. A server that
                // serves fewer than asked for sets the page size.
                let total = total.unwrap_or(items.len());
                let size = if items.len() < PAGE && items.len() < total { items.len().max(1) } else { PAGE };
                let mut pager = Pager::new(total, size);
                pager.insert(0, items, total);
                col.pager = Some(pager);
                col.items.clear();
                col.view.clear();
            }
        }
        col.loaded = true;
        col.error = None;
        col.move_to(col.cursor);
        self.fetch_pages(idx);
        if idx + 1 == self.columns.len() {
            self.after_selection_change();
        }
    }

    pub(super) fn on_page_failed(&mut self, id: u64, generation: u64, page: usize, error: String) {
        let Some(idx) = self.column_index(id) else { return };
        let col = &mut self.columns[idx];
        if col.generation != generation {
            return;
        }
        if let Some(pager) = &mut col.pager {
            pager.fail(page);
            col.loading = pager.loading() > 0;
        }
        col.error = Some(error.clone());
        let title = col.title.clone();
        self.error(format!("Loading {title}: {error}"));
    }

    /// Filtering, hiding watched titles and local sorting need the whole
    /// listing: leave the window and load everything (the cursor's entry is
    /// selected again as it arrives).
    pub(super) fn go_full(&mut self, idx: usize) {
        let Some(col) = self.columns.get_mut(idx) else { return };
        // Windowed, or its first page is on the way: either way, start over in full.
        let windowed = col.pager.is_some() || (col.loading && !col.want_full && matches!(col.source, Source::Library(_)));
        col.want_full = true;
        if !windowed {
            return;
        }
        if let Some(item) = col.selected() {
            col.select_on_load = Some(Column::identity(item));
        }
        col.pager = None;
        col.items.clear();
        col.view.clear();
        col.cursor = 0;
        col.scroll = 0;
        self.load(idx);
    }

    pub(super) fn column_index(&self, id: u64) -> Option<usize> {
        self.columns.iter().position(|c| c.id == id)
    }

    pub(super) fn open_column(&mut self, title: String, path: String) {
        self.open_source(title, Source::Path(path), Vec::new());
    }

    /// Push a column for `source`, showing `items` until it loads.
    pub(super) fn open_source(&mut self, title: String, source: Source, items: Vec<Item>) {
        if source == Source::Settings {
            self.probe_mpv();
        }
        let mut col = self.new_column(title, source);
        if !items.is_empty() || !col.source.loads() {
            col.items = items;
            col.loaded = true;
            col.rebuild(&mut self.fuzzy);
        }
        self.columns.push(col);
        let idx = self.columns.len() - 1;
        self.load(idx);
    }

    pub(super) fn drill(&mut self) {
        let Some(col) = self.columns.last() else { return };
        let Some(item) = col.selected().cloned() else { return };
        if item.kind() == Kind::Remote {
            self.info(format!("{} isn't on this server", item.title()));
            return;
        }
        if item.link_source() == Some(&Source::Search) {
            return self.open_search();
        }
        if let Some(link) = item.link.clone() {
            self.open_source(item.title().trim().to_string(), link.source, link.items);
            if let Some(col) = self.columns.last_mut() {
                col.from = Some(Column::identity(&item));
            }
            return;
        }
        if item.kind() == Kind::Section
            && let Some(view) = library_view(&item)
        {
            return self.open_source(item.title().to_string(), Source::Library(view), Vec::new());
        }
        let Some(key) = item.children_key() else { return };
        if item.is_playable() {
            return;
        }
        let path = match &col.path {
            Some(parent) => resolve_key(parent, &key),
            None => key,
        };
        let title = match item.kind() {
            Kind::Season | Kind::Album => item.full_title(),
            _ => item.title().to_string(),
        };
        self.open_column(title, path);
    }

    /// `c`: related hubs, cast & crew and tags of the selection, as a column.
    pub(super) fn open_related(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        let (Some(rating_key), Some(kind)) = (item.rating_key.clone(), item.meta.kind.clone()) else { return };
        if !detail_kind(&item) {
            self.info("Related items, cast and tags are for library items");
            return;
        }
        self.search = None;
        self.open_source(format!("{} · Related", item.title()), Source::Related { rating_key, kind }, Vec::new());
    }

    pub(super) fn back(&mut self) {
        if self.columns.len() > 1 {
            self.columns.pop();
            self.inspector_scroll = 0;
        }
    }

    pub(super) fn on_listing(&mut self, id: u64, generation: u64, items: Vec<Item>, append: bool, total: Option<usize>, done: bool) {
        let Some(idx) = self.column_index(id) else { return };
        let col = &mut self.columns[idx];
        if col.generation != generation {
            return;
        }
        if done && col.source == Source::Watchlist {
            self.watchlist = items.iter().filter_map(|i| i.guid.clone()).collect();
        }
        if append {
            col.append(items, &mut self.fuzzy);
        } else {
            col.set_items(items, &mut self.fuzzy);
        }
        col.total = total.or(col.total);
        col.loaded = true;
        if let Some(k) = col.select_on_load.clone()
            && col.select_identity(&k)
        {
            col.select_on_load = None;
        }
        if done {
            col.loading = false;
            col.total = None;
            col.error = None;
            // A static column opened from here (a hub's items) follows the reload.
            let items = &self.columns[idx].items;
            if let Some(next) = self.columns.get(idx + 1).filter(|n| n.source == Source::Static && n.from.is_some()) {
                let entry = items.iter().find(|i| Some(Column::identity(i)) == next.from);
                let fresh = entry.and_then(|i| i.link.as_ref()).map_or(Vec::new(), |l| l.items.clone());
                self.columns[idx + 1].set_items(fresh, &mut self.fuzzy);
            }
        }
        if done {
            self.finish_mark_all(idx);
        }
        if idx + 1 == self.columns.len() {
            self.after_selection_change();
        }
    }

    pub(super) fn apply_refresh(&mut self, refresh: Refresh) {
        match refresh {
            Refresh::Nothing => {}
            Refresh::WatchState => self.core.bump(),
            Refresh::Playlists => {
                let paths: Vec<String> = self.columns.iter().filter_map(|c| c.path.clone()).filter(|p| p.starts_with(PLAYLISTS)).collect();
                self.apply_refresh(Refresh::Paths(paths));
            }
            Refresh::Paths(paths) => {
                for idx in 0..self.columns.len() {
                    if self.columns[idx].path.as_ref().is_some_and(|p| paths.contains(p)) {
                        self.load(idx);
                    }
                }
            }
            Refresh::Source(source) => {
                for idx in 0..self.columns.len() {
                    if self.columns[idx].source == source {
                        self.load(idx);
                    }
                }
            }
            Refresh::Item(key) => {
                self.details.remove(&key);
                self.fetch_detail(key.clone());
                for idx in 1..self.columns.len() {
                    let col = &self.columns[idx];
                    let shows_it = col.items.iter().any(|i| i.rating_key.as_deref() == Some(key.as_str()));
                    if shows_it && col.path.as_deref().is_some_and(|p| !is_paged(p)) {
                        self.load(idx);
                    }
                }
            }
            Refresh::Removed(key) => {
                self.details.remove(&key);
                let paged: Vec<usize> = (0..self.columns.len()).filter(|&i| self.columns[i].pager.as_ref().is_some_and(|p| p.contains(|it| it.rating_key.as_deref() == Some(key.as_str())))).collect();
                for idx in paged {
                    self.load(idx);
                }
                for col in &mut self.columns {
                    let before = col.items.len();
                    col.items.retain(|i| i.rating_key.as_deref() != Some(key.as_str()));
                    if col.items.len() != before {
                        col.rebuild(&mut self.fuzzy);
                    }
                }
            }
            Refresh::Watchlist => {
                self.load_watchlist_guids();
                self.apply_refresh(Refresh::Source(Source::Watchlist));
            }
        }
    }

    pub(super) fn refresh_all(&mut self) {
        self.details.clear();
        let core = self.core.clone();
        tokio::spawn(async move {
            let _ = session::reload_sections(&core).await;
        });
        for idx in 1..self.columns.len() {
            self.load(idx);
        }
        self.info("Refreshing everything…");
    }
}

/// The default library listing of a section entry.
pub(super) fn library_view(section: &Item) -> Option<LibraryView> {
    Some(LibraryView::new(section.key.as_deref()?, section.meta.kind.as_deref().unwrap_or("")))
}
