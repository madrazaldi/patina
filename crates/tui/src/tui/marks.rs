//! Items marked for a bulk action, like the desktop app's selection mode.

use std::collections::HashMap;

use super::column::Column;
use crate::item::{Item, Kind};

/// Whether an entry can be marked: a library item the server can act on.
pub fn markable(item: &Item) -> bool {
    item.row.is_none()
        && item.rating_key.is_some()
        && matches!(
            item.kind(),
            Kind::Movie | Kind::Show | Kind::Season | Kind::Episode | Kind::Artist | Kind::Album | Kind::Track | Kind::Clip | Kind::Collection
        )
}

/// The marked items of one column, in the order they were marked. Items are
/// kept whole, so marks survive the column reloading or paging them out.
#[derive(Default)]
pub struct Marks {
    order: Vec<String>,
    items: HashMap<String, Item>,
}

impl Marks {
    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn contains(&self, item: &Item) -> bool {
        self.items.contains_key(&Column::identity(item))
    }

    /// Mark `item`, or unmark it if it was; returns whether it is marked now.
    pub fn toggle(&mut self, item: &Item) -> bool {
        let id = Column::identity(item);
        if self.items.remove(&id).is_some() {
            self.order.retain(|x| *x != id);
            false
        } else {
            self.order.push(id.clone());
            self.items.insert(id, item.clone());
            true
        }
    }

    /// Mark `item` unless it already is.
    pub fn add(&mut self, item: &Item) {
        if !self.contains(item) {
            self.toggle(item);
        }
    }

    pub fn clear(&mut self) {
        self.order.clear();
        self.items.clear();
    }

    /// The marked items in marking order, with the identity each is marked under.
    pub fn entries(&self) -> Vec<(String, Item)> {
        self.order.iter().filter_map(|id| Some((id.clone(), self.items.get(id)?.clone()))).collect()
    }

    pub fn items(&self) -> Vec<Item> {
        self.entries().into_iter().map(|(_, i)| i).collect()
    }
}

/// What a bulk action does to every marked item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BulkOp {
    PlayNext,
    AddToQueue,
    Download,
    Watched,
    Unwatched,
    Playlist,
    Collection,
    /// Take them out of the playlist or collection the column shows.
    Remove,
    Clear,
}

/// The playlist type shared by every item (`video` or `audio`), or why there is none.
pub fn playlist_kind(items: &[Item]) -> Result<&'static str, &'static str> {
    let first = items.first().ok_or("Nothing is marked")?;
    let kind = first.playlist_kind();
    if kind == "photo" {
        return Err("Photos can't go in a playlist");
    }
    if items.iter().any(|i| i.playlist_kind() != kind) {
        return Err("Mark only video or only music for a playlist");
    }
    Ok(kind)
}

/// The Plex type shared by every item, for a collection, or why there is none.
pub fn collection_kind(items: &[Item]) -> Result<&str, &'static str> {
    let first = items.first().ok_or("Nothing is marked")?;
    let kind = first.meta.kind.as_deref().unwrap_or("");
    if !matches!(kind, "movie" | "show" | "artist" | "album") {
        return Err("Collections hold movies, shows, artists and albums");
    }
    if items.iter().any(|i| i.meta.kind.as_deref() != Some(kind)) {
        return Err("Mark items of one type for a collection");
    }
    Ok(kind)
}

/// The result of a batch of per-item calls: `ok` of `total` worked, `failures`
/// say why the others didn't. Nothing failing reports `done`; anything failing
/// is an error that names the first few.
pub fn outcome(done: &str, total: usize, failures: &[String]) -> Result<Option<String>, String> {
    if failures.is_empty() {
        return Ok(Some(done.to_string()));
    }
    let shown = failures.iter().take(2).cloned().collect::<Vec<_>>().join("; ");
    let more = failures.len().saturating_sub(2);
    let more = if more > 0 { format!(" (+{more} more)") } else { String::new() };
    Err(format!("{} of {total} done. {shown}{more}", total - failures.len().min(total)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(kind: &str, key: &str) -> Item {
        let mut i = crate::item::test_item(kind, key);
        i.rating_key = Some(key.into());
        i
    }

    #[test]
    fn marking_keeps_order_and_toggles() {
        let mut m = Marks::default();
        let (a, b, c) = (item("movie", "1"), item("movie", "2"), item("movie", "3"));
        assert!(m.toggle(&b));
        assert!(m.toggle(&a));
        m.add(&c);
        m.add(&c);
        assert_eq!(m.len(), 3);
        assert_eq!(m.items().iter().map(|i| i.title().to_string()).collect::<Vec<_>>(), ["2", "1", "3"]);
        assert!(!m.toggle(&a));
        assert!(!m.contains(&a) && m.contains(&b));
        assert_eq!(m.len(), 2);
        m.clear();
        assert!(m.is_empty() && m.entries().is_empty());
    }

    #[test]
    fn only_library_items_can_be_marked() {
        assert!(markable(&item("movie", "1")));
        assert!(markable(&item("episode", "2")));
        assert!(!markable(&crate::item::test_item("movie", "no key")));
        assert!(!markable(&item("playlist", "3")));
        assert!(!markable(&crate::tui::rows::heading("Section")));
    }

    #[test]
    fn playlists_need_one_kind_of_media() {
        let movie = item("movie", "1");
        let episode = item("episode", "2");
        let track = item("track", "3");
        assert_eq!(playlist_kind(&[movie.clone(), episode]), Ok("video"));
        assert_eq!(playlist_kind(std::slice::from_ref(&track)), Ok("audio"));
        assert!(playlist_kind(&[movie, track]).is_err());
        assert!(playlist_kind(&[item("photo", "4")]).is_err());
        assert!(playlist_kind(&[]).is_err());
    }

    #[test]
    fn collections_need_one_type() {
        assert_eq!(collection_kind(&[item("movie", "1"), item("movie", "2")]), Ok("movie"));
        assert!(collection_kind(&[item("movie", "1"), item("show", "2")]).is_err());
        assert!(collection_kind(&[item("episode", "1")]).is_err());
    }

    #[test]
    fn batch_outcomes_name_the_failures() {
        assert_eq!(outcome("3 titles updated", 3, &[]), Ok(Some("3 titles updated".into())));
        assert_eq!(outcome("x", 3, &["A: boom".into()]), Err("2 of 3 done. A: boom".into()));
        let many: Vec<String> = ["A", "B", "C", "D"].iter().map(|s| s.to_string()).collect();
        assert_eq!(outcome("x", 4, &many), Err("0 of 4 done. A; B (+2 more)".into()));
    }
}
