//! Modal dialog state.

use patina_core::config::QUALITIES;
use patina_core::ops::library::FilterView;
use patina_core::ops::server::LibraryForm;
use patina_core::ops::views::SavedView;
use patina_core::tracks;
use patina_plex::{Directory, HomeUser, MatchResult, Stream, stream_type};

use crate::item::Item;

use super::column::SortKey;
use super::marks::BulkOp;
use super::rows::SettingKey;
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
    /// Audio and subtitle tracks for a whole show or season.
    Tracks(Box<TracksModal>),
    /// The running play queue; `sel` is the uid of the selected entry.
    Queue {
        sel: Option<u64>,
    },
    /// Lyrics of the playing track. `manual` holds a line picked with j/k,
    /// and when, pausing the follow of synced lyrics for a while.
    Lyrics {
        manual: Option<(usize, std::time::Instant)>,
    },
}

// ---- action menu ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// An action on every marked item.
    Bulk(BulkOp),
    Play,
    PlayFromStart,
    PlayNext,
    AddToQueue,
    Radio,
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
    Tracks,
    SwitchUser,
    GoToParent,
    GoToGrandparent,
    DeleteHistory,
    /// An action of a tool row.
    Row(RowOp),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowOp {
    PauseDownload,
    Play,
    PlayGroup,
    Retry,
    Remove,
    OpenFolder,
    GoToItem,
    Watch,
    Record,
    Later,
    DeleteRule,
    Cancel,
    Kill,
    Start,
    Stop,
    StartAll,
    StopAll,
    Run,
    Scan,
    CancelScan,
    RefreshLibrary,
    AnalyzeLibrary,
    EmptyTrash,
    EditLibrary,
    LibrarySettings,
    BrowseLibrary,
    DeleteLibrary,
    Change,
}

pub struct Menu {
    pub title: String,
    pub rows: Vec<(MenuAction, &'static str, &'static str)>,
    pub cursor: usize,
}

// ---- text input ----------------------------------------------------------

pub enum InputPurpose {
    /// Name for the library view in the last column, to save.
    SaveView,
    /// Name for a new collection holding the marked items.
    NewCollectionWith {
        section_id: String,
        kind: u32,
        rating_keys: Vec<String>,
    },
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
    /// Signing in with a plex.tv account: email, password, then a 2FA code.
    Login,
    Password {
        login: String,
    },
    TwoFactor {
        login: String,
        password: String,
    },
    Setting(SettingKey),
    Pref {
        id: String,
        section: Option<String>,
    },
    /// Adding a library: its name, folders and language.
    LibraryName(Box<LibraryForm>),
    LibraryFolders(Box<LibraryForm>),
    LibraryLanguage(Box<LibraryForm>),
    /// Editing a library: its name, then its folders.
    EditLibraryName {
        id: String,
        folders: String,
    },
    EditLibraryFolders {
        id: String,
        name: String,
    },
}

impl InputPurpose {
    /// Typed text is hidden.
    pub fn secret(&self) -> bool {
        matches!(self, InputPurpose::TwoFactor { .. } | InputPurpose::Password { .. })
    }
}

pub struct Input {
    pub title: String,
    pub hint: String,
    pub value: String,
    pub purpose: InputPurpose,
}

// ---- confirmation --------------------------------------------------------

pub enum ConfirmAction {
    /// Take the marked entries out of a playlist.
    RemoveMarkedFromPlaylist {
        playlist_id: String,
        entries: Vec<i64>,
    },
    /// Take the marked items out of a collection.
    RemoveMarkedFromCollection {
        collection_id: String,
        rating_keys: Vec<String>,
    },
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
    RemoveDownload {
        id: String,
    },
    DeleteRule {
        id: String,
    },
    DeleteLibrary {
        id: String,
        title: String,
    },
    DeleteHistory {
        key: String,
    },
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
    /// Saved views of the library in the last column.
    Views,
    /// A playlist for the marked items (their keys, and the playlist type).
    BulkPlaylist { keys: Vec<String>, kind: &'static str },
    /// A collection for the marked items.
    BulkCollection { keys: Vec<String>, section_id: String, kind: u32 },
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
    Setting(SettingKey),
    Pref { id: String, section: Option<String> },
    Servers,
    LibraryKind,
    LibraryAgent(Box<LibraryForm>),
    LibraryScanner(Box<LibraryForm>),
}

pub enum PickEntry {
    /// A saved view.
    View(Box<SavedView>),
    /// "Save the current view".
    SaveView,
    /// A playlist or collection (id, title).
    Target(String, String),
    /// "New playlist / collection".
    NewTarget,
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
    /// A value to store (a setting, a server id, a library type, agent or scanner).
    Text(String),
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

// ---- audio & subtitles for a whole show ----------------------------------

/// The rows of the tracks dialog, in order.
pub const TRACK_FIELDS: [&str; 4] = ["Apply to", "Take tracks from", "Audio", "Subtitles"];

/// Choices made in the tracks dialog; the core plans them into a [`tracks::View`].
pub struct TracksModal {
    /// The show's rating key and title.
    pub show: String,
    pub title: String,
    pub loaded: bool,
    pub error: Option<String>,
    pub season: Option<i64>,
    pub reference: Option<String>,
    pub audio: Option<i64>,
    /// "keep", "off" or a stream id, as the core expects.
    pub subtitle: String,
    pub view: Option<tracks::View>,
    /// (done, total) while applying.
    pub progress: Option<(usize, usize)>,
    /// A field row, or `TRACK_FIELDS.len()` for Apply.
    pub cursor: usize,
    /// The field whose choices are listed, and the cursor in them.
    pub choosing: Option<(usize, usize)>,
}

impl TracksModal {
    /// For a show, season or episode: the show, with the season and episode preselected as on the GUI.
    pub fn new(item: &Item) -> Option<Self> {
        let (show, title, season, reference) = match item.meta.kind.as_deref()? {
            "show" => (item.rating_key.clone()?, item.title().to_string(), None, None),
            "season" => (item.parent_rating_key.clone()?, item.parent_title.clone().unwrap_or_default(), item.index, None),
            "episode" => (item.grandparent_rating_key.clone()?, item.grandparent_title.clone().unwrap_or_default(), item.parent_index, item.rating_key.clone()),
            _ => return None,
        };
        Some(Self {
            show,
            title,
            loaded: false,
            error: None,
            season,
            reference,
            audio: None,
            subtitle: "keep".into(),
            view: None,
            progress: None,
            cursor: 0,
            choosing: None,
        })
    }

    pub fn query(&self) -> tracks::Query {
        tracks::Query {
            show: self.show.clone(),
            season: self.season,
            reference: self.reference.clone(),
            audio: self.audio,
            subtitle: self.subtitle.clone(),
        }
    }

    /// Labels of a field's choices and the index of the current one.
    pub fn choices(&self, field: usize) -> (Vec<String>, usize) {
        let Some(v) = &self.view else { return (Vec::new(), 0) };
        let eps = |n: usize| format!("{n} episode{}", if n == 1 { "" } else { "s" });
        let pos = |labels: Vec<String>, i: Option<usize>| (labels, i.unwrap_or(0));
        match field {
            0 => {
                let mut l = vec![format!("All seasons ({})", eps(v.total))];
                l.extend(v.seasons.iter().map(|s| format!("{} ({})", s.title, eps(s.count))));
                let i = self.season.and_then(|n| v.seasons.iter().position(|s| s.index == n)).map(|i| i + 1);
                pos(l, i.or(Some(0)))
            }
            1 => {
                let i = v.scope.iter().position(|e| Some(&e.rating_key) == v.reference.as_ref());
                pos(v.scope.iter().map(|e| e.label.clone()).collect(), i)
            }
            2 => {
                let mut l = vec!["Don't change".to_string()];
                l.extend(v.audio.iter().map(|t| t.label.clone()));
                let i = self.audio.and_then(|a| v.audio.iter().position(|t| t.id == a)).map(|i| i + 1);
                pos(l, i)
            }
            _ => {
                let mut l = vec!["Don't change".to_string(), "None (turn off)".to_string()];
                l.extend(v.subtitles.iter().map(|t| format!("{}{}", t.label, if t.external { " · external" } else { "" })));
                let i = match self.subtitle.as_str() {
                    "keep" => Some(0),
                    "off" => Some(1),
                    id => v.subtitles.iter().position(|t| t.id.to_string() == id).map(|i| i + 2),
                };
                pos(l, i)
            }
        }
    }

    /// Choose entry `i` of `field`. As on the GUI, a new season or reference
    /// episode resets the track choices (their ids belong to that episode).
    pub fn choose(&mut self, field: usize, i: usize) {
        let Some(v) = &self.view else { return };
        match field {
            0 => {
                self.season = i.checked_sub(1).and_then(|i| v.seasons.get(i)).map(|s| s.index);
                self.reference = None;
                self.reset_tracks();
            }
            1 => {
                if let Some(e) = v.scope.get(i) {
                    self.reference = Some(e.rating_key.clone());
                    self.reset_tracks();
                }
            }
            2 => self.audio = i.checked_sub(1).and_then(|i| v.audio.get(i)).map(|t| t.id),
            _ => {
                self.subtitle = match i {
                    0 => "keep".into(),
                    1 => "off".into(),
                    i => v.subtitles.get(i - 2).map_or("keep".into(), |t| t.id.to_string()),
                }
            }
        }
    }

    fn reset_tracks(&mut self) {
        self.audio = None;
        self.subtitle = "keep".into();
    }

    pub fn can_apply(&self) -> bool {
        self.progress.is_none() && self.view.as_ref().is_some_and(|v| !v.plan.changes.is_empty())
    }
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

    fn tracks_view() -> tracks::View {
        use tracks::{Episode, Season, Track};
        let track = |id, label: &str| Track { id, label: label.into(), external: false };
        tracks::View {
            total: 4,
            seasons: vec![Season { index: 1, title: "Season 1".into(), count: 2 }, Season { index: 2, title: "Season 2".into(), count: 2 }],
            scope: vec![Episode { rating_key: "11".into(), label: "S01E01".into() }, Episode { rating_key: "12".into(), label: "S01E02".into() }],
            reference: Some("11".into()),
            audio: vec![track(5, "English"), track(6, "Japanese")],
            subtitles: vec![track(9, "English")],
            audio_name: None,
            subtitle_name: None,
            nothing_chosen: true,
            plan: Default::default(),
        }
    }

    #[test]
    fn tracks_choices_start_where_the_item_is() {
        let ep = Item::from(patina_plex::Metadata {
            kind: Some("episode".into()),
            rating_key: Some("12".into()),
            grandparent_rating_key: Some("1".into()),
            grandparent_title: Some("Show".into()),
            parent_index: Some(1),
            ..Default::default()
        });
        let mut m = TracksModal::new(&ep).unwrap();
        assert_eq!((m.show.as_str(), m.season, m.reference.as_deref()), ("1", Some(1), Some("12")));
        m.view = Some(tracks_view());
        assert_eq!(m.choices(0), (vec!["All seasons (4 episodes)".into(), "Season 1 (2 episodes)".into(), "Season 2 (2 episodes)".into()], 1));
        assert_eq!(m.choices(3).0[1], "None (turn off)");

        m.choose(2, 2);
        m.choose(3, 2);
        assert_eq!((m.audio, m.subtitle.as_str()), (Some(6), "9"));
        assert_eq!((m.choices(2).1, m.choices(3).1), (2, 2));
        m.choose(3, 1);
        assert_eq!(m.subtitle, "off");
        // A new episode or season resets the tracks, whose ids belong to the old one.
        m.choose(1, 0);
        assert_eq!((m.reference.as_deref(), m.audio, m.subtitle.as_str()), (Some("11"), None, "keep"));
        m.choose(2, 1);
        m.choose(0, 0);
        assert_eq!((m.season, m.reference.as_deref(), m.audio), (None, None, None));
        assert!(TracksModal::new(&Item::from(patina_plex::Metadata { kind: Some("movie".into()), ..Default::default() })).is_none());
    }

}
