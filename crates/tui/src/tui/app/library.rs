//! Library listings as the GUI's Library page offers them: views (types,
//! Recommended, Collections, Folders), server sorts, filters and A–Z.

use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use patina_core::ops::library::{self, FilterView};

use super::nav::library_view;
use crate::tui::column::SortKey;
use crate::tui::source::{Filter, LibraryView, letter_starts, section_types};

impl App {
    /// The section of the selection or of the last column.
    fn current_section(&self) -> Option<Item> {
        let selected = self.selected().filter(|i| i.kind() == Kind::Section).and_then(|i| i.key.clone());
        let id = self.columns.last()?.section_id().map(str::to_string).or(selected)?;
        self.sections.iter().find(|s| s.key.as_deref() == Some(&id)).cloned()
    }

    /// The library listing in the last column, if that's what it shows.
    fn library(&mut self, what: &str) -> Option<LibraryView> {
        let view = self.columns.last().and_then(|c| c.library()).cloned();
        if view.is_none() {
            self.info(format!("{what} works in a library listing"));
        }
        view
    }

    /// Change the last column's library query and reload it.
    fn update_library(&mut self, f: impl FnOnce(&mut LibraryView)) {
        let idx = self.columns.len() - 1;
        let Some(mut view) = self.columns[idx].library().cloned() else { return };
        f(&mut view);
        let title = library_title(&self.sections, &view);
        let col = &mut self.columns[idx];
        col.title = title;
        col.set_library(view);
        self.load(idx);
    }

    // ---- views -------------------------------------------------------------

    pub(super) fn open_browse(&mut self) {
        let Some(section) = self.current_section() else {
            self.info("Views are for a library");
            return;
        };
        let Some(base) = library_view(&section) else { return };
        let id = base.section.clone();
        let mut rows: Vec<(String, Source)> = match section_types(&base.section_kind) {
            [] => vec![("Library".into(), Source::Library(base.clone()))],
            types => types
                .iter()
                .map(|(k, label)| {
                    let mut v = base.clone();
                    v.set_kind(*k);
                    (label.to_string(), Source::Library(v))
                })
                .collect(),
        };
        rows.push(("Recommended".into(), Source::SectionHubs(id.clone())));
        rows.push(("Collections".into(), Source::Path(format!("/library/sections/{id}/collections"))));
        rows.push(("Folders".into(), Source::Folder { section: id.clone(), key: None }));
        rows.extend(BROWSE_VIEWS.iter().map(|(label, sub)| (label.to_string(), Source::Path(format!("/library/sections/{id}/{sub}")))));
        // Start on the view shown now.
        let current = self.columns.last().map(|c| match &c.source {
            Source::Library(v) => Source::Library(LibraryView { kind: v.kind, ..base.clone() }),
            s => s.clone(),
        });
        let cursor = rows.iter().position(|(_, s)| Some(s) == current.as_ref()).unwrap_or(0);
        self.modal = Some(Modal::Browse { section_id: id, section_title: section.title().to_string(), rows, cursor });
    }

    /// Show a view of a library, replacing its column (or opening one from the root).
    pub(super) fn browse_to(&mut self, section_id: &str, section_title: &str, label: &str, source: Source) {
        let title = match &source {
            Source::Library(v) => library_title(&self.sections, v),
            _ => format!("{section_title} · {label}"),
        };
        let keep = if self.columns.last().and_then(|c| c.section_id()) == Some(section_id) {
            self.columns.iter().position(|c| c.section_id() == Some(section_id)).unwrap_or(1)
        } else {
            self.columns.len()
        };
        self.columns.truncate(keep.max(1));
        self.open_source(title, source, Vec::new());
    }

    /// `t`: the next type (shows → seasons → episodes …).
    pub(super) fn cycle_type(&mut self) {
        let Some(view) = self.library("Switching type") else { return };
        let types = section_types(&view.section_kind);
        if types.is_empty() {
            self.info("This library lists one type");
            return;
        }
        let at = types.iter().position(|t| Some(t.0) == view.kind).map_or(0, |i| (i + 1) % types.len());
        let (kind, label) = types[at];
        self.update_library(|v| v.set_kind(kind));
        self.info(format!("Showing {}", label.to_lowercase()));
    }

    // ---- sorts ---------------------------------------------------------------

    /// `s` in a library: the server's sorts.
    pub(super) fn open_sort(&mut self) {
        let Some(view) = self.library("Sorting") else { return };
        let server = self.server();
        self.open_picker(
            "Sort".into(),
            PickKind::Sort,
            " enter choose (again to reverse) · esc close ",
            async move {
                let mut sorts = server?.section_sorts(&view.section).await.map_err(err)?;
                if sorts.is_empty() {
                    sorts = [("titleSort", "Title"), ("addedAt", "Date added"), ("originallyAvailableAt", "Release date"), ("rating", "Rating")]
                        .into_iter()
                        .map(|(k, t)| patina_plex::Directory { key: Some(k.into()), title: Some(t.into()), ..Default::default() })
                        .collect();
                }
                Ok(sorts
                    .into_iter()
                    .filter(|d| d.key.is_some())
                    .map(|d| {
                        let on = d.key.as_deref() == Some(view.sort.as_str());
                        let dir = if view.desc { "↓" } else { "↑" };
                        let label = format!("{} {}{}", if on { "●" } else { " " }, d.title.as_deref().unwrap_or(""), if on { format!(" {dir}") } else { String::new() });
                        (PickEntry::Sort(Box::new(d)), label)
                    })
                    .collect())
            },
        );
    }

    pub(super) fn choose_sort(&mut self, d: patina_plex::Directory) {
        let Some(key) = d.key.clone() else { return };
        self.update_library(|v| {
            if v.sort == key {
                v.desc = !v.desc;
            } else {
                v.desc = d.default_direction.as_deref() == Some("desc");
                v.sort = key;
                v.sort_title = d.title.clone().unwrap_or_default();
                v.desc_key = d.desc_key.clone();
            }
        });
    }

    // ---- A–Z -------------------------------------------------------------------

    /// `A`: jump to a letter, from the server's first-character index.
    pub(super) fn open_letters(&mut self) {
        let Some(view) = self.library("Jumping to a letter") else { return };
        if !view.alphabetical() {
            self.info("Jump to a letter works when sorted by title, A–Z, without filters");
            return;
        }
        let server = self.server();
        self.open_picker("Jump to".into(), PickKind::Letters, " type a letter · enter jump · esc close ", async move {
            let chars = server?.section_first_characters(&view.section, view.kind).await.map_err(err)?;
            Ok(letter_starts(&chars).into_iter().map(|(l, at, n)| (PickEntry::Letter(l.clone(), at), format!("{l:<3} {n}"))).collect())
        });
    }

    pub(super) fn jump_to_letter(&mut self, letter: &str, at: usize) {
        let Some(col) = self.columns.last_mut() else { return };
        if col.pager.is_some() {
            // A windowed listing is in server order; the page loads as the cursor lands there.
            return col.move_to(at);
        }
        let server_order = col.sort == SortKey::Default && col.filter.is_empty() && !col.hide_watched;
        let local = col.view.iter().position(|&i| first_letter(col.items[i].sort_title()) == letter);
        match (server_order && at < col.view.len(), local) {
            (true, _) => col.move_to(at),
            (false, Some(pos)) => col.move_to(pos),
            (false, None) => col.move_to(at),
        }
    }

    // ---- filters -------------------------------------------------------------

    /// `F`: the server's filters for the listed type.
    pub(super) fn open_filters(&mut self) {
        let Some(view) = self.library("Filtering") else { return };
        let section_title = self.sections.iter().find(|s| s.key.as_deref() == Some(&view.section)).map(|s| s.title().to_string()).unwrap_or_default();
        self.modal = Some(Modal::Filter(FilterPicker { section_title, defs: Vec::new(), loading: true, error: None, cursor: 0, values: None }));
        let (core, tx) = (self.core.clone(), self.tx.clone());
        tokio::spawn(async move {
            let result = library::section_filters(&core, &view.section, view.kind).await.map_err(err).map(|defs| {
                // As on the GUI, Unwatched is always offered for watchable libraries.
                let mut defs: Vec<FilterView> = defs.into_iter().filter(|f| f.filter != "unwatched").collect();
                if view.watchable() {
                    defs.insert(0, FilterView { filter: "unwatched".into(), title: "Unwatched".into(), kind: "boolean".into() });
                }
                defs
            });
            let _ = tx.send(Msg::FilterDefs(result));
        });
    }

    fn load_filter_values(&mut self, def: &FilterView) {
        let Some(view) = self.columns.last().and_then(|c| c.library()).cloned() else { return };
        let (server, tx, filter) = (self.server(), self.tx.clone(), def.filter.clone());
        tokio::spawn(async move {
            let result = async {
                let dirs = server?.section_filter_values(&view.section, &filter, view.kind).await.map_err(err)?;
                Ok(dirs.into_iter().filter_map(|d| Some((d.key.clone()?, d.title.clone().or(d.key)?))).collect())
            }
            .await;
            let _ = tx.send(Msg::FilterValues { filter, result });
        });
    }

    pub(super) fn filter_modal_key(&mut self, mut p: FilterPicker, key: KeyEvent) -> Option<Modal> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let active = self.columns.last().and_then(|c| c.library()).map_or(0, |v| v.filters.len());
        if let Some(mut vals) = p.values.take() {
            let n = vals.shown().len();
            match key.code {
                KeyCode::Esc | KeyCode::Left => return Some(Modal::Filter(p)),
                KeyCode::Down => vals.cursor = (vals.cursor + 1).min(n.saturating_sub(1)),
                KeyCode::Char('n' | 'j') if ctrl => vals.cursor = (vals.cursor + 1).min(n.saturating_sub(1)),
                KeyCode::Up => vals.cursor = vals.cursor.saturating_sub(1),
                KeyCode::Char('p' | 'k') if ctrl => vals.cursor = vals.cursor.saturating_sub(1),
                KeyCode::Enter => {
                    if let Some((value, label)) = vals.shown().get(vals.cursor).map(|v| (*v).clone()) {
                        let f = Filter { filter: vals.def.filter.clone(), title: vals.def.title.clone(), value, label };
                        self.update_library(|v| v.toggle(f));
                    }
                }
                KeyCode::Backspace => {
                    if vals.query.pop().is_none() {
                        return Some(Modal::Filter(p));
                    }
                    vals.cursor = 0;
                }
                KeyCode::Char(c) if !ctrl => {
                    vals.query.push(c);
                    vals.cursor = 0;
                }
                _ => {}
            }
            p.values = Some(vals);
            return Some(Modal::Filter(p));
        }
        let rows = p.defs.len() + usize::from(active > 0);
        match key.code {
            KeyCode::Esc | KeyCode::Char('q' | 'h') | KeyCode::Left => return None,
            KeyCode::Char('j') | KeyCode::Down => p.cursor = (p.cursor + 1).min(rows.saturating_sub(1)),
            KeyCode::Char('k') | KeyCode::Up => p.cursor = p.cursor.saturating_sub(1),
            KeyCode::Enter | KeyCode::Char(' ' | 'l') | KeyCode::Right if !p.loading => match p.defs.get(p.cursor).cloned() {
                Some(def) if def.kind == "boolean" => {
                    let f = Filter { filter: def.filter, title: def.title, value: "1".into(), label: String::new() };
                    self.update_library(|v| v.toggle(f));
                }
                Some(def) => {
                    self.load_filter_values(&def);
                    p.values = Some(FilterValues { def, all: Vec::new(), loading: true, error: None, query: String::new(), cursor: 0 });
                }
                None if active > 0 => {
                    self.update_library(|v| v.filters.clear());
                    p.cursor = 0;
                }
                None => {}
            },
            _ => {}
        }
        Some(Modal::Filter(p))
    }
}

/// `TV Shows`, or `TV Shows · Episodes` when not the section's first type.
pub(super) fn library_title(sections: &[Item], v: &LibraryView) -> String {
    let name = sections.iter().find(|s| s.key.as_deref() == Some(&v.section)).map(|s| s.title().to_string()).unwrap_or_default();
    let first = section_types(&v.section_kind).first().map(|t| t.0);
    match v.type_label() {
        Some(label) if v.kind != first => format!("{name} · {label}"),
        _ => name,
    }
}

/// The A–Z rail letter of a title: its first letter, or `#`.
fn first_letter(title: &str) -> String {
    match title.chars().next() {
        Some(c) if c.is_alphabetic() => c.to_uppercase().collect(),
        _ => "#".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters() {
        assert_eq!(first_letter("alien"), "A");
        assert_eq!(first_letter("2001"), "#");
        assert_eq!(first_letter(""), "#");
    }
}
