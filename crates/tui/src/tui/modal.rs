//! Modal dialog state.

use patina_core::config::QUALITIES;
use patina_core::ops::library::FilterView;
use patina_plex::{Directory, HomeUser, MatchResult, Stream, stream_type};

use crate::item::Item;

use super::column::SortKey;
use super::source::Source;

pub enum Modal {
    Help {
        scroll: u16,
    },
    Sort {
        cursor: usize,
    },
    /// A library's views: its types, Recommended, Collections, Folders, browse-by lists.
    Browse {
        section_id: String,
        section_title: String,
        rows: Vec<(String, Source)>,
        cursor: usize,
    },
    /// Server filters of the library listing in the last column.
    Filter(FilterPicker),
    Menu(Menu),
    Input(Input),
    Confirm(Confirm),
    Playlists(PlaylistPicker),
    PlayOptions(PlayOptionsModal),
    Collections(CollectionPicker),
    Edit(EditModal),
    /// A list loaded in the background, then chosen from.
    Pick(Picker),
}

// ---- action menu ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Play,
    PlayFromStart,
    PlayAll,
    Shuffle,
    PlayOptions,
    Sort,
    Filter,
    Letters,
    SwitchType,
    MarkWatched,
    MarkUnwatched,
    ClearProgress,
    RemoveFromContinue,
    Rate,
    AddToPlaylist,
    Related,
    OpenWeb,
    CopyUrl,
    Download,
    RefreshMetadata,
    Analyze,
    ScanLibrary,
    ForceScanLibrary,
    EmptyTrash,
    BrowseBy,
    RenamePlaylist,
    DeletePlaylist,
    RemoveFromPlaylist,
    RemoveFromCollection,
    TerminateSession,
    Watchlist,
    Collections,
    SmartPlaylist,
    Extras,
    EditDetails,
    ChangePoster,
    ChangeArt,
    CollectionMode,
    CollectionSort,
    ClearPlaylist,
    EditPlaylistSummary,
    FixMatch,
    DeleteMedia,
    SearchSubtitles,
    SwitchUser,
}

pub struct Menu {
    pub title: String,
    pub rows: Vec<(MenuAction, &'static str, &'static str)>,
    pub cursor: usize,
}

// ---- text input ----------------------------------------------------------

pub enum InputPurpose {
    NewPlaylist {
        rating_keys: Vec<String>,
        kind: &'static str,
    },
    RenamePlaylist {
        playlist_id: String,
    },
    Rate {
        rating_key: String,
    },
    NewCollection {
        rating_key: String,
    },
    SmartPlaylist {
        kind: &'static str,
        filter_path: String,
    },
    /// A field of the item in the edit dialog, which reopens afterwards.
    EditField {
        edit: Box<EditModal>,
        field: &'static str,
    },
    ArtworkUrl {
        rating_key: String,
        art: bool,
    },
    PlaylistSummary {
        playlist_id: String,
    },
    MatchQuery {
        rating_key: String,
        title: String,
        year: Option<i64>,
    },
    SubtitleLanguage {
        rating_key: String,
    },
    UserPin {
        user: Box<HomeUser>,
    },
    /// Signing in to a server directly: its URL, then its token.
    ServerUrl,
    ServerToken {
        url: String,
    },
}

pub struct Input {
    pub title: String,
    pub hint: String,
    pub value: String,
    pub purpose: InputPurpose,
}

// ---- confirmation --------------------------------------------------------

pub enum ConfirmAction {
    DeletePlaylist {
        playlist_id: String,
    },
    RemoveFromPlaylist {
        playlist_id: String,
        playlist_item_id: i64,
    },
    RemoveFromCollection {
        collection_id: String,
        rating_key: String,
    },
    TerminateSession {
        session_id: String,
    },
    EmptyTrash {
        section_id: String,
    },
    Unmatch {
        rating_key: String,
    },
    DeleteMedia {
        rating_key: String,
        title: String,
    },
    DeleteCollection {
        rating_key: String,
        title: String,
    },
    ClearPlaylist {
        playlist_id: String,
    },
    Logout,
}

pub struct Confirm {
    pub message: String,
    pub action: ConfirmAction,
}

// ---- playlist membership -------------------------------------------------

pub struct PlaylistRow {
    pub playlist: Item,
    /// The entry id of the item within this playlist, if it is a member.
    pub entry_id: Option<i64>,
    pub busy: bool,
}

pub struct PlaylistPicker {
    pub item: Item,
    pub kind: &'static str,
    pub rows: Vec<PlaylistRow>,
    pub cursor: usize,
    pub loading: bool,
    pub error: Option<String>,
}

// ---- play options --------------------------------------------------------

/// Choices from the play-options dialog.
#[derive(Debug, Clone, Default)]
pub struct PlayOptions {
    pub from_start: bool,
    /// Index into `item.media` (alternate versions).
    pub media_index: usize,
    /// Plex stream id of the audio track.
    pub audio_stream_id: Option<i64>,
    /// Plex stream id of the subtitle track; `Some(0)` means off.
    pub subtitle_stream_id: Option<i64>,
    /// Index into [`QUALITIES`]; 0 plays the original file.
    pub quality: usize,
}

impl PlayOptions {
    pub fn with_quality(id: &str) -> Self {
        Self { quality: QUALITIES.iter().position(|q| q.id == id).unwrap_or(0), ..Default::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptRow {
    Header(&'static str),
    Version(usize),
    Audio(i64),
    Subtitle(Option<i64>),
    Resume(bool),
    Quality(usize),
    Play,
}

pub struct PlayOptionsModal {
    /// Full metadata (with streams); `None` while loading.
    pub item: Option<Item>,
    pub opts: PlayOptions,
    pub cursor: usize,
    pub rows: Vec<(OptRow, String)>,
}

impl PlayOptionsModal {
    pub fn selectable(&self, i: usize) -> bool {
        !matches!(self.rows.get(i), Some((OptRow::Header(_), _)) | None)
    }

    pub fn is_checked(&self, row: &OptRow) -> bool {
        let item = self.item.as_ref();
        let part = item.and_then(|i| i.media.get(self.opts.media_index)).and_then(|m| m.parts.first());
        let selected = |id: i64| part.and_then(|p| p.streams.iter().find(|s| s.id == Some(id))).is_some_and(|s| s.selected);
        match row {
            OptRow::Version(i) => *i == self.opts.media_index,
            OptRow::Audio(id) => match self.opts.audio_stream_id {
                Some(sel) => sel == *id,
                None => selected(*id),
            },
            OptRow::Subtitle(id) => match (self.opts.subtitle_stream_id, id) {
                (Some(sel), None) => sel == 0,
                (Some(sel), Some(id)) => sel == *id,
                (None, Some(id)) => selected(*id),
                (None, None) => part.is_some_and(|p| !p.streams.iter().any(|s| s.stream_type == Some(stream_type::SUBTITLE) && s.selected)),
            },
            OptRow::Resume(from_start) => *from_start == self.opts.from_start,
            OptRow::Quality(q) => *q == self.opts.quality,
            _ => false,
        }
    }
}

// ---- collection membership -----------------------------------------------

pub struct CollectionRow {
    pub collection: Item,
    pub member: bool,
    pub busy: bool,
}

pub struct CollectionPicker {
    pub item: Item,
    pub rows: Vec<CollectionRow>,
    pub cursor: usize,
    pub loading: bool,
    pub error: Option<String>,
}

// ---- metadata editing ----------------------------------------------------

/// Fields the GUI's editor offers for a kind, as (Plex field, label):
/// short fields, then the `genre`/`label` tag lists, then the summary.
pub fn edit_fields(kind: &str) -> Vec<(&'static str, &'static str)> {
    let fields: &[&str] = match kind {
        "movie" => &["title", "titleSort", "originalTitle", "editionTitle", "year", "originallyAvailableAt", "contentRating", "studio", "tagline", "summary"],
        "show" => &["title", "titleSort", "originalTitle", "year", "originallyAvailableAt", "contentRating", "studio", "tagline", "summary"],
        "season" => &["title", "summary"],
        "episode" => &["title", "titleSort", "originallyAvailableAt", "summary"],
        "artist" => &["title", "titleSort", "summary"],
        "album" => &["title", "titleSort", "year", "originallyAvailableAt", "studio", "summary"],
        "track" => &["title", "titleSort", "originalTitle"],
        "collection" => &["title", "titleSort", "contentRating", "summary"],
        _ => &["title", "titleSort", "summary"],
    };
    let tags: &[&str] = match kind {
        "movie" | "show" | "artist" | "album" => &["genre", "label"],
        "collection" => &["label"],
        _ => &[],
    };
    let label = |f: &'static str| match (kind, f) {
        ("album", "studio") => "Record label",
        ("track", "originalTitle") => "Track artist",
        (_, "title") => "Title",
        (_, "titleSort") => "Sort title",
        (_, "originalTitle") => "Original title",
        (_, "editionTitle") => "Edition",
        (_, "year") => "Year",
        (_, "originallyAvailableAt") => "Release date",
        (_, "contentRating") => "Content rating",
        (_, "studio") => "Studio",
        (_, "tagline") => "Tagline",
        (_, "summary") => "Summary",
        (_, "genre") => "Genres",
        (_, "label") => "Labels",
        _ => f,
    };
    let short = fields.iter().copied().filter(|f| *f != "summary");
    let summary = fields.iter().copied().filter(|f| *f == "summary");
    short.chain(tags.iter().copied()).chain(summary).map(|f| (f, label(f))).collect()
}

/// Whether an edit field is a tag list rather than a value.
pub fn is_tag_field(field: &str) -> bool {
    matches!(field, "genre" | "label")
}

/// Current value of an editable field.
pub fn field_value(item: &Item, field: &str) -> String {
    let s = |o: &Option<String>| o.clone().unwrap_or_default();
    let tags = |list: &[patina_plex::Tag]| list.iter().map(|t| t.name()).filter(|n| !n.is_empty()).collect::<Vec<_>>().join(", ");
    match field {
        "title" => item.title().to_string(),
        "titleSort" => s(&item.title_sort),
        "originalTitle" => s(&item.original_title),
        "editionTitle" => s(&item.edition_title),
        "originallyAvailableAt" => s(&item.originally_available_at),
        "year" => item.year.map(|y| y.to_string()).unwrap_or_default(),
        "studio" => s(&item.studio),
        "contentRating" => s(&item.content_rating),
        "tagline" => s(&item.tagline),
        "summary" => s(&item.summary),
        "genre" => tags(&item.genres),
        "label" => tags(&item.labels),
        _ => String::new(),
    }
}

/// Why a value can't be saved, as the GUI's editor checks it.
pub fn invalid_value(field: &str, value: &str) -> Option<&'static str> {
    let v = value.trim();
    let digits = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_digit());
    match field {
        "year" if !v.is_empty() && !digits(v, 4) => Some("Year should be four digits."),
        "originallyAvailableAt" if !v.is_empty() => {
            let p: Vec<&str> = v.split('-').collect();
            (p.len() != 3 || !digits(p[0], 4) || !digits(p[1], 2) || !digits(p[2], 2)).then_some("Release date should be YYYY-MM-DD.")
        }
        _ => None,
    }
}

/// Split a comma-separated tag list, dropping blanks and duplicates.
pub fn split_tags(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        if !out.iter().any(|x| x == t) {
            out.push(t.to_string());
        }
    }
    out
}

/// Tags to add and remove to go from `before` to `after`, ignoring case.
pub fn tag_diff(before: &[String], after: &[String]) -> (Vec<String>, Vec<String>) {
    let has = |list: &[String], t: &str| list.iter().any(|x| x.eq_ignore_ascii_case(t));
    let add = after.iter().filter(|t| !has(before, t)).cloned().collect();
    let remove = before.iter().filter(|t| !has(after, t)).cloned().collect();
    (add, remove)
}

/// Apply an edit locally so the UI reflects it before the server confirms.
pub fn set_field_value(item: &mut Item, field: &str, value: &str) {
    let opt = || Some(value.to_string()).filter(|v| !v.is_empty());
    match field {
        "title" => item.title = Some(value.to_string()),
        "titleSort" => item.title_sort = opt(),
        "originalTitle" => item.original_title = opt(),
        "editionTitle" => item.edition_title = opt(),
        "originallyAvailableAt" => item.originally_available_at = opt(),
        "year" => item.year = value.parse().ok(),
        "studio" => item.studio = opt(),
        "contentRating" => item.content_rating = opt(),
        "tagline" => item.tagline = opt(),
        "summary" => item.summary = opt(),
        _ => {}
    }
}

pub struct EditModal {
    pub item: Item,
    pub fields: Vec<(&'static str, &'static str)>,
    pub cursor: usize,
}

// ---- generic background-loaded picker ------------------------------------

pub enum PickKind {
    /// Server sorts of the library listing in the last column.
    Sort,
    /// A–Z jump in the library listing in the last column.
    Letters,
    /// Posters, or backgrounds when `art`.
    Artwork { rating_key: String, art: bool },
    CollectionMode { rating_key: String },
    CollectionSort { rating_key: String },
    Matches { rating_key: String, title: String, year: Option<i64> },
    Subtitles { rating_key: String },
    Users,
}

pub enum PickEntry {
    Sort(Box<Directory>),
    /// A letter and the row it starts at.
    Letter(String, usize),
    Artwork(Box<Item>),
    ArtworkFromUrl,
    /// A numeric setting (collection mode or sort).
    Value(i32),
    Match(Box<MatchResult>),
    MatchSearch,
    Unmatch,
    Subtitle(Box<Stream>),
    User(Box<HomeUser>),
}

pub struct Picker {
    pub title: String,
    pub kind: PickKind,
    pub rows: Vec<(PickEntry, String)>,
    pub cursor: usize,
    pub loading: bool,
    pub error: Option<String>,
    pub hint: &'static str,
}

/// A collection's "show in library" modes and sorts, as the GUI offers them.
pub const COLLECTION_MODES: &[(i32, &str)] = &[(-1, "Library default"), (0, "Hide collection"), (1, "Hide items"), (2, "Show both")];
pub const COLLECTION_SORTS: &[(i32, &str)] = &[(0, "Release date"), (1, "Title"), (2, "Custom")];

pub fn sort_rows() -> &'static [SortKey] {
    &SortKey::ALL
}

// ---- library filters -----------------------------------------------------

pub struct FilterPicker {
    pub section_title: String,
    pub defs: Vec<FilterView>,
    pub loading: bool,
    pub error: Option<String>,
    pub cursor: usize,
    /// The value filter being chosen from.
    pub values: Option<FilterValues>,
}

pub struct FilterValues {
    pub def: FilterView,
    /// (value, label)
    pub all: Vec<(String, String)>,
    pub loading: bool,
    pub error: Option<String>,
    pub query: String,
    pub cursor: usize,
}

impl FilterValues {
    /// Values matching the typed query.
    pub fn shown(&self) -> Vec<&(String, String)> {
        let q = self.query.trim().to_lowercase();
        self.all.iter().filter(|(_, l)| q.is_empty() || l.to_lowercase().contains(&q)).collect()
    }
}

/// Section sub-listings offered next to the library's own views.
pub const BROWSE_VIEWS: &[(&str, &str)] = &[
    ("Recently added", "recentlyAdded"),
    ("Recently viewed", "recentlyViewed"),
    ("On deck", "onDeck"),
    ("By genre", "genre"),
    ("By year", "year"),
    ("By decade", "decade"),
    ("By director", "director"),
    ("By actor", "actor"),
    ("By studio", "studio"),
    ("By content rating", "contentRating"),
    ("By resolution", "resolution"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_fields_follow_the_gui() {
        let movie: Vec<&str> = edit_fields("movie").into_iter().map(|f| f.0).collect();
        assert_eq!(movie.first(), Some(&"title"));
        assert_eq!(&movie[movie.len() - 3..], &["genre", "label", "summary"]);
        assert!(edit_fields("episode").iter().all(|f| !is_tag_field(f.0)));
        assert_eq!(edit_fields("album").iter().find(|f| f.0 == "studio").map(|f| f.1), Some("Record label"));
        assert_eq!(edit_fields("collection").iter().filter(|f| is_tag_field(f.0)).count(), 1);
    }

    #[test]
    fn tags_split_and_diff() {
        assert_eq!(split_tags(" Drama, ,Thriller,Drama "), vec!["Drama", "Thriller"]);
        let (add, remove) = tag_diff(&split_tags("Drama, Mystery"), &split_tags("drama, Thriller"));
        assert_eq!(add, vec!["Thriller"]);
        assert_eq!(remove, vec!["Mystery"]);
    }

    #[test]
    fn validates_like_the_gui() {
        assert!(invalid_value("year", "99").is_some());
        assert!(invalid_value("year", "").is_none());
        assert!(invalid_value("originallyAvailableAt", "2014-5-3").is_some());
        assert!(invalid_value("originallyAvailableAt", "2014-05-03").is_none());
        assert!(invalid_value("title", "x").is_none());
    }
}
