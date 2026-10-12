//! Saved views: turning a library listing's query into the entry the desktop
//! app also reads (`ui.saved_views`), and back.

use patina_core::ops::library::FilterView;
use patina_core::ops::views::{Picked, SavedView, TuiExtras};
use patina_plex::Directory;

use super::source::{Filter, LibraryView, section_types};

const UNWATCHED: &str = "unwatched";

/// A view called `name` for the library listing `view`, as the desktop app
/// stores it: boolean filters are `toggles`, the unwatched filter has its own
/// flag, and the rest are `picked`. What only the terminal has (hiding watched
/// titles, the `/` filter) is kept apart in `tui`, which the desktop app ignores.
pub fn capture(scope: &str, name: &str, view: &LibraryView, hide_watched: bool, text_filter: &str) -> SavedView {
    let mut saved = SavedView::new(scope, &view.section, name);
    saved.media_type = view.kind.map_or(0, i64::from);
    saved.sort = view.sort.clone();
    saved.desc = view.desc;
    for f in &view.filters {
        if f.filter == UNWATCHED && f.value == "1" {
            saved.unwatched = true;
        } else if f.value == "1" && f.label.is_empty() {
            saved.toggles.push(f.filter.clone());
        } else {
            saved.picked.push(Picked { filter: f.filter.clone(), title: f.title.clone(), value: f.value.clone(), label: f.label.clone() });
        }
    }
    saved.tui = Some(TuiExtras { hide_watched, filter: text_filter.to_string() });
    saved
}

/// The Plex type a saved view lists: its own, or the library's first.
pub fn kind_of(saved: &SavedView, base: &LibraryView) -> Option<u32> {
    match u32::try_from(saved.media_type) {
        Ok(k) if k > 0 => Some(k),
        _ => base.kind,
    }
}

/// The listing a saved view describes, on top of `base` (the library as it
/// opens). `sorts` and `defs` are the library's sorts and filters, used to
/// find the descending sort key and the titles of boolean filters; without
/// them the view still applies, only with generic names. A view saved by the
/// desktop app has no terminal settings (`None`): the column keeps its own.
pub fn restore(saved: &SavedView, base: &LibraryView, sorts: &[Directory], defs: &[FilterView]) -> (LibraryView, Option<TuiExtras>) {
    let mut view = base.clone();
    view.kind = kind_of(saved, base);
    if !saved.sort.is_empty() {
        view.sort = saved.sort.clone();
    }
    view.desc = saved.desc;
    let sort = sorts.iter().find(|d| d.key.as_deref() == Some(view.sort.as_str()));
    view.sort_title = sort.and_then(|d| d.title.clone()).unwrap_or_else(|| view.sort.clone());
    view.desc_key = sort.and_then(|d| d.desc_key.clone());
    view.filters.clear();
    let boolean = |key: &str, title: Option<&str>| Filter { filter: key.to_string(), title: title.unwrap_or(key).to_string(), value: "1".into(), label: String::new() };
    let title_of = |key: &str| defs.iter().find(|d| d.filter == key).map(|d| d.title.as_str());
    if saved.unwatched {
        view.filters.push(boolean(UNWATCHED, Some("Unwatched")));
    }
    for t in &saved.toggles {
        view.filters.push(boolean(t, title_of(t)));
    }
    for p in &saved.picked {
        view.filters.push(Filter { filter: p.filter.clone(), title: p.title.clone(), value: p.value.clone(), label: p.label.clone() });
    }
    (view, saved.tui.clone())
}

/// A one-line summary of what a view selects, for the list of views.
pub fn describe(saved: &SavedView, section_kind: &str) -> String {
    let mut parts = Vec::new();
    let types = section_types(section_kind);
    if let Some((_, label)) = types.iter().skip(1).find(|t| i64::from(t.0) == saved.media_type) {
        parts.push((*label).to_string());
    }
    if saved.unwatched || saved.tui.as_ref().is_some_and(|t| t.hide_watched) {
        parts.push("Unwatched".into());
    }
    parts.extend(saved.toggles.iter().cloned());
    parts.extend(saved.picked.iter().map(|p| format!("{}: {}", p.title, p.label)));
    if let Some(t) = saved.tui.as_ref().filter(|t| !t.filter.is_empty()) {
        parts.push(format!("/{}", t.filter));
    }
    if (!saved.sort.is_empty() && saved.sort != "titleSort") || saved.desc {
        parts.push(format!("{} {}", saved.sort, if saved.desc { "↓" } else { "↑" }));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shows() -> LibraryView {
        LibraryView::new("2", "show")
    }

    fn filter(f: &str, title: &str, value: &str, label: &str) -> Filter {
        Filter { filter: f.into(), title: title.into(), value: value.into(), label: label.into() }
    }

    #[test]
    fn a_listing_is_captured_the_way_the_desktop_app_stores_it() {
        let mut v = shows();
        v.set_kind(4);
        v.sort = "addedAt".into();
        v.desc = true;
        v.filters = vec![filter("unwatched", "Unwatched", "1", ""), filter("hdr", "HDR", "1", ""), filter("genre", "Genre", "10", "Drama"), filter("year", "Year", "1", "1")];
        let saved = capture("acct:srv", " Fresh drama ", &v, false, "");
        assert_eq!((saved.name.as_str(), saved.scope.as_str(), saved.library.as_str()), ("Fresh drama", "acct:srv", "2"));
        assert_eq!((saved.media_type, saved.sort.as_str(), saved.desc, saved.unwatched), (4, "addedAt", true, true));
        assert_eq!(saved.toggles, ["hdr"]);
        let picked: Vec<_> = saved.picked.iter().map(|p| (p.filter.as_str(), p.value.as_str())).collect();
        assert_eq!(picked, [("genre", "10"), ("year", "1")], "a value of 1 with a label is a choice, not a switch");
        assert_eq!(saved.tui, Some(TuiExtras::default()), "the terminal's own settings are recorded too");
    }

    #[test]
    fn terminal_only_settings_are_kept_apart() {
        let saved = capture("a:s", "x", &shows(), true, "dune");
        assert_eq!(saved.tui, Some(TuiExtras { hide_watched: true, filter: "dune".into() }));
        let (_, extras) = restore(&saved, &shows(), &[], &[]);
        assert_eq!(extras, Some(TuiExtras { hide_watched: true, filter: "dune".into() }));
        // A view from the desktop app has none, so the column keeps its own.
        let mut gui = saved.clone();
        gui.tui = None;
        assert_eq!(restore(&gui, &shows(), &[], &[]).1, None);
    }

    #[test]
    fn restoring_gives_back_the_same_listing() {
        let mut v = shows();
        v.sort = "rating".into();
        v.desc = true;
        v.desc_key = Some("rating:desc".into());
        v.sort_title = "Rating".into();
        v.filters = vec![filter("unwatched", "Unwatched", "1", ""), filter("hdr", "HDR", "1", ""), filter("genre", "Genre", "10", "Drama")];
        let saved = capture("a:s", "x", &v, false, "");
        let sorts = vec![Directory { key: Some("rating".into()), title: Some("Rating".into()), desc_key: Some("rating:desc".into()), ..Default::default() }];
        let defs = vec![FilterView { filter: "hdr".into(), title: "HDR".into(), kind: "boolean".into() }];
        let (back, _) = restore(&saved, &shows(), &sorts, &defs);
        assert_eq!(back, v);
        assert_eq!(back.path(), v.path());
    }

    #[test]
    fn a_desktop_view_applies_without_server_help() {
        let saved: SavedView = serde_json::from_value(serde_json::json!({
            "id": "1", "name": "n", "scope": "a:s", "library": "2", "mediaType": 0,
            "sort": "titleSort", "desc": false, "unwatched": true, "toggles": ["hdr"], "picked": []
        }))
        .unwrap();
        let (v, _) = restore(&saved, &shows(), &[], &[]);
        assert_eq!(v.kind, shows().kind, "type 0 is the library's first type");
        assert_eq!(v.desc_key, None);
        assert_eq!(v.filters.iter().map(|f| f.filter.as_str()).collect::<Vec<_>>(), ["unwatched", "hdr"]);
        assert_eq!(v.filters[1].title, "hdr");
        assert!(v.path().contains("unwatched=1") && v.path().contains("hdr=1"));
    }

    #[test]
    fn views_are_described_in_a_line() {
        let mut v = shows();
        v.set_kind(4);
        v.sort = "addedAt".into();
        v.desc = true;
        v.filters = vec![filter("unwatched", "Unwatched", "1", ""), filter("genre", "Genre", "10", "Drama")];
        let saved = capture("a:s", "x", &v, false, "");
        assert_eq!(describe(&saved, "show"), "Episodes · Unwatched · Genre: Drama · addedAt ↓");
        assert_eq!(describe(&capture("a:s", "x", &shows(), false, ""), "show"), "");
    }
}
