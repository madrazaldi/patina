use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Msg, SEARCH_DELAY, SearchState, err};
use crate::item::Item;
use crate::tui::column::Column;
use crate::tui::source::{Source, search_groups};

/// Results per type, as on the GUI.
const PER_TYPE: u32 = 24;
/// Shorter queries aren't sent, as on the GUI.
const MIN_QUERY: usize = 2;

impl SearchState {
    pub fn set_groups(&mut self, groups: Vec<Item>) {
        self.results.clear();
        self.group_of.clear();
        for (g, group) in groups.iter().enumerate() {
            let items = group.link.as_ref().map_or(&[][..], |l| &l.items[..]);
            self.results.extend(items.iter().cloned());
            self.group_of.extend(std::iter::repeat_n(g, items.len()));
        }
        self.groups = groups;
    }
}

impl App {
    /// Run the query once typing pauses.
    pub(super) fn search_tick(&mut self, now: Instant) {
        let Some(s) = &mut self.search else { return };
        if !s.dirty_at.is_some_and(|d| now.duration_since(d) >= SEARCH_DELAY) {
            return;
        }
        s.dirty_at = None;
        let query = s.input.trim().to_string();
        if query.chars().count() < MIN_QUERY {
            s.set_groups(Vec::new());
            s.pending = false;
            return;
        }
        s.pending = true;
        if self.xs.on {
            return self.search_all_servers(query);
        }
        let (core, tx) = (self.core.clone(), self.tx.clone());
        tokio::spawn(async move {
            let result = match core.server() {
                Ok(server) => server.search_hubs(&query, PER_TYPE, None).await.map(search_groups).map_err(err),
                Err(e) => Err(err(e)),
            };
            let _ = tx.send(match result {
                Ok(groups) => Msg::SearchResults { query, groups },
                Err(e) => Msg::SearchFailed(query, e),
            });
        });
    }

    pub(super) fn open_search(&mut self) {
        self.search = Some(SearchState {
            input: String::new(),
            groups: Vec::new(),
            results: Vec::new(),
            group_of: Vec::new(),
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
        s.pending = s.input.trim().chars().count() >= MIN_QUERY;
        self.after_selection_change();
    }

    /// Jump from a search result into the browser: a column of result
    /// types, then the chosen type's results, then into the result if it opens.
    fn search_accept(&mut self) {
        let Some(s) = self.search.take() else { return };
        let (Some(item), Some(&g)) = (s.results.get(s.cursor).cloned(), s.group_of.get(s.cursor)) else { return };
        self.columns.truncate(1);
        let group = Column::identity(&s.groups[g]);
        self.open_source(format!("Search: {}", s.input.trim()), Source::Static, s.groups);
        if let Some(col) = self.columns.last_mut() {
            col.select_identity(&group);
        }
        self.drill();
        if let Some(col) = self.columns.last_mut() {
            col.select_identity(&Column::identity(&item));
        }
        if item.is_container() {
            self.drill();
        }
        self.after_selection_change();
    }

    pub(super) fn search_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if key.code == KeyCode::Enter && self.xs.on && self.search_all_accept() {
            return;
        }
        let Some(s) = &mut self.search else { return };
        let last = s.results.len().saturating_sub(1);
        match key.code {
            KeyCode::Esc => self.search = None,
            KeyCode::Char('a') if ctrl => self.toggle_all_servers(),
            KeyCode::Enter => self.search_accept(),
            KeyCode::Down => s.cursor = (s.cursor + 1).min(last),
            KeyCode::Char('n' | 'j') if ctrl => s.cursor = (s.cursor + 1).min(last),
            KeyCode::Up => s.cursor = s.cursor.saturating_sub(1),
            KeyCode::Char('p' | 'k') if ctrl => s.cursor = s.cursor.saturating_sub(1),
            // Next / previous result type.
            KeyCode::Tab => {
                let g = s.group_of.get(s.cursor).copied().unwrap_or(0);
                s.cursor = s.group_of.iter().position(|&x| x > g).unwrap_or(s.cursor);
            }
            KeyCode::BackTab => {
                let g = s.group_of.get(s.cursor).copied().unwrap_or(0);
                let target = if s.group_of.get(s.cursor.wrapping_sub(1)) == Some(&g) { g } else { g.saturating_sub(1) };
                s.cursor = s.group_of.iter().position(|&x| x == target).unwrap_or(0);
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_flatten_in_order() {
        let group = |t: &str, n: usize| {
            let items = (0..n).map(|i| crate::item::test_item("movie", &format!("{t}{i}"))).collect();
            Item::link(t, "hub", Source::Static, items)
        };
        let mut s = SearchState {
            input: String::new(),
            groups: Vec::new(),
            results: Vec::new(),
            group_of: Vec::new(),
            cursor: 0,
            scroll: 0,
            pending: false,
            dirty_at: None,
            error: None,
        };
        s.set_groups(vec![group("a", 2), group("b", 1)]);
        assert_eq!(s.results.iter().map(|i| i.title()).collect::<Vec<_>>(), ["a0", "a1", "b0"]);
        assert_eq!(s.group_of, [0, 0, 1]);
    }
}
