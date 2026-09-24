//! What a column lists, and how it loads.
//!
//! A new screen is a [`Source`] variant plus its arm in [`fetch`]; a root
//! entry for it is one line in `App::root_items` (via [`crate::item::Item::nav`]).

use patina_core::AppState;
use patina_core::ops::{library, lists};
use patina_plex::{Hub, Metadata, MetadataOptions, Params, Tag};

use crate::fetch::watchlist_items;
use crate::item::Item;

const PAGE_SIZE: usize = 500;

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// The root navigation column.
    Root,
    /// Items handed over when the column opens (search results, hub contents).
    Static,
    /// A server listing; big ones load in pages.
    Path(String),
    /// A library section's items with server-side type, sort and filters.
    Library(LibraryView),
    /// A section's recommended hubs.
    SectionHubs(String),
    /// A section's folder tree at `key` (its top level when `None`).
    Folder { section: String, key: Option<String> },
    /// An item's related hubs (incl. sonically similar), people and tags.
    Related { rating_key: String, kind: String },
    /// The server's home hubs (Continue Watching, Recently Added, ...).
    Home,
    /// A hub's items, paged from its key.
    Hub(String),
    Watchlist,
    Sessions,
    History,
    /// Opens the search overlay rather than a column.
    Search,
}

impl Source {
    /// The server path listed, used for paging, refreshes and smart playlists.
    pub fn path(&self) -> Option<String> {
        match self {
            Source::Path(p) => Some(p.clone()),
            Source::Library(v) => Some(v.path()),
            _ => None,
        }
    }

    /// Whether there is anything to (re)load.
    pub fn loads(&self) -> bool {
        !matches!(self, Source::Root | Source::Static | Source::Search)
    }
}

/// Media types a section can list, as (Plex type id, label); the first is the default.
pub fn section_types(section_kind: &str) -> &'static [(u32, &'static str)] {
    match section_kind {
        "show" => &[(2, "Shows"), (3, "Seasons"), (4, "Episodes")],
        "artist" => &[(8, "Artists"), (9, "Albums"), (10, "Tracks")],
        _ => &[],
    }
}

/// One active server filter: `filter=value`, shown as `title: label`.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub filter: String,
    pub title: String,
    pub value: String,
    pub label: String,
}

/// A library listing as the GUI's Library page builds it.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryView {
    pub section: String,
    /// The section's type (`movie`, `show`, `artist`, ...).
    pub section_kind: String,
    /// Plex media type listed; `None` lists the section's default.
    pub kind: Option<u32>,
    pub sort: String,
    pub sort_title: String,
    pub desc: bool,
    /// The server's descending key for `sort`, when it has one.
    pub desc_key: Option<String>,
    /// Active filters; booleans have the value `1`.
    pub filters: Vec<Filter>,
}

impl LibraryView {
    pub fn new(section: &str, section_kind: &str) -> Self {
        Self {
            section: section.into(),
            section_kind: section_kind.into(),
            kind: section_types(section_kind).first().map(|t| t.0),
            sort: "titleSort".into(),
            sort_title: "Title".into(),
            desc: false,
            desc_key: None,
            filters: Vec::new(),
        }
    }

    pub fn path(&self) -> String {
        let sort = match (&self.desc_key, self.desc) {
            (Some(k), true) => k.clone(),
            (None, true) => format!("{}:desc", self.sort),
            _ => self.sort.clone(),
        };
        let mut p = Params::new().opt("type", self.kind).set("sort", sort);
        for f in &self.filters {
            p.push(f.filter.clone(), &f.value);
        }
        format!("/library/sections/{}/all?{}", self.section, p.to_query_string())
    }

    /// Whether the section can be watched (so offers the Unwatched filter).
    pub fn watchable(&self) -> bool {
        !matches!(self.section_kind.as_str(), "artist" | "photo")
    }

    /// The label of the listed type, when the section has several.
    pub fn type_label(&self) -> Option<&'static str> {
        section_types(&self.section_kind).iter().find(|t| Some(t.0) == self.kind).map(|t| t.1)
    }

    /// Switch the listed type; filters belong to a type, so they are cleared.
    pub fn set_kind(&mut self, kind: u32) {
        self.kind = Some(kind);
        self.filters.clear();
    }

    pub fn is_on(&self, filter: &str, value: &str) -> bool {
        self.filters.iter().any(|f| f.filter == filter && f.value == value)
    }

    /// Add or remove `filter=value`.
    pub fn toggle(&mut self, f: Filter) {
        match self.filters.iter().position(|x| x.filter == f.filter && x.value == f.value) {
            Some(i) => {
                self.filters.remove(i);
            }
            None => self.filters.push(f),
        }
    }

    /// Titles are listed A–Z by the server, so the letter index applies.
    pub fn alphabetical(&self) -> bool {
        self.sort.starts_with("titleSort") && !self.desc && self.filters.is_empty()
    }

    /// Seasons, episodes, albums and tracks are listed out of their parent.
    pub fn leaves(&self) -> bool {
        matches!(self.kind, Some(3 | 4 | 9 | 10))
    }

    /// `Title ↑ · Genre: Drama · HDR`
    pub fn describe(&self) -> String {
        let mut parts = vec![format!("{} {}", self.sort_title, if self.desc { "↓" } else { "↑" })];
        for f in &self.filters {
            parts.push(if f.value == "1" && f.label.is_empty() { f.title.clone() } else { format!("{}: {}", f.title, f.label) });
        }
        parts.join(" · ")
    }
}

/// The row index of each letter in an A–Z listing, from the server's
/// `firstCharacter` counts.
pub fn letter_starts(chars: &[patina_plex::Directory]) -> Vec<(String, usize, usize)> {
    let mut out: Vec<(String, usize, usize)> = Vec::new();
    let mut n = 0;
    for c in chars {
        let letter = c.title.clone().or_else(|| c.key.clone()).unwrap_or_default().to_uppercase();
        let size = c.size.unwrap_or(0).max(0) as usize;
        if !out.iter().any(|(l, _, _)| *l == letter) {
            out.push((letter, n, size));
        }
        n += size;
    }
    out
}

/// Where a synthetic entry (nav entry, hub, person, tag) leads.
#[derive(Debug, Clone)]
pub struct Link {
    pub source: Source,
    /// Shown until the source loads, or for good when it is [`Source::Static`].
    pub items: Vec<Item>,
}

pub fn is_paged(path: &str) -> bool {
    (path.starts_with("/library/sections/") && path != "/library/sections")
        || path.starts_with("/library/collections/")
        || (path.starts_with("/playlists/") && path.ends_with("/items"))
}

/// Load a source. Paged listings report each page through `page(items,
/// append, total)` when `progressive`, before the full result is returned.
pub async fn fetch(core: &AppState, source: &Source, progressive: bool, page: &mut (dyn FnMut(Vec<Item>, bool, Option<usize>) + Send)) -> anyhow::Result<Vec<Item>> {
    let server = core.server()?;
    match source {
        Source::Root | Source::Static | Source::Search => Ok(Vec::new()),
        Source::Home => Ok(hubs(server.home_hubs(20).await?)),
        Source::Hub(key) => paged(&server, key, progressive, page).await,
        Source::Sessions => Ok(server.sessions().await?.into_iter().map(Item::from).collect()),
        Source::History => Ok(lists::history(core, 0).await?.items.into_iter().map(Item::from).collect()),
        Source::Watchlist => watchlist_items(core, &server).await,
        Source::Library(v) => paged(&server, &v.path(), progressive, page).await,
        Source::SectionHubs(id) => Ok(hubs(server.section_hubs(id, 18).await?)),
        Source::Folder { section, key } => {
            let view = library::section_folders(core, section, key.as_deref()).await?;
            let folders = view.directories.into_iter().map(|d| {
                let source = Source::Folder { section: section.clone(), key: d.key.clone() };
                let mut it = Item::directory(d);
                it.link = Some(Box::new(Link { source, items: Vec::new() }));
                it
            });
            Ok(folders.chain(view.items.into_iter().map(Item::from)).collect())
        }
        Source::Related { rating_key, kind } => {
            let opts = MetadataOptions { related: true, ..Default::default() };
            let meta = server.metadata(rating_key, opts).await?.ok_or_else(|| anyhow::anyhow!("item not found"))?;
            let extra = library::item_hubs(core, rating_key, kind).await.unwrap_or_default();
            Ok(related(meta, extra))
        }
        Source::Path(p) if is_paged(p) => paged(&server, p, progressive, page).await,
        Source::Path(p) => Ok(Item::listing(server.fetch(p, &Params::new()).await?)),
    }
}

/// Hub entries, skipping empty hubs; in-progress hubs come first, as on the GUI's home.
pub fn hubs(list: Vec<patina_plex::Hub>) -> Vec<Item> {
    let continuing = |h: &patina_plex::Hub| {
        let id = h.hub_identifier.as_deref().unwrap_or("").to_ascii_lowercase();
        ["continue", "ondeck", "inprogress"].iter().any(|s| id.contains(s))
    };
    let (mut first, rest): (Vec<_>, Vec<_>) = list.into_iter().filter(|h| !h.is_empty()).partition(continuing);
    first.extend(rest);
    first.into_iter().map(Item::hub).collect()
}

/// Search results as one entry per result type (the GUI's grouped search);
/// people and tags become entries listing their titles.
pub fn search_groups(list: Vec<Hub>) -> Vec<Item> {
    const PEOPLE: [&str; 6] = ["actor", "director", "writer", "producer", "person", "tag"];
    list.into_iter()
        .filter(|h| !h.is_empty())
        .map(|h| {
            let title = h.title.clone().unwrap_or_default();
            let mut items: Vec<Item> = h.metadata.into_iter().map(Item::from).collect();
            items.extend(h.directory.into_iter().filter_map(|d| {
                let name = d.title.clone().or(d.tag.clone())?;
                let kind = if PEOPLE.contains(&d.kind.as_deref().unwrap_or("")) { "person" } else { "tag" };
                let mut it = Item::link(&name, kind, Source::Path(d.key.clone()?), Vec::new());
                it.summary = d.count.map(|n| format!("{n} titles")).or(d.kind.clone());
                it.thumb = d.thumb;
                Some(it)
            }));
            let mut group = Item::link(&title, "hub", Source::Static, items);
            group.child_count = Some(group.link.as_ref().map_or(0, |l| l.items.len()) as i64);
            group.key = h.hub_identifier;
            group
        })
        .collect()
}

/// The item's related hubs plus extra hubs it doesn't already have (as the
/// GUI's detail page dedupes them), then its people and tags.
fn related(mut meta: Metadata, extra: Vec<Hub>) -> Vec<Item> {
    let own: Vec<Hub> = meta.related.take().map(|r| r.hub).unwrap_or_default().into_iter().filter(|h| !h.metadata.is_empty()).collect();
    let seen = |h: &Hub| {
        own.iter().any(|r| {
            (h.hub_identifier.is_some() && r.hub_identifier == h.hub_identifier)
                || h.title.as_deref().is_some_and(|t| r.title.as_deref().is_some_and(|rt| rt.eq_ignore_ascii_case(t)))
        })
    };
    let extra: Vec<Hub> = extra.into_iter().filter(|h| !h.metadata.is_empty() && !seen(h)).collect();
    let mut out: Vec<Item> = own.into_iter().chain(extra).map(Item::hub).collect();
    let Some(section) = meta.library_section_id else { return out };
    let music = matches!(meta.kind.as_deref(), Some("album" | "artist" | "track"));
    let entry = |kind: &str, t: &Tag, note: String| {
        let filter = t.filter.as_deref()?;
        let source = Source::Path(format!("/library/sections/{section}/all?{filter}"));
        let mut it = Item::link(t.name(), kind, source, Vec::new());
        it.summary = Some(note);
        it.thumb = t.thumb.clone();
        Some(it)
    };
    let cast = if music { "Featuring" } else { "Cast" };
    out.extend(meta.roles.iter().take(40).filter_map(|t| entry("person", t, t.role.clone().filter(|r| !r.is_empty()).unwrap_or_else(|| cast.into()))));
    for (label, list) in [("Director", &meta.directors), ("Writer", &meta.writers)] {
        out.extend(list.iter().take(8).filter_map(|t| entry("person", t, label.into())));
    }
    let tags = [
        ("Genre", &meta.genres),
        ("Collection", &meta.collections),
        ("Mood", &meta.moods),
        ("Style", &meta.styles),
        ("Country", &meta.countries),
        ("Label", &meta.labels),
    ];
    for (label, list) in tags {
        out.extend(list.iter().take(8).filter_map(|t| entry("tag", t, label.into())));
    }
    out
}

async fn paged(
    server: &patina_plex::PlexServer,
    path: &str,
    progressive: bool,
    page: &mut (dyn FnMut(Vec<Item>, bool, Option<usize>) + Send),
) -> anyhow::Result<Vec<Item>> {
    let mut all: Vec<Item> = Vec::new();
    let mut start = 0;
    loop {
        let params = Params::new()
            .set("X-Plex-Container-Start", start)
            .set("X-Plex-Container-Size", PAGE_SIZE)
            .set("includeGuids", 1);
        let mc = server.fetch(path, &params).await?;
        let total = mc.total_size.or(mc.size).map(|n| n.max(0) as usize);
        let items = Item::listing(mc);
        let n = items.len();
        start += n;
        let finished = n == 0 || total.map_or(n < PAGE_SIZE, |t| start >= t);
        if progressive && !finished {
            page(items.clone(), start > n, total);
        }
        all.extend(items);
        if finished {
            return Ok(all);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::Kind;

    fn hub(id: &str, n: usize, more: bool) -> Hub {
        Hub {
            hub_identifier: Some(id.into()),
            title: Some(id.into()),
            key: Some(format!("/hubs/{id}")),
            more,
            metadata: (0..n).map(|i| Metadata { rating_key: Some(i.to_string()), kind: Some("movie".into()), ..Default::default() }).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn hubs_put_in_progress_first_and_skip_empty() {
        let items = hubs(vec![hub("movie.recentlyadded", 3, true), hub("empty", 0, false), hub("home.continue", 2, false)]);
        let titles: Vec<_> = items.iter().map(|i| i.title().to_string()).collect();
        assert_eq!(titles, ["home.continue", "movie.recentlyadded"]);
        assert_eq!(items[0].kind(), Kind::Hub);
        assert!(items[0].is_container());
        assert_eq!(items[0].child_count, Some(2));
        let link = items[0].link.as_ref().unwrap();
        assert_eq!(link.source, Source::Static);
        assert_eq!(link.items.len(), 2);
        assert_eq!(items[1].link_source(), Some(&Source::Hub("/hubs/movie.recentlyadded".into())));
    }

    fn filter(filter: &str, value: &str, label: &str) -> Filter {
        Filter { filter: filter.into(), title: filter.into(), value: value.into(), label: label.into() }
    }

    #[test]
    fn library_paths() {
        let mut v = LibraryView::new("2", "show");
        assert_eq!(v.kind, Some(2));
        assert_eq!(v.path(), "/library/sections/2/all?type=2&sort=titleSort");
        assert!(v.alphabetical());
        v.toggle(filter("genre", "12", "Drama"));
        v.toggle(filter("genre", "13", "Comedy"));
        v.toggle(filter("hdr", "1", ""));
        v.desc = true;
        assert_eq!(v.path(), "/library/sections/2/all?type=2&sort=titleSort%3Adesc&genre=12&genre=13&hdr=1");
        assert!(v.is_on("genre", "13"));
        v.toggle(filter("genre", "12", "Drama"));
        assert!(!v.is_on("genre", "12"));
        v.desc_key = Some("titleSort:reverse".into());
        assert!(v.path().contains("sort=titleSort%3Areverse"));
        assert_eq!(v.describe(), "Title ↓ · genre: Comedy · hdr");
        assert!(!v.alphabetical());
        v.set_kind(4);
        assert!(v.filters.is_empty() && v.leaves());
        assert_eq!(v.type_label(), Some("Episodes"));
        let m = LibraryView::new("1", "movie");
        assert_eq!(m.kind, None);
        assert_eq!(m.path(), "/library/sections/1/all?sort=titleSort");
        assert!(!LibraryView::new("3", "artist").watchable());
    }

    #[test]
    fn letter_indices() {
        let d = |t: &str, n: i64| patina_plex::Directory { title: Some(t.into()), size: Some(n), ..Default::default() };
        let starts = letter_starts(&[d("#", 2), d("A", 3), d("b", 1), d("C", 4)]);
        assert_eq!(starts, vec![("#".into(), 0, 2), ("A".into(), 2, 3), ("B".into(), 5, 1), ("C".into(), 6, 4)]);
    }

    #[test]
    fn related_dedupes_hubs_and_links_people_and_tags() {
        let tag = |name: &str, filter: &str| Tag { tag: Some(name.into()), filter: Some(filter.into()), ..Default::default() };
        let mut meta = Metadata { library_section_id: Some(1), kind: Some("movie".into()), ..Default::default() };
        meta.related = Some(patina_plex::Nested { hub: vec![hub("similar", 3, false)], ..Default::default() });
        meta.roles = vec![Tag { role: Some("Iris".into()), ..tag("Mara", "actor=300") }, Tag { filter: None, ..tag("Nobody", "") }];
        meta.genres = vec![tag("Drama", "genre=10")];
        let mut dup = hub("other", 2, false);
        dup.title = Some("SIMILAR".into());
        let items = related(meta, vec![dup, hub("director", 2, false), hub("empty", 0, false)]);
        let names: Vec<_> = items.iter().map(|i| (i.kind(), i.title().to_string())).collect();
        assert_eq!(
            names,
            [(Kind::Hub, "similar".into()), (Kind::Hub, "director".into()), (Kind::Person, "Mara".into()), (Kind::Tag, "Drama".into())]
        );
        assert_eq!(items[2].summary.as_deref(), Some("Iris"));
        assert_eq!(items[3].link_source(), Some(&Source::Path("/library/sections/1/all?genre=10".into())));
    }

    #[test]
    fn search_groups_link_people() {
        let people = Hub {
            title: Some("Actors".into()),
            hub_identifier: Some("actor".into()),
            directory: vec![patina_plex::Directory {
                key: Some("/library/sections/1/all?actor=300".into()),
                tag: Some("Mara Okafor".into()),
                kind: Some("actor".into()),
                count: Some(20),
                ..Default::default()
            }],
            ..Default::default()
        };
        let groups = search_groups(vec![hub("movie", 2, false), hub("none", 0, false), people]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].child_count, Some(2));
        let mara = &groups[1].link.as_ref().unwrap().items[0];
        assert_eq!((mara.kind(), mara.title()), (Kind::Person, "Mara Okafor"));
        assert_eq!(mara.summary.as_deref(), Some("20 titles"));
        assert_eq!(mara.link_source(), Some(&Source::Path("/library/sections/1/all?actor=300".into())));
    }

    #[test]
    fn paged_paths() {
        assert!(is_paged("/library/sections/1/all"));
        assert!(is_paged("/library/sections/1/all?genre=3"));
        assert!(!is_paged("/library/sections"));
        assert!(is_paged("/playlists/9/items"));
        assert!(!is_paged("/playlists"));
        assert!(!is_paged("/library/metadata/4/children"));
    }
}
