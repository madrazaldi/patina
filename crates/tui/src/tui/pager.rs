//! A big listing seen through a window.
//!
//! A library of tens of thousands of titles is not worth fetching whole just
//! to scroll through its first screens. A [`Pager`] knows how many entries
//! there are and keeps only the pages near the cursor: moving asks for the
//! pages that come into range, pages far from the cursor are dropped (the
//! cache never holds more than [`MAX_PAGES`]), and requests for pages that
//! are no longer near are aborted.
//!
//! Filtering, hiding watched titles and local sorting need every entry, so a
//! column that wants them leaves the pager and loads the whole listing
//! instead (see `App::go_full`); paging only serves the plain, server-ordered
//! view.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;

use super::task::TaskSlot;
use crate::item::Item;

/// Entries requested per page.
pub const PAGE: usize = 200;
/// Pages kept in memory.
pub const MAX_PAGES: usize = 8;

pub struct Pager {
    /// Entries the server reports in the listing.
    pub total: usize,
    size: usize,
    /// Bumped by [`invalidate`](Pager::invalidate); pages from earlier epochs stay
    /// visible until their replacement arrives but count as missing.
    epoch: u64,
    pages: BTreeMap<usize, (u64, Vec<Item>)>,
    inflight: HashMap<usize, TaskSlot>,
    failed: HashSet<usize>,
}

impl Pager {
    /// A pager over `total` entries read `size` at a time.
    pub fn new(total: usize, size: usize) -> Self {
        Self { total, size: size.max(1), epoch: 0, pages: BTreeMap::new(), inflight: HashMap::new(), failed: HashSet::new() }
    }

    /// Entries per page; requests for page `p` start at `p * size()`.
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn page_count(&self) -> usize {
        self.total.div_ceil(self.size)
    }

    /// The entry at `pos`, if its page is loaded (possibly out of date).
    pub fn item(&self, pos: usize) -> Option<&Item> {
        self.pages.get(&(pos / self.size))?.1.get(pos % self.size)
    }

    pub fn loaded_pages(&self) -> usize {
        self.pages.len()
    }

    pub fn loading(&self) -> usize {
        self.inflight.len()
    }

    pub fn failed(&self) -> bool {
        !self.failed.is_empty()
    }

    /// Every loaded entry, for updates in place.
    pub fn items_mut(&mut self) -> impl Iterator<Item = &mut Item> {
        self.pages.values_mut().flat_map(|(_, items)| items.iter_mut())
    }

    pub fn contains(&self, mut f: impl FnMut(&Item) -> bool) -> bool {
        self.pages.values().any(|(_, items)| items.iter().any(&mut f))
    }

    /// The position of the first loaded entry for which `f` holds.
    pub fn position(&self, mut f: impl FnMut(&Item) -> bool) -> Option<usize> {
        self.pages.iter().find_map(|(page, (_, items))| items.iter().position(&mut f).map(|i| page * self.size + i))
    }

    /// Loaded entries from `pos` on, up to the first gap.
    pub fn loaded_from(&self, pos: usize) -> impl Iterator<Item = &Item> {
        (pos..self.total).map_while(|p| self.item(p))
    }

    fn fresh(&self, page: usize) -> bool {
        self.pages.get(&page).is_some_and(|(epoch, _)| *epoch == self.epoch)
    }

    /// The pages worth having with the cursor at `cursor` and `height` rows
    /// on screen: the screen, one more screen behind and half a page ahead.
    pub fn wanted(&self, cursor: usize, height: usize) -> Vec<usize> {
        if self.total == 0 {
            return Vec::new();
        }
        let last = self.total - 1;
        let cursor = cursor.min(last);
        let lo = cursor.saturating_sub(height.max(1));
        let hi = (cursor + height.max(1) + self.size / 2).min(last);
        let at = cursor / self.size;
        let mut pages: Vec<usize> = (lo / self.size..=hi / self.size).collect();
        pages.sort_by_key(|p| p.abs_diff(at));
        pages
    }

    /// Pages in `wanted` that need a request, nearest first.
    pub fn missing(&self, wanted: &[usize]) -> Vec<usize> {
        wanted.iter().copied().filter(|p| !self.fresh(*p) && !self.inflight.contains_key(p) && !self.failed.contains(p)).collect()
    }

    /// Start `fut` as the request for `page`.
    pub fn fetch<F>(&mut self, page: usize, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.inflight.entry(page).or_default().spawn(fut);
    }

    /// Abort requests for pages outside `wanted` and drop pages beyond the
    /// cache bound, farthest from the cursor first.
    pub fn trim(&mut self, wanted: &[usize], cursor: usize) {
        self.inflight.retain(|p, _| wanted.contains(p));
        let at = cursor.min(self.total.saturating_sub(1)) / self.size;
        if self.pages.len() > MAX_PAGES {
            let mut by_distance: Vec<usize> = self.pages.keys().copied().collect();
            by_distance.sort_by_key(|p| p.abs_diff(at));
            for page in by_distance.into_iter().skip(MAX_PAGES) {
                self.pages.remove(&page);
            }
        }
    }

    /// A requested page arrived.
    pub fn insert(&mut self, page: usize, items: Vec<Item>, total: usize) {
        self.inflight.remove(&page);
        self.failed.remove(&page);
        self.total = total;
        self.pages.insert(page, (self.epoch, items));
    }

    pub fn fail(&mut self, page: usize) {
        self.inflight.remove(&page);
        self.failed.insert(page);
    }

    /// The listing may have changed: pages are refetched when next wanted,
    /// requests under way are abandoned, and failures may be tried again.
    pub fn invalidate(&mut self) {
        self.epoch += 1;
        self.inflight.clear();
        self.failed.clear();
    }

    /// Whether every loaded page is out of date.
    pub fn stale(&self) -> bool {
        self.pages.values().all(|(epoch, _)| *epoch != self.epoch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    fn page_of(page: usize, size: usize, total: usize) -> Vec<Item> {
        (page * size..total.min((page + 1) * size)).map(|i| crate::item::test_item("movie", &format!("m{i}"))).collect()
    }

    fn title(p: &Pager, pos: usize) -> Option<String> {
        p.item(pos).map(|i| i.title().to_string())
    }

    #[test]
    fn positions_map_to_pages() {
        let mut p = Pager::new(450, 200);
        assert_eq!(p.page_count(), 3);
        assert_eq!(title(&p, 0), None);
        p.insert(1, page_of(1, 200, 450), 450);
        assert_eq!(title(&p, 199), None);
        assert_eq!(title(&p, 200).as_deref(), Some("m200"));
        assert_eq!(title(&p, 399).as_deref(), Some("m399"));
        p.insert(2, page_of(2, 200, 450), 450);
        assert_eq!(title(&p, 449).as_deref(), Some("m449"));
        assert_eq!(title(&p, 450), None);
        assert_eq!(p.loaded_from(398).count(), 2 + 50);
        assert_eq!(p.loaded_from(100).count(), 0, "a gap ends the run");
    }

    #[test]
    fn wanted_pages_follow_the_cursor_nearest_first() {
        let mut p = Pager::new(10_000, 200);
        assert_eq!(p.wanted(0, 40), vec![0]);
        // Half a page from the end of page 0 the next one is wanted too.
        assert_eq!(p.wanted(100, 40), vec![0, 1]);
        assert_eq!(p.wanted(250, 40), vec![1]);
        assert_eq!(p.wanted(300, 40), vec![1, 2]);
        // Just into a page, the screen behind it is wanted too, after the page the cursor is on.
        assert_eq!(p.wanted(210, 40), vec![1, 0]);
        // Jumping to the end wants only the end.
        assert_eq!(p.wanted(9_999, 40), vec![49]);
        assert_eq!(Pager::new(0, 200).wanted(0, 40), Vec::<usize>::new());
        // Only pages that are neither fresh nor on their way are missing.
        p.insert(0, page_of(0, 200, 10_000), 10_000);
        assert_eq!(p.missing(&p.wanted(100, 40)), vec![1]);
        p.invalidate();
        assert_eq!(p.missing(&p.wanted(100, 40)), vec![0, 1], "stale pages are refetched");
        assert!(p.item(5).is_some(), "but stay visible meanwhile");
        assert!(p.stale());
    }

    #[test]
    fn the_cache_stays_bounded_around_the_cursor() {
        let mut p = Pager::new(100_000, 200);
        for page in 0..30 {
            p.insert(page, page_of(page, 200, 100_000), 100_000);
            p.trim(&[], page * 200);
            assert!(p.loaded_pages() <= MAX_PAGES);
        }
        // The pages kept are the ones nearest the last cursor (page 29).
        assert!(p.item(29 * 200).is_some() && p.item(22 * 200).is_some());
        assert!(p.item(21 * 200).is_none() && p.item(0).is_none());
        // Scrolling far away drops the old neighbourhood as the new one arrives.
        for page in 70..78 {
            p.insert(page, page_of(page, 200, 100_000), 100_000);
        }
        p.trim(&[], 70 * 200);
        assert_eq!(p.loaded_pages(), MAX_PAGES);
        assert!(p.item(70 * 200).is_some() && p.item(29 * 200).is_none());
    }

    #[test]
    fn failures_are_not_retried_until_invalidated() {
        let mut p = Pager::new(1000, 200);
        p.fail(0);
        assert!(p.failed());
        assert!(p.missing(&[0, 1]).contains(&1) && !p.missing(&[0, 1]).contains(&0));
        p.invalidate();
        assert_eq!(p.missing(&[0, 1]), vec![0, 1]);
    }

    #[test]
    fn total_follows_the_server() {
        let mut p = Pager::new(1000, 200);
        p.insert(0, page_of(0, 200, 950), 950);
        assert_eq!((p.total, p.page_count()), (950, 5));
        assert_eq!(p.wanted(10_000, 10), vec![4], "a cursor past the end is clamped");
    }

    struct Lived(Arc<AtomicBool>);
    impl Drop for Lived {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    fn parked() -> (impl Future<Output = ()> + Send + 'static, Arc<AtomicBool>) {
        let dropped = Arc::new(AtomicBool::new(false));
        let guard = Lived(dropped.clone());
        (
            async move {
                let _guard = guard;
                tokio::time::sleep(Duration::from_secs(3600)).await;
            },
            dropped,
        )
    }

    #[tokio::test]
    async fn requests_for_pages_that_are_no_longer_near_are_aborted() {
        let mut p = Pager::new(10_000, 200);
        let (near, near_dropped) = parked();
        let (far, far_dropped) = parked();
        p.fetch(0, near);
        p.fetch(30, far);
        assert_eq!(p.loading(), 2);
        // The cursor wants page 0 only: the request for page 30 is obsolete.
        p.trim(&[0], 0);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(far_dropped.load(Ordering::SeqCst) && !near_dropped.load(Ordering::SeqCst));
        assert_eq!(p.loading(), 1);
        // Reloading the listing abandons whatever is under way, and so does dropping the pager.
        p.invalidate();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(near_dropped.load(Ordering::SeqCst));
        let (last, last_dropped) = parked();
        p.fetch(2, last);
        drop(p);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(last_dropped.load(Ordering::SeqCst));
    }
}
