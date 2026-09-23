use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::*;

/// Top-level response wrapper: `{"MediaContainer": {...}}`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Envelope {
    #[serde(rename = "MediaContainer", default)]
    pub media_container: MediaContainer,
}

/// The generic container almost every server endpoint returns. Each endpoint
/// populates a different subset of the collections.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaContainer {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub size: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub total_size: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub offset: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title1: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title2: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub identifier: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub art: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub thumb: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub view_group: Option<String>,
    #[serde(rename = "librarySectionID", default, deserialize_with = "crate::de::opt_i64")]
    pub library_section_id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub library_section_title: Option<String>,
    #[serde(rename = "librarySectionUUID", default, deserialize_with = "crate::de::opt_string")]
    pub library_section_uuid: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub machine_identifier: Option<String>,

    // Play queue fields.
    #[serde(rename = "playQueueID", default, deserialize_with = "crate::de::opt_i64")]
    pub play_queue_id: Option<i64>,
    #[serde(rename = "playQueueSelectedItemID", default, deserialize_with = "crate::de::opt_i64")]
    pub play_queue_selected_item_id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub play_queue_selected_item_offset: Option<i64>,
    #[serde(rename = "playQueueSelectedMetadataItemID", default, deserialize_with = "crate::de::opt_string")]
    pub play_queue_selected_metadata_item_id: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub play_queue_shuffled: bool,
    #[serde(rename = "playQueueSourceURI", default, deserialize_with = "crate::de::opt_string")]
    pub play_queue_source_uri: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub play_queue_total_count: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub play_queue_version: Option<i64>,

    // Updater fields.
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub can_install: bool,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub checked_at: Option<i64>,
    #[serde(rename = "downloadURL", default, deserialize_with = "crate::de::opt_string")]
    pub download_url: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub status: Option<String>,

    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<Metadata>,
    #[serde(rename = "Directory", default)]
    pub directory: Vec<Directory>,
    #[serde(rename = "Hub", default)]
    pub hub: Vec<Hub>,
    #[serde(rename = "Setting", default)]
    pub setting: Vec<Setting>,
    #[serde(rename = "Activity", default)]
    pub activity: Vec<Activity>,
    #[serde(rename = "Account", default)]
    pub account: Vec<Account>,
    #[serde(rename = "Device", default)]
    pub device: Vec<Device>,
    #[serde(rename = "Server", default)]
    pub server: Vec<ServerDevice>,
    #[serde(rename = "TranscodeSession", default)]
    pub transcode_session: Vec<TranscodeSession>,
    #[serde(rename = "StatisticsMedia", default)]
    pub statistics_media: Vec<StatisticsMedia>,
    #[serde(rename = "StatisticsResources", default)]
    pub statistics_resources: Vec<StatisticsResources>,
    #[serde(rename = "StatisticsBandwidth", default)]
    pub statistics_bandwidth: Vec<StatisticsBandwidth>,
    #[serde(rename = "Release", default)]
    pub release: Vec<Release>,
    #[serde(rename = "SearchResult", default)]
    pub search_result: Vec<MatchResult>,
    #[serde(rename = "Meta", default)]
    pub meta: Option<LibraryMeta>,

    /// Any attribute not modelled above (server capabilities, flags, ...).
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl MediaContainer {
    /// Look up an attribute from `extra` as a string.
    pub fn attr(&self, key: &str) -> Option<String> {
        match self.extra.get(key)? {
            Value::String(s) => Some(s.clone()),
            Value::Null => None,
            other => Some(other.to_string()),
        }
    }

    /// Whether there are more items after this page.
    pub fn has_more(&self) -> bool {
        match (self.offset, self.size, self.total_size) {
            (Some(off), Some(size), Some(total)) => off + size < total,
            _ => false,
        }
    }
}

/// A hub (row) of related content: "Continue Watching", "Recently Added", search groups, ...
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Hub {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub hub_key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(rename = "type", default, deserialize_with = "crate::de::opt_string")]
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub hub_identifier: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub context: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub size: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub more: bool,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub style: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub promoted: bool,
    #[serde(rename = "Metadata", default)]
    pub metadata: Vec<Metadata>,
    #[serde(rename = "Directory", default)]
    pub directory: Vec<Directory>,
}

impl Hub {
    pub fn is_empty(&self) -> bool {
        self.metadata.is_empty() && self.directory.is_empty()
    }
}

/// A "directory" entry: library sections, filter values, sorts, tag lists, capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Directory {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(rename = "type", default, deserialize_with = "crate::de::opt_string")]
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub fast_key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub thumb: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub art: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub composite: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub count: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub tag: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub filter: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub filter_type: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub default: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub default_direction: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub desc_key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub first_character_key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub secondary: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub search: bool,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub prompt: Option<String>,

    // Library section attributes.
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub agent: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub scanner: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub language: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub uuid: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub updated_at: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub created_at: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub scanned_at: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub content_changed_at: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub refreshing: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub hidden: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub allow_sync: bool,
    #[serde(rename = "Location", default)]
    pub location: Vec<Location>,
}

impl Directory {
    pub fn title(&self) -> &str {
        self.title.as_deref().or(self.tag.as_deref()).unwrap_or("")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Location {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub path: Option<String>,
}

/// `includeMeta=1` payload describing a library's types, filters, sorts and fields.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryMeta {
    #[serde(rename = "Type", default)]
    pub types: Vec<LibraryMetaType>,
    #[serde(rename = "FieldType", default)]
    pub field_types: Vec<FieldType>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMetaType {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub key: Option<String>,
    #[serde(rename = "type", default, deserialize_with = "crate::de::opt_string")]
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub active: bool,
    #[serde(rename = "Filter", default)]
    pub filters: Vec<Directory>,
    #[serde(rename = "Sort", default)]
    pub sorts: Vec<Directory>,
    #[serde(rename = "Field", default)]
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(rename = "type", default, deserialize_with = "crate::de::opt_string")]
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub sub_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FieldType {
    #[serde(rename = "type", default, deserialize_with = "crate::de::opt_string")]
    pub kind: Option<String>,
    #[serde(rename = "Operator", default)]
    pub operators: Vec<Operator>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Operator {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
}
