use super::*;
use patina_core::session;
use crate::item::resolve_key;
use crate::tui::source::{self, is_paged};

impl App {
    // ---- loading ---------------------------------------------------------

    /// Start (or restart) loading a column's listing.
    pub(super) fn load(&mut self, idx: usize) {
        let Some(col) = self.columns.get_mut(idx) else { return };
        if !col.source.loads() {
            return;
        }
        col.generation += 1;
        col.loading = true;
        col.error = None;
        let (id, generation, source) = (col.id, col.generation, col.source.clone());
        let progressive = col.items.is_empty();
        let (core, tx) = (self.core.clone(), self.tx.clone());
        tokio::spawn(async move {
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

    pub(super) fn column_index(&self, id: u64) -> Option<usize> {
        self.columns.iter().position(|c| c.id == id)
    }

    pub(super) fn open_column(&mut self, title: String, path: String) {
        self.open_source(title, Source::Path(path), Vec::new());
    }

    /// Push a column for `source`, showing `items` until it loads.
    pub(super) fn open_source(&mut self, title: String, source: Source, items: Vec<Item>) {
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
        if let Some(link) = item.link {
            return self.open_source(item.meta.title.clone().unwrap_or_default(), link.source, link.items);
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
        if done {
            col.loading = false;
            col.total = None;
            col.error = None;
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

    pub(super) fn open_browse(&mut self) {
        let sid = self
            .selected()
            .filter(|i| i.kind() == Kind::Section)
            .and_then(|i| i.key.clone())
            .or_else(|| self.columns.last()?.section_id().map(str::to_string));
        let Some(section_id) = sid else {
            self.info("Browse-by works in a library");
            return;
        };
        let section_title = self.sections.iter().find(|s| s.key.as_deref() == Some(&section_id)).map(|s| s.title().to_string()).unwrap_or_default();
        self.modal = Some(Modal::Browse { section_id, section_title, cursor: 0 });
    }

    pub(super) fn browse_to(&mut self, section_id: &str, section_title: &str, view: usize) {
        let (label, sub) = BROWSE_VIEWS[view];
        let path = format!("/library/sections/{section_id}/{sub}");
        let title = if view == 0 { section_title.to_string() } else { format!("{section_title} · {label}") };
        // Replace the library's column (or open one from the root).
        let keep = if self.columns.last().and_then(|c| c.section_id()) == Some(section_id) {
            self.columns.iter().position(|c| c.section_id() == Some(section_id)).unwrap_or(1)
        } else {
            self.columns.len()
        };
        self.columns.truncate(keep.max(1));
        self.open_column(title, path);
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
