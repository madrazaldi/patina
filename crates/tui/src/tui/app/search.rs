use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Msg, SEARCH_DELAY, SearchState, err};
use crate::fetch::search;
use crate::tui::source::Source;

impl App {
    /// Run the query once typing pauses.
    pub(super) fn search_tick(&mut self, now: Instant) {
        if let Some(s) = &mut self.search
            && s.dirty_at.is_some_and(|d| now.duration_since(d) >= SEARCH_DELAY)
        {
            s.dirty_at = None;
            let query = s.input.trim().to_string();
            if query.is_empty() {
                s.results.clear();
                s.pending = false;
            } else {
                s.pending = true;
                let (core, tx) = (self.core.clone(), self.tx.clone());
                tokio::spawn(async move {
                    let result = match core.server() {
                        Ok(server) => search(&server, &query, 30).await.map_err(err),
                        Err(e) => Err(err(e)),
                    };
                    let _ = tx.send(match result {
                        Ok(items) => Msg::SearchResults { query, items },
                        Err(e) => Msg::SearchFailed(query, e),
                    });
                });
            }
        }
    }

    // ---- search ----------------------------------------------------------

    pub(super) fn open_search(&mut self) {
        self.search = Some(SearchState {
            input: String::new(),
            results: Vec::new(),
            cursor: 0,
            scroll: 0,
            pending: false,
            dirty_at: None,
            error: None,
        });
    }

    pub(super) fn search_changed(&mut self) {
        let Some(s) = &mut self.search else { return };
        s.error = None;
        s.dirty_at = Some(Instant::now());
        s.pending = !s.input.trim().is_empty();
        self.after_selection_change();
    }

    /// Jump from a search result into the browser.
    fn search_accept(&mut self) {
        let Some(s) = self.search.take() else { return };
        let Some(item) = s.results.get(s.cursor).cloned() else { return };
        self.columns.truncate(1);
        let mut col = self.new_column(format!("Search: {}", s.input.trim()), Source::Static);
        col.items = s.results;
        col.loaded = true;
        col.rebuild(&mut self.fuzzy);
        col.cursor = s.cursor.min(col.view.len().saturating_sub(1));
        self.columns.push(col);
        if item.is_container() {
            self.drill();
        }
        self.after_selection_change();
    }

    pub(super) fn search_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let Some(s) = &mut self.search else { return };
        match key.code {
            KeyCode::Esc => self.search = None,
            KeyCode::Enter => self.search_accept(),
            KeyCode::Down => s.cursor = (s.cursor + 1).min(s.results.len().saturating_sub(1)),
            KeyCode::Char('n' | 'j') if ctrl => s.cursor = (s.cursor + 1).min(s.results.len().saturating_sub(1)),
            KeyCode::Up => s.cursor = s.cursor.saturating_sub(1),
            KeyCode::Char('p' | 'k') if ctrl => s.cursor = s.cursor.saturating_sub(1),
            KeyCode::Char('u') if ctrl => {
                s.input.clear();
                self.search_changed();
            }
            KeyCode::Backspace => {
                s.input.pop();
                self.search_changed();
            }
            KeyCode::Char(c) if !ctrl => {
                s.input.push(c);
                self.search_changed();
            }
            _ => {}
        }
        self.after_selection_change();
    }
}
