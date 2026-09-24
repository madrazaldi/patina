//! Modal dialog state.

use patina_core::config::QUALITIES;
use patina_plex::{HomeUser, MatchResult, Stream, stream_type};

use crate::item::Item;

use super::column::SortKey;

pub enum Modal {
    Help {
        scroll: u16,
    },
    Sort {
        cursor: usize,
    },
    Browse {
        section_id: String,
        section_title: String,
        cursor: usize,
    },
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
    FixMatch,
    Unmatch,
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
    EditField {
        item: Box<Item>,
        field: &'static str,
    },
    PosterUrl {
        rating_key: String,
    },
    MatchQuery {
        rating_key: String,
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

/// Editable fields: Plex field name and label. `labels` is a tag list.
pub const EDIT_FIELDS: &[(&str, &str)] = &[
    ("title", "Title"),
    ("titleSort", "Sort title"),
    ("originalTitle", "Original title"),
    ("originallyAvailableAt", "Release date"),
    ("year", "Year"),
    ("studio", "Studio"),
    ("contentRating", "Content rating"),
    ("tagline", "Tagline"),
    ("summary", "Summary"),
    ("labels", "Labels"),
];

/// Current value of an editable field.
pub fn field_value(item: &Item, field: &str) -> String {
    let s = |o: &Option<String>| o.clone().unwrap_or_default();
    match field {
        "title" => item.title().to_string(),
        "titleSort" => s(&item.title_sort),
        "originalTitle" => s(&item.original_title),
        "originallyAvailableAt" => s(&item.originally_available_at),
        "year" => item.year.map(|y| y.to_string()).unwrap_or_default(),
        "studio" => s(&item.studio),
        "contentRating" => s(&item.content_rating),
        "tagline" => s(&item.tagline),
        "summary" => s(&item.summary),
        "labels" => item
            .labels
            .iter()
            .map(|t| t.name())
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

/// Apply an edit locally so the UI reflects it before the server confirms.
pub fn set_field_value(item: &mut Item, field: &str, value: &str) {
    let opt = || Some(value.to_string()).filter(|v| !v.is_empty());
    match field {
        "title" => item.title = Some(value.to_string()),
        "titleSort" => item.title_sort = opt(),
        "originalTitle" => item.original_title = opt(),
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
    pub cursor: usize,
}

// ---- generic background-loaded picker ------------------------------------

pub enum PickKind {
    Posters { rating_key: String },
    Matches { rating_key: String },
    Subtitles { rating_key: String },
    Users,
}

pub enum PickEntry {
    Poster(Box<Item>),
    PosterFromUrl,
    Match(Box<MatchResult>),
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

pub fn sort_rows() -> &'static [SortKey] {
    &SortKey::ALL
}

/// Section sub-views (Plex "secondary" directories).
pub const BROWSE_VIEWS: &[(&str, &str)] = &[
    ("All", "all"),
    ("Unwatched", "all?unwatched=1"),
    ("Recently added", "recentlyAdded"),
    ("Recently viewed", "recentlyViewed"),
    ("On deck", "onDeck"),
    ("Collections", "collections"),
    ("Genres", "genre"),
    ("Years", "year"),
    ("Decades", "decade"),
    ("Directors", "director"),
    ("Actors", "actor"),
    ("Studios", "studio"),
    ("Content ratings", "contentRating"),
    ("Resolutions", "resolution"),
    ("By first letter", "firstCharacter"),
    ("Folders", "folder"),
];
