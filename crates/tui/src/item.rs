//! The terminal client's view of a Plex entry.
//!
//! Plex's API is hierarchical: every entry carries a `key` that points at its
//! children, so one [`Item`] models libraries, movies, shows, episodes,
//! tracks, playlists, sessions, history rows and browse-by folders. It wraps
//! a [`Metadata`] (and derefs to it) plus where the entry came from.

use std::ops::{Deref, DerefMut};

use patina_plex::{Directory, Hub, MediaContainer, Metadata, Part};
use serde_json::Value;

use crate::tui::rows::Row;
use crate::tui::source::{Link, Source};

/// Where an item came from. Plex uses `type: "movie"` both for a movie and a
/// movie *library section*, so origin disambiguates the two.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Origin {
    #[default]
    Metadata,
    Directory,
    Section,
    /// A synthetic navigation entry (Continue Watching, Playlists, ...).
    Nav,
    /// A plex.tv Discover entry (watchlist) that isn't in any local library.
    Discover,
    /// A synthetic entry leading somewhere through its `link` (a hub, a person, a tag).
    Link,
    /// A row of a tool screen (downloads, guide, server tools, settings).
    Row,
}

/// Coarse classification used for presentation and behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Nav,
    Section,
    Movie,
    Show,
    Season,
    Episode,
    Artist,
    Album,
    Track,
    Photo,
    PhotoAlbum,
    Clip,
    Collection,
    Playlist,
    Folder,
    /// A Discover (watchlist) title with no copy on this server.
    Remote,
    /// A group of items (a server hub, search results of one type).
    Hub,
    /// A person (cast or crew) whose titles can be listed.
    Person,
    /// A genre, label, collection or other tag whose titles can be listed.
    Tag,
    /// A row of a tool screen; see [`Row`].
    Row,
    Other,
}

#[derive(Debug, Clone, Default)]
pub struct Item {
    pub origin: Origin,
    pub meta: Metadata,
    /// The directory entry the item was made from (sections, browse-by folders).
    pub dir: Option<Directory>,
    /// Where a synthetic entry leads, instead of its `key`.
    pub link: Option<Box<Link>>,
    pub row: Option<Box<Row>>,
}

impl Deref for Item {
    type Target = Metadata;
    fn deref(&self) -> &Metadata {
        &self.meta
    }
}

impl DerefMut for Item {
    fn deref_mut(&mut self) -> &mut Metadata {
        &mut self.meta
    }
}

impl From<Metadata> for Item {
    fn from(meta: Metadata) -> Self {
        Self { origin: Origin::Metadata, meta, ..Default::default() }
    }
}

impl Item {
    pub fn discover(meta: Metadata) -> Self {
        Self { origin: Origin::Discover, meta, ..Default::default() }
    }

    pub fn directory(dir: Directory) -> Self {
        Self::from_dir(dir, Origin::Directory)
    }

    pub fn section(dir: Directory) -> Self {
        Self::from_dir(dir, Origin::Section)
    }

    fn from_dir(dir: Directory, origin: Origin) -> Self {
        // Directory fields the metadata model lacks (fastKey, agent, ...) land in `extra`.
        let mut meta: Metadata = serde_json::to_value(&dir).and_then(serde_json::from_value).unwrap_or_default();
        if meta.title.is_none() {
            meta.title = Some(dir.title().to_string());
        }
        Self { origin, meta, dir: Some(dir), ..Default::default() }
    }

    /// A root navigation entry opening `source`.
    pub fn nav(title: &str, source: Source) -> Self {
        let meta = Metadata { title: Some(title.into()), kind: Some("nav".into()), ..Default::default() };
        Self { origin: Origin::Nav, meta, link: Some(Box::new(Link { source, items: Vec::new() })), ..Default::default() }
    }

    /// A root entry listing `items` (a group of screens).
    pub fn group(title: &str, source: Source, items: Vec<Item>) -> Self {
        let mut it = Self::nav(title, source);
        if let Some(l) = it.link.as_mut() {
            l.items = items;
        }
        it
    }

    /// A row of a tool screen.
    pub fn row(title: &str, row: Row) -> Self {
        let meta = Metadata { title: Some(title.into()), kind: Some("row".into()), ..Default::default() };
        Self { origin: Origin::Row, meta, row: Some(Box::new(row)), ..Default::default() }
    }

    /// A section heading of a tool screen, which the cursor skips.
    pub fn is_heading(&self) -> bool {
        self.row.as_ref().is_some_and(|r| matches!(r.act, crate::tui::rows::Act::Heading))
    }

    /// A synthetic entry of `kind` (`hub`, `person` or `tag`) opening `source`.
    pub fn link(title: &str, kind: &str, source: Source, items: Vec<Item>) -> Self {
        let meta = Metadata { title: Some(title.into()), kind: Some(kind.into()), ..Default::default() };
        Self { origin: Origin::Link, meta, link: Some(Box::new(Link { source, items })), ..Default::default() }
    }

    /// A server hub: its items, paged from its key when the hub has more.
    pub fn hub(hub: Hub) -> Self {
        let source = match hub.key.clone().filter(|_| hub.more) {
            Some(key) => Source::Hub(key),
            None => Source::Static,
        };
        let size = hub.size.filter(|n| *n > 0).unwrap_or((hub.metadata.len() + hub.directory.len()) as i64);
        let title = hub.title.clone().unwrap_or_default();
        let ident = hub.hub_identifier.clone();
        let items = Item::listing(MediaContainer { metadata: hub.metadata, directory: hub.directory, ..Default::default() });
        let mut item = Item::link(&title, "hub", source, items);
        item.child_count = Some(size);
        // Keeps the selection on the same hub across reloads.
        item.key = ident;
        item
    }

    /// The source a synthetic entry opens.
    pub fn link_source(&self) -> Option<&Source> {
        self.link.as_ref().map(|l| &l.source)
    }

    /// Every entry of a listing: metadata then directories, or the contents of its hubs.
    pub fn listing(mc: MediaContainer) -> Vec<Item> {
        let mut out: Vec<Item> = Vec::new();
        if mc.hub.is_empty() {
            out.extend(mc.metadata.into_iter().map(Item::from));
            out.extend(mc.directory.into_iter().map(Item::directory));
        } else {
            for hub in mc.hub {
                out.extend(hub.metadata.into_iter().map(Item::from));
                out.extend(hub.directory.into_iter().map(Item::directory));
            }
        }
        out
    }

    /// The JSON the CLI prints: Plex's own fields for the entry.
    pub fn json(&self) -> Value {
        let v = match &self.dir {
            Some(d) => serde_json::to_value(d),
            None => serde_json::to_value(&self.meta),
        };
        v.unwrap_or(Value::Null)
    }

    pub fn kind(&self) -> Kind {
        match self.origin {
            Origin::Nav => return Kind::Nav,
            Origin::Discover => return Kind::Remote,
            Origin::Section => return Kind::Section,
            Origin::Row => return Kind::Row,
            Origin::Link => {
                return match self.meta.kind.as_deref() {
                    Some("hub") => Kind::Hub,
                    Some("person") => Kind::Person,
                    _ => Kind::Tag,
                };
            }
            _ => {}
        }
        match self.meta.kind.as_deref() {
            Some("movie") => Kind::Movie,
            Some("show") => Kind::Show,
            Some("season") => Kind::Season,
            Some("episode") => Kind::Episode,
            Some("artist") => Kind::Artist,
            Some("album") => Kind::Album,
            Some("track") => Kind::Track,
            Some("photo") if self.origin == Origin::Metadata => Kind::Photo,
            Some("photo" | "photoalbum") => Kind::PhotoAlbum,
            Some("clip") => Kind::Clip,
            Some("collection") => Kind::Collection,
            Some("playlist") => Kind::Playlist,
            _ if self.origin == Origin::Directory => Kind::Folder,
            _ => Kind::Other,
        }
    }

    /// Items that are played rather than drilled into.
    pub fn is_playable(&self) -> bool {
        matches!(self.kind(), Kind::Movie | Kind::Episode | Kind::Track | Kind::Clip | Kind::Photo)
    }

    /// Items with children that can be browsed.
    pub fn is_container(&self) -> bool {
        self.link.is_some() || (!self.is_playable() && self.children_key().is_some())
    }

    /// Items whose watch state can be toggled with scrobble/unscrobble.
    pub fn is_watchable(&self) -> bool {
        self.rating_key.is_some() && matches!(self.kind(), Kind::Movie | Kind::Show | Kind::Season | Kind::Episode | Kind::Clip)
    }

    /// `fastKey` of a directory entry (a ready-made filtered listing).
    pub fn fast_key(&self) -> Option<String> {
        self.extra.get("fastKey").and_then(Value::as_str).map(str::to_string)
    }

    /// The key to load for this item's children, before resolving relative
    /// keys against the parent path.
    pub fn children_key(&self) -> Option<String> {
        match self.origin {
            Origin::Section => return Some(format!("/library/sections/{}/all", self.key.as_deref()?)),
            Origin::Directory => return self.fast_key().or_else(|| self.key.clone()),
            Origin::Discover | Origin::Row => return None,
            _ => {}
        }
        if self.kind() == Kind::Playlist {
            return self.rating_key.as_ref().map(|k| format!("/playlists/{k}/items")).or_else(|| self.key.clone());
        }
        self.key.clone()
    }

    pub fn sort_title(&self) -> &str {
        self.title_sort.as_deref().unwrap_or(self.title())
    }

    pub fn in_progress(&self) -> bool {
        self.view_offset.unwrap_or(0) > 0
    }

    /// The library section id, as used in paths.
    pub fn section_id(&self) -> Option<String> {
        match self.origin {
            Origin::Section => self.key.clone(),
            _ => self.library_section_id.map(|id| id.to_string()),
        }
    }

    /// Main display title including episode/track context.
    pub fn display_title(&self) -> String {
        let title = self.title();
        match self.kind() {
            Kind::Episode => match (self.parent_index, self.index) {
                (Some(s), Some(e)) => dot_join(&[&format!("S{s:02}E{e:02}"), title]),
                (None, Some(e)) => dot_join(&[&format!("E{e:02}"), title]),
                _ => title.to_string(),
            },
            Kind::Track => match self.index {
                Some(n) => format!("{n:>2}. {title}"),
                None => title.to_string(),
            },
            _ => title.to_string(),
        }
    }

    /// Title with its parents, used where an item appears out of context
    /// (search results, On Deck, playlists, sessions).
    pub fn full_title(&self) -> String {
        let or_empty = |s: &Option<String>| s.as_deref().unwrap_or("").to_string();
        let title = self.title();
        match self.kind() {
            Kind::Episode => match (self.parent_index, self.index) {
                (Some(s), Some(e)) => dot_join(&[&or_empty(&self.grandparent_title), &format!("S{s:02}E{e:02}"), title]),
                _ => dot_join(&[&or_empty(&self.grandparent_title), title]),
            },
            Kind::Season | Kind::Album => dot_join(&[&or_empty(&self.parent_title), title]),
            Kind::Track => dot_join(&[&or_empty(&self.grandparent_title), title]),
            _ => title.to_string(),
        }
    }

    /// The rating shown in lists: audience, then critic.
    pub fn best_rating(&self) -> Option<f64> {
        self.audience_rating.or(self.rating).filter(|r| *r > 0.0)
    }

    pub fn primary_part(&self) -> Option<&Part> {
        self.media.first().and_then(|m| m.parts.first())
    }

    /// Whether Plex reports the field as locked against agent updates.
    pub fn is_field_locked(&self, name: &str) -> bool {
        let Some(Value::Array(fields)) = self.extra.get("Field") else { return false };
        fields.iter().any(|f| {
            f.get("name").and_then(Value::as_str) == Some(name)
                && f.get("locked").is_some_and(|l| l.as_bool().unwrap_or(l.as_i64() == Some(1)))
        })
    }

    /// Plex's numeric metadata type, used by edit and collection endpoints.
    pub fn type_number(&self) -> Option<u32> {
        patina_plex::MediaType::from_name(self.meta.kind.as_deref()?).map(patina_plex::MediaType::id)
    }

    /// A collection's sort: 0 release date, 1 title, 2 custom.
    pub fn collection_sort(&self) -> Option<i32> {
        let v = self.extra.get("collectionSort")?;
        v.as_i64().or_else(|| v.as_str()?.parse().ok()).map(|n| n as i32)
    }

    /// The playlist type that can hold this item.
    pub fn playlist_kind(&self) -> &'static str {
        match self.kind() {
            Kind::Track | Kind::Album | Kind::Artist => "audio",
            Kind::Photo | Kind::PhotoAlbum => "photo",
            _ => "video",
        }
    }

    /// Apply a watched/unwatched change locally so the UI reflects it at once.
    pub fn set_watched(&mut self, watched: bool) {
        if let Some(total) = self.leaf_count {
            self.viewed_leaf_count = Some(if watched { total } else { 0 });
        }
        self.view_count = Some(if watched { self.view_count.unwrap_or(0).max(1) } else { 0 });
        self.view_offset = None;
    }
}

/// Resolve a child key against the path it was listed under. Plex returns
/// relative keys (e.g. `"18"` for a genre under `/library/sections/1/genre`).
pub fn resolve_key(parent_path: &str, key: &str) -> String {
    if key.starts_with('/') {
        return key.to_string();
    }
    let base = parent_path.split('?').next().unwrap_or(parent_path);
    format!("{}/{}", base.trim_end_matches('/'), key)
}

/// Human label for an extra's `extraType`.
pub fn extra_label(extra_type: Option<i64>, subtype: Option<&str>) -> &'static str {
    match (extra_type, subtype) {
        (Some(1), _) | (_, Some("trailer")) => "Trailer",
        (Some(2), _) | (_, Some("deletedScene")) => "Deleted scene",
        (Some(3), _) | (_, Some("interview")) => "Interview",
        (Some(4), _) | (_, Some("musicVideo")) => "Music video",
        (Some(5), _) | (_, Some("behindTheScenes")) => "Behind the scenes",
        (Some(6), _) | (_, Some("sceneOrSample")) => "Scene",
        (Some(7), _) | (_, Some("liveMusicVideo")) => "Live music video",
        (Some(8), _) | (_, Some("lyricMusicVideo")) => "Lyric video",
        (Some(9), _) | (_, Some("concert")) => "Concert",
        (Some(10), _) | (_, Some("featurette")) => "Featurette",
        (Some(11), _) | (_, Some("short")) => "Short",
        _ => "Extra",
    }
}

/// Joins the non-empty parts with " · ", so missing titles leave no dangling separators.
fn dot_join(parts: &[&str]) -> String {
    parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" · ")
}

#[cfg(test)]
pub fn test_item(kind: &str, title: &str) -> Item {
    Item::from(Metadata { kind: Some(kind.into()), title: Some(title.into()), ..Default::default() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> MediaContainer {
        #[derive(serde::Deserialize)]
        struct Env {
            #[serde(rename = "MediaContainer")]
            mc: MediaContainer,
        }
        serde_json::from_str::<Env>(json).unwrap().mc
    }

    #[test]
    fn parses_show_listing() {
        let mc = parse(
            r#"{"MediaContainer":{"size":1,"totalSize":40,"Metadata":[
            {"ratingKey":"101","key":"/library/metadata/101/children","type":"show",
             "title":"Severance","year":2022,"leafCount":19,"viewedLeafCount":9,
             "librarySectionID":2,"Genre":[{"tag":"Drama"}]}]}}"#,
        );
        assert_eq!(mc.total_size, Some(40));
        let items = Item::listing(mc);
        let show = &items[0];
        assert_eq!(show.kind(), Kind::Show);
        assert!(show.is_container());
        assert!(!show.is_playable());
        assert_eq!(show.section_id().as_deref(), Some("2"));
        assert_eq!(show.unwatched_leaves(), Some(10));
        assert!(!show.is_watched());
        assert_eq!(show.genres[0].name(), "Drama");
    }

    #[test]
    fn directories_use_fast_key() {
        let mc = parse(
            r#"{"MediaContainer":{"Directory":[
            {"key":"18","fastKey":"/library/sections/1/all?genre=18","title":"Comedy"}]}}"#,
        );
        let items = Item::listing(mc);
        assert_eq!(items[0].kind(), Kind::Folder);
        assert_eq!(items[0].title(), "Comedy");
        assert_eq!(items[0].children_key().as_deref(), Some("/library/sections/1/all?genre=18"));
        assert_eq!(items[0].json()["fastKey"], "/library/sections/1/all?genre=18");
    }

    #[test]
    fn sections_list_their_items() {
        let dir = Directory { key: Some("3".into()), kind: Some("movie".into()), title: Some("Films".into()), ..Default::default() };
        let s = Item::section(dir);
        assert_eq!(s.kind(), Kind::Section);
        assert_eq!(s.children_key().as_deref(), Some("/library/sections/3/all"));
        assert_eq!(s.section_id().as_deref(), Some("3"));
    }

    #[test]
    fn episode_titles() {
        let mut ep = test_item("episode", "Pilot");
        ep.grandparent_title = Some("Show".into());
        ep.parent_index = Some(1);
        ep.index = Some(2);
        ep.duration = Some(1000);
        ep.view_offset = Some(250);
        assert_eq!(ep.display_title(), "S01E02 · Pilot");
        assert_eq!(ep.full_title(), "Show · S01E02 · Pilot");
        assert_eq!(ep.progress(), Some(0.25));
        assert!(ep.in_progress());
    }

    #[test]
    fn titles_skip_missing_parts() {
        let mut ep = test_item("episode", "");
        ep.grandparent_title = Some("Detective Conan".into());
        ep.parent_index = Some(29);
        ep.index = Some(3);
        assert_eq!(ep.full_title(), "Detective Conan · S29E03");
        assert_eq!(ep.display_title(), "S29E03");
    }

    #[test]
    fn set_watched_updates_counters() {
        let mut season = test_item("season", "S1");
        season.leaf_count = Some(10);
        season.viewed_leaf_count = Some(3);
        season.set_watched(true);
        assert!(season.is_watched());
        season.set_watched(false);
        assert_eq!(season.unwatched_leaves(), Some(10));
    }

    #[test]
    fn reads_field_locks() {
        let mc = parse(r#"{"MediaContainer":{"Metadata":[{"ratingKey":"7","type":"movie","title":"A","Field":[{"name":"title","locked":true}]}]}}"#);
        let it = &Item::listing(mc)[0];
        assert!(it.is_field_locked("title"));
        assert!(!it.is_field_locked("summary"));
        assert_eq!(it.type_number(), Some(1));
    }

    #[test]
    fn discover_items_are_inert() {
        let mut it = Item::discover(Metadata { kind: Some("movie".into()), ..Default::default() });
        it.rating_key = Some("5d77".into());
        it.key = Some("/library/metadata/5d77".into());
        assert_eq!(it.kind(), Kind::Remote);
        assert!(!it.is_playable());
        assert!(!it.is_container());
        assert!(!it.is_watchable());
    }

    #[test]
    fn resolves_relative_keys() {
        assert_eq!(resolve_key("/library/sections/1/genre", "18"), "/library/sections/1/genre/18");
        assert_eq!(resolve_key("/library/sections/1/folder?parent=3", "/library/metadata/9"), "/library/metadata/9");
        assert_eq!(resolve_key("/library/sections/1/folder?parent=3", "x"), "/library/sections/1/folder/x");
    }

    #[test]
    fn labels_extras() {
        assert_eq!(extra_label(Some(1), None), "Trailer");
        assert_eq!(extra_label(None, Some("featurette")), "Featurette");
    }
}
