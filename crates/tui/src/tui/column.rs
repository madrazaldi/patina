//! One Miller column: a listing plus its own cursor, filter and sort.

use std::cmp::Ordering;

use super::source::{LibraryView, Source};
use crate::item::{Item, Kind};
use crate::search::Fuzzy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Default,
    Title,
    Year,
    Added,
    Rating,
    Duration,
    LastViewed,
    Unwatched,
}

impl SortKey {
    pub const ALL: [SortKey; 8] = [
        SortKey::Default,
        SortKey::Title,
        SortKey::Year,
        SortKey::Added,
        SortKey::Rating,
        SortKey::Duration,
        SortKey::LastViewed,
        SortKey::Unwatched,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SortKey::Default => "Server order",
            SortKey::Title => "Title",
            SortKey::Year => "Year",
            SortKey::Added => "Date added",
            SortKey::Rating => "Rating",
            SortKey::Duration => "Duration",
            SortKey::LastViewed => "Last viewed",
            SortKey::Unwatched => "Unwatched first",
        }
    }

    /// Whether a fresh choice of this key should start descending.
    pub fn natural_desc(self) -> bool {
        matches!(
            self,
            SortKey::Added | SortKey::Rating | SortKey::LastViewed | SortKey::Year
        )
    }

    fn compare(self, a: &Item, b: &Item) -> Ordering {
        match self {
            SortKey::Default => Ordering::Equal,
            SortKey::Title => a
                .sort_title()
                .to_lowercase()
                .cmp(&b.sort_title().to_lowercase()),
            SortKey::Year => a.year.cmp(&b.year),
            SortKey::Added => a.added_at.cmp(&b.added_at),
            SortKey::Rating => a
                .user_rating
                .or(a.best_rating())
                .partial_cmp(&b.user_rating.or(b.best_rating()))
                .unwrap_or(Ordering::Equal),
            SortKey::Duration => a.duration.cmp(&b.duration),
            SortKey::LastViewed => a.last_viewed_at.cmp(&b.last_viewed_at),
            SortKey::Unwatched => unwatched_rank(a).cmp(&unwatched_rank(b)),
        }
    }
}

fn unwatched_rank(i: &Item) -> u8 {
    if i.in_progress() && !i.is_watched() {
        0
    } else if !i.is_watched() {
        1
    } else {
        2
    }
}

pub struct Column {
    pub id: u64,
    pub title: String,
    pub source: Source,
    /// Identity of the entry this column was opened from.
    pub from: Option<String>,
    /// The server path of `source`, if it has one.
    pub path: Option<String>,
    pub items: Vec<Item>,
    /// Visible item indices after hiding, filtering and sorting.
    pub view: Vec<usize>,
    pub cursor: usize,
    pub scroll: usize,
    pub filter: String,
    pub filtering: bool,
    pub sort: SortKey,
    pub sort_desc: bool,
    pub hide_watched: bool,
    pub generation: u64,
    pub loading: bool,
    pub loaded: bool,
    pub error: Option<String>,
    pub total: Option<usize>,
}

impl Column {
    pub fn new(id: u64, title: impl Into<String>, source: Source) -> Self {
        Self {
            id,
            title: title.into(),
            path: source.path(),
            source,
            from: None,
            items: Vec::new(),
            view: Vec::new(),
            cursor: 0,
            scroll: 0,
            filter: String::new(),
            filtering: false,
            sort: SortKey::Default,
            sort_desc: false,
            hide_watched: false,
            generation: 0,
            loading: false,
            loaded: false,
            error: None,
            total: None,
        }
    }

    #[cfg(test)]
    pub fn with_items(mut self, items: Vec<Item>) -> Self {
        self.items = items;
        self.loaded = true;
        self
    }

    pub fn selected(&self) -> Option<&Item> {
        self.view.get(self.cursor).map(|&i| &self.items[i])
    }

    /// Items in view order starting at the cursor (for queueing playback).
    pub fn queue_from_cursor(&self) -> Vec<Item> {
        self.view[self.cursor.min(self.view.len())..]
            .iter()
            .map(|&i| self.items[i].clone())
            .collect()
    }

    pub fn identity(item: &Item) -> String {
        item.playlist_item_id
            .map(|id| id.to_string())
            .or_else(|| item.rating_key.clone())
            .or_else(|| item.children_key())
            .unwrap_or_else(|| item.title().to_string())
    }

    fn selected_identity(&self) -> Option<String> {
        self.selected().map(Self::identity)
    }

    pub fn select_identity(&mut self, id: &str) -> bool {
        if let Some(pos) = self
            .view
            .iter()
            .position(|&i| Self::identity(&self.items[i]) == id)
        {
            self.cursor = pos;
            true
        } else {
            false
        }
    }

    /// Replace the items, keeping the selection on the same entry if it survives.
    pub fn set_items(&mut self, items: Vec<Item>, fuzzy: &mut Fuzzy) {
        let keep = self.selected_identity();
        let old_cursor = self.cursor;
        self.items = items;
        self.rebuild(fuzzy);
        if !keep.is_some_and(|k| self.select_identity(&k)) {
            self.cursor = old_cursor.min(self.view.len().saturating_sub(1));
        }
    }

    pub fn append(&mut self, items: Vec<Item>, fuzzy: &mut Fuzzy) {
        let keep = self.selected_identity();
        self.items.extend(items);
        self.rebuild(fuzzy);
        if let Some(k) = keep {
            self.select_identity(&k);
        }
    }

    /// Recompute the view after any change to items, filter, sort or hiding.
    pub fn rebuild(&mut self, fuzzy: &mut Fuzzy) {
        let visible: Vec<usize> = (0..self.items.len())
            .filter(|&i| {
                let it = &self.items[i];
                !(self.hide_watched && it.is_watchable() && it.is_watched())
            })
            .collect();

        self.view = if self.filter.is_empty() {
            let mut v = visible;
            if self.sort != SortKey::Default {
                let key = self.sort;
                v.sort_by(|&a, &b| key.compare(&self.items[a], &self.items[b]));
            }
            if self.sort_desc {
                v.reverse();
            }
            v
        } else {
            let hay: Vec<String> = visible.iter().map(|&i| haystack(&self.items[i])).collect();
            fuzzy
                .rank(&self.filter, hay.iter().map(String::as_str))
                .into_iter()
                .map(|(pos, _)| visible[pos])
                .collect()
        };
        self.cursor = self.cursor.min(self.view.len().saturating_sub(1));
    }

    pub fn refilter(&mut self, fuzzy: &mut Fuzzy) {
        self.rebuild(fuzzy);
        self.cursor = 0;
        self.scroll = 0;
    }

    pub fn move_by(&mut self, delta: isize) {
        if self.view.is_empty() {
            return;
        }
        let max = self.view.len() as isize - 1;
        self.cursor = (self.cursor as isize + delta).clamp(0, max) as usize;
    }

    pub fn move_to(&mut self, pos: usize) {
        self.cursor = pos.min(self.view.len().saturating_sub(1));
    }

    /// Keep the cursor within the rendered window of `height` rows.
    pub fn clamp_scroll(&mut self, height: usize) {
        if height == 0 {
            return;
        }
        if self.cursor < self.scroll {
            self.scroll = self.cursor;
        } else if self.cursor >= self.scroll + height {
            self.scroll = self.cursor + 1 - height;
        }
        let max_scroll = self.view.len().saturating_sub(height);
        self.scroll = self.scroll.min(max_scroll);
    }

    /// Mutate every item with this rating key; returns whether any matched.
    pub fn patch(&mut self, rating_key: &str, f: &mut dyn FnMut(&mut Item)) -> bool {
        let mut hit = false;
        for it in self
            .items
            .iter_mut()
            .filter(|i| i.rating_key.as_deref() == Some(rating_key))
        {
            f(it);
            hit = true;
        }
        hit
    }

    pub fn is_playlist(&self) -> Option<&str> {
        let rest = self.path.as_deref()?.strip_prefix("/playlists/")?;
        rest.strip_suffix("/items")
    }

    pub fn is_collection(&self) -> Option<&str> {
        let rest = self
            .path
            .as_deref()?
            .strip_prefix("/library/collections/")?;
        rest.strip_suffix("/children")
    }

    /// Section id when this column lists something inside one section.
    pub fn section_id(&self) -> Option<&str> {
        match &self.source {
            Source::Library(v) => return Some(&v.section),
            Source::SectionHubs(id) | Source::Folder { section: id, .. } => return Some(id),
            _ => {}
        }
        let rest = self.path.as_deref()?.strip_prefix("/library/sections/")?;
        rest.split(['/', '?']).next()
    }

    pub fn library(&self) -> Option<&LibraryView> {
        match &self.source {
            Source::Library(v) => Some(v),
            _ => None,
        }
    }

    /// Change a library listing's query; it reloads from the top.
    pub fn set_library(&mut self, view: LibraryView) {
        self.path = Some(view.path());
        self.source = Source::Library(view);
        self.items.clear();
        self.view.clear();
        self.cursor = 0;
        self.scroll = 0;
        self.loaded = false;
    }

    pub fn is_sessions(&self) -> bool {
        self.source == Source::Sessions
    }

    pub fn count_label(&self) -> String {
        let shown = self.view.len();
        let all = self.items.len();
        if shown != all {
            format!("{shown}/{all}")
        } else if self.loading && self.total.is_some_and(|t| t > all) {
            format!("{all}/{}", self.total.unwrap_or(0))
        } else {
            all.to_string()
        }
    }
}

fn haystack(i: &Item) -> String {
    match i.kind() {
        Kind::Episode | Kind::Track | Kind::Album | Kind::Season => {
            format!("{} {}", i.display_title(), i.full_title())
        }
        _ => match i.year {
            Some(y) => format!("{} {y}", i.title()),
            None => i.title().to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn it(key: &str, title: &str, year: i64, watched: bool) -> Item {
        let mut i = crate::item::test_item("movie", title);
        i.rating_key = Some(key.into());
        i.year = Some(year);
        i.view_count = Some(watched as i64);
        i
    }

    fn col() -> Column {
        let mut c = Column::new(1, "Movies", Source::Path("/library/sections/3/all?type=1".into()))
            .with_items(vec![
                it("1", "Heat", 1995, true),
                it("2", "Alien", 1979, false),
                it("3", "Zodiac", 2007, false),
            ]);
        c.rebuild(&mut Fuzzy::default());
        c
    }

    #[test]
    fn sorts_hides_and_filters() {
        let mut f = Fuzzy::default();
        let mut c = col();
        c.sort = SortKey::Title;
        c.rebuild(&mut f);
        assert_eq!(c.selected().unwrap().title(), "Alien");

        c.sort = SortKey::Year;
        c.sort_desc = true;
        c.rebuild(&mut f);
        assert_eq!(c.selected().unwrap().title(), "Zodiac");

        c.hide_watched = true;
        c.rebuild(&mut f);
        assert_eq!(c.view.len(), 2);
        assert_eq!(c.count_label(), "2/3");

        c.hide_watched = false;
        c.filter = "zod".into();
        c.refilter(&mut f);
        assert_eq!(c.view.len(), 1);
        assert_eq!(c.selected().unwrap().title(), "Zodiac");
    }

    #[test]
    fn set_items_keeps_selection_by_identity() {
        let mut f = Fuzzy::default();
        let mut c = col();
        c.move_to(2); // Zodiac
        c.set_items(
            vec![it("3", "Zodiac", 2007, false), it("9", "New", 2024, false)],
            &mut f,
        );
        assert_eq!(c.selected().unwrap().title(), "Zodiac");
        assert_eq!(c.cursor, 0);
    }

    #[test]
    fn path_helpers() {
        let c = col();
        assert_eq!(c.section_id(), Some("3"));
        let p = Column::new(2, "P", Source::Path("/playlists/77/items".into()));
        assert_eq!(p.is_playlist(), Some("77"));
        let k = Column::new(3, "C", Source::Path("/library/collections/5/children".into()));
        assert_eq!(k.is_collection(), Some("5"));
    }

    #[test]
    fn scroll_follows_cursor() {
        let mut c = col();
        c.move_to(2);
        c.clamp_scroll(2);
        assert_eq!(c.scroll, 1);
        c.move_to(0);
        c.clamp_scroll(2);
        assert_eq!(c.scroll, 0);
    }
}
