//! Sparse page cache for a library listing: a jump to Z never loads A–Y, and old
//! results survive refresh failures. A change on the server marks cached pages
//! stale instead of dropping them: the pages in view refetch at once, the others
//! when they scroll into view, and everything stays on screen meanwhile.

use std::collections::{HashMap, HashSet};

use patina_plex::Metadata;

/// Items per request.
pub const PAGE: usize = 120;
/// Pages kept in memory, nearest the viewport first.
const KEEP: usize = 12;

#[derive(Default)]
pub struct Pages {
    /// What the pages list (the query); a different key starts over.
    pub key: String,
    pub revision: u64,
    pub total: Option<usize>,
    pub error: Option<String>,
    generation: u64,
    pages: HashMap<usize, Vec<Metadata>>,
    pending: HashSet<usize>,
    failed: HashSet<usize>,
    stale: HashSet<usize>,
    visible: (usize, usize),
}

/// A page to fetch: its offset, and the generation the answer must match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fetch {
    pub offset: usize,
    pub generation: u64,
}

impl Pages {
    /// Start over for another query.
    pub fn reset(&mut self, key: String, revision: u64) {
        *self = Pages { key, revision, generation: self.generation + 1, ..Default::default() };
    }

    pub fn loading(&self) -> bool {
        !self.pending.is_empty()
    }

    pub fn at(&self, index: usize) -> Option<&Metadata> {
        self.pages.get(&(index / PAGE * PAGE))?.get(index % PAGE)
    }

    /// Pages covering items `start..end` that need fetching; marks them pending.
    pub fn ensure(&mut self, start: usize, end: usize) -> Vec<Fetch> {
        self.visible = (start, end);
        let mut out = Vec::new();
        let mut offset = start / PAGE * PAGE;
        while offset < end.max(start + 1) {
            let needed = !self.pages.contains_key(&offset) || self.stale.contains(&offset);
            if needed && !self.pending.contains(&offset) && !self.failed.contains(&offset) {
                self.pending.insert(offset);
                out.push(Fetch { offset, generation: self.generation });
            }
            offset += PAGE;
        }
        out
    }

    /// Everything cached may be out of date: refetch what's in view now and the rest lazily.
    pub fn refresh(&mut self) -> Vec<Fetch> {
        self.generation += 1;
        self.pending.clear();
        self.failed.clear();
        self.error = None;
        self.stale.extend(self.pages.keys().copied());
        let (start, end) = self.visible;
        self.ensure(start, end.max(PAGE))
    }

    /// A page arrived (`total` from the container, if it said).
    pub fn loaded(&mut self, fetch: Fetch, items: Vec<Metadata>, total: Option<usize>) {
        if fetch.generation != self.generation {
            return;
        }
        self.pending.remove(&fetch.offset);
        self.total = Some(total.unwrap_or(fetch.offset + items.len()));
        self.pages.insert(fetch.offset, items);
        self.stale.remove(&fetch.offset);
        // Bound memory: keep the pages nearest the viewport.
        let visible = self.visible.0;
        let mut offsets: Vec<usize> = self.pages.keys().copied().collect();
        offsets.sort_by_key(|o| o.abs_diff(visible));
        for o in offsets.into_iter().skip(KEEP) {
            self.pages.remove(&o);
            self.stale.remove(&o);
        }
    }

    pub fn failed(&mut self, fetch: Fetch, error: String) {
        if fetch.generation != self.generation {
            return;
        }
        self.pending.remove(&fetch.offset);
        self.failed.insert(fetch.offset);
        self.error = Some(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(n: usize) -> Vec<Metadata> {
        (0..n).map(|i| Metadata { rating_key: Some(i.to_string()), ..Default::default() }).collect()
    }

    #[test]
    fn fetches_only_the_pages_in_view() {
        let mut p = Pages::default();
        p.reset("q".into(), 0);
        let f = p.ensure(1000, 1100);
        assert_eq!(f.iter().map(|f| f.offset).collect::<Vec<_>>(), vec![960, 1080]);
        // Already pending: nothing new.
        assert!(p.ensure(1000, 1100).is_empty());
        p.loaded(f[0], items(PAGE), Some(5000));
        assert_eq!(p.total, Some(5000));
        assert_eq!(p.at(960).and_then(|m| m.rating_key.as_deref()), Some("0"));
        assert!(p.at(0).is_none());
    }

    #[test]
    fn stale_answers_are_dropped() {
        let mut p = Pages::default();
        p.reset("a".into(), 0);
        let f = p.ensure(0, 10);
        p.reset("b".into(), 0);
        p.loaded(f[0], items(3), None);
        assert_eq!(p.total, None);
    }

    #[test]
    fn refresh_keeps_old_pages_on_screen() {
        let mut p = Pages::default();
        p.reset("q".into(), 0);
        let f = p.ensure(0, 10);
        p.loaded(f[0], items(5), None);
        assert_eq!(p.total, Some(5));
        let again = p.refresh();
        assert_eq!(again.len(), 1);
        assert!(p.at(0).is_some(), "stale page still shows");
        p.failed(again[0], "boom".into());
        assert_eq!(p.error.as_deref(), Some("boom"));
        assert!(p.at(0).is_some());
        // A failed page isn't retried until the next refresh.
        assert!(p.ensure(0, 10).is_empty());
    }

    #[test]
    fn memory_is_bounded() {
        let mut p = Pages::default();
        p.reset("q".into(), 0);
        for i in 0..20 {
            let f = p.ensure(i * PAGE, i * PAGE + 1);
            p.loaded(f[0], items(PAGE), Some(20 * PAGE));
        }
        assert!(p.at(19 * PAGE).is_some());
        assert!(p.at(0).is_none());
    }
}
