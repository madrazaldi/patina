//! Saved library views (`V`): a library's sort and filters under a name,
//! stored in the same place as the desktop app's, so a view saved in either
//! is there in both.

use super::*;
use patina_core::ops::{library, views};
use patina_core::ops::views::{SavedView, TuiExtras};

use super::library::library_title;
use crate::tui::saved;
use crate::tui::source::LibraryView;

impl App {
    /// `V`: the saved views of the library in the last column.
    pub(super) fn open_views(&mut self) {
        let Some(view) = self.columns.last().and_then(|c| c.library()).cloned() else {
            return self.info("Saved views work in a library listing");
        };
        if views::scope(&self.core).is_none() {
            return self.info("Saved views need a server connection");
        }
        let library = self.sections.iter().find(|s| s.key.as_deref() == Some(view.section.as_str())).map(|s| s.title().to_string()).unwrap_or_default();
        self.modal = Some(Modal::Pick(Picker {
            title: format!("Saved views · {library}"),
            kind: PickKind::Views,
            rows: self.view_rows(&view),
            cursor: 0,
            loading: false,
            error: None,
            hint: " enter apply · x delete · esc close ",
        }));
    }

    fn view_rows(&self, view: &LibraryView) -> Vec<(PickEntry, String)> {
        let mut rows = vec![(PickEntry::SaveView, "+ Save the current view…".to_string())];
        for saved_view in views::list(&self.core, &view.section) {
            let summary = saved::describe(&saved_view, &view.section_kind);
            let label = if summary.is_empty() { saved_view.name.clone() } else { format!("{}  ·  {summary}", saved_view.name) };
            rows.push((PickEntry::View(Box::new(saved_view)), label));
        }
        rows
    }

    /// `x` in the list of views: delete the one under the cursor.
    pub(super) fn forget_view(&mut self, mut p: Picker) {
        let doomed = match p.rows.get(p.cursor) {
            Some((PickEntry::View(v), _)) => Some((v.id.clone(), v.name.clone())),
            _ => None,
        };
        if let Some((id, name)) = doomed {
            match views::remove(&self.core, &id) {
                Ok(()) => self.info(format!("Deleted view {name}")),
                Err(e) => self.error(err(e)),
            }
            if let Some(view) = self.columns.last().and_then(|c| c.library()).cloned() {
                p.rows = self.view_rows(&view);
                p.cursor = p.cursor.min(p.rows.len().saturating_sub(1));
            }
        }
        self.modal = Some(Modal::Pick(p));
    }

    /// A row of the list of views was chosen.
    pub(super) fn pick_view(&mut self, entry: PickEntry) {
        match entry {
            PickEntry::SaveView => {
                self.modal = Some(Modal::Input(Input {
                    title: "Save view".into(),
                    hint: "Name".into(),
                    value: String::new(),
                    purpose: InputPurpose::SaveView,
                }));
            }
            PickEntry::View(view) => self.apply_view(*view),
            _ => {}
        }
    }

    pub(super) fn save_view(&mut self, name: &str) {
        if name.is_empty() {
            return;
        }
        let Some(col) = self.columns.last() else { return };
        let (Some(view), Some(scope)) = (col.library(), views::scope(&self.core)) else {
            return self.error("Saved views work in a library listing");
        };
        let saved_view = saved::capture(&scope, name, view, col.hide_watched, &col.filter);
        match views::save(&self.core, saved_view) {
            Ok(()) => self.success(format!("Saved view {name}")),
            Err(e) => self.error(err(e)),
        }
    }

    /// Apply a saved view to the library column: look up the library's sorts
    /// and filters (for the descending sort key and filter titles), then
    /// replace the column's query.
    pub(super) fn apply_view(&mut self, saved_view: SavedView) {
        let Some(col) = self.columns.last() else { return };
        let Some(base) = col.library().map(|v| LibraryView::new(&v.section, &v.section_kind)) else { return };
        let id = col.id;
        let (core, tx) = (self.core.clone(), self.tx.clone());
        // Choosing another view before this one is ready replaces it.
        self.view_task.spawn(async move {
            let sorts = match core.server() {
                Ok(server) => server.section_sorts(&base.section).await.unwrap_or_default(),
                Err(_) => Vec::new(),
            };
            let defs = library::section_filters(&core, &base.section, saved::kind_of(&saved_view, &base)).await.unwrap_or_default();
            let (view, extras) = saved::restore(&saved_view, &base, &sorts, &defs);
            let _ = tx.send(Msg::ViewApplied { id, name: saved_view.name, view: Box::new(view), extras });
        });
    }

    pub(super) fn on_view_applied(&mut self, id: u64, name: String, view: LibraryView, extras: Option<TuiExtras>) {
        let Some(idx) = self.column_index(id) else { return };
        let title = library_title(&self.sections, &view);
        let col = &mut self.columns[idx];
        col.title = title;
        col.set_library(view);
        if let Some(extras) = extras {
            col.hide_watched = extras.hide_watched;
            col.filter = extras.filter;
        }
        col.filtering = false;
        self.load(idx);
        self.success(format!("View {name}"));
    }
}
