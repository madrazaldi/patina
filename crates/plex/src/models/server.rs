use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SessionUser {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub thumb: Option<String>,
}

/// The client playing a session.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub address: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub device: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub machine_identifier: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub model: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub platform: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub platform_version: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub product: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub profile: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub remote_public_address: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub state: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub vendor: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub version: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub local: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub relayed: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub secure: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SessionInfo {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub bandwidth: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub location: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscodeSession {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub throttled: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub complete: bool,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub progress: Option<f64>,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub speed: Option<f64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub duration: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub remaining: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub context: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub source_video_codec: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub source_audio_codec: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub video_decision: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub audio_decision: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub subtitle_decision: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub protocol: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub container: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub video_codec: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub audio_codec: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub audio_channels: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub width: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub height: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub transcode_hw_requested: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub transcode_hw_full_pipeline: bool,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub max_offset_available: Option<f64>,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub min_offset_available: Option<f64>,
}

/// A server or account preference.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub id: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub label: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub summary: Option<String>,
    #[serde(rename = "type", default, deserialize_with = "crate::de::opt_string")]
    pub kind: Option<String>,
    #[serde(default)]
    pub default: Value,
    #[serde(default)]
    pub value: Value,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub hidden: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub advanced: bool,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub group: Option<String>,
    /// `value:Label|value:Label` pairs for enum settings.
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub enum_values: Option<String>,
}

impl Setting {
    pub fn value_string(&self) -> String {
        match &self.value {
            Value::String(s) => s.clone(),
            Value::Null => String::new(),
            v => v.to_string(),
        }
    }

    pub fn enum_options(&self) -> Vec<(String, String)> {
        self.enum_values
            .as_deref()
            .unwrap_or("")
            .split('|')
            .filter(|s| !s.is_empty())
            .map(|pair| match pair.split_once(':') {
                Some((v, l)) => (v.to_string(), l.to_string()),
                None => (pair.to_string(), pair.to_string()),
            })
            .collect()
    }
}

/// A background server activity (scan, refresh, optimise, ...).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub uuid: Option<String>,
    #[serde(rename = "type", default, deserialize_with = "crate::de::opt_string")]
    pub kind: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub cancellable: bool,
    #[serde(rename = "userID", default, deserialize_with = "crate::de::opt_i64")]
    pub user_id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub subtitle: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub progress: Option<i64>,
    #[serde(rename = "Context", default)]
    pub context: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub default_audio_language: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub auto_select_audio: bool,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub default_subtitle_language: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub subtitle_mode: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub thumb: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub platform: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub client_identifier: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub created_at: Option<i64>,
}

/// Entries from `/clients` and `/servers`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerDevice {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub host: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub address: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub port: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub machine_identifier: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub version: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub protocol: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub product: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub device_class: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub protocol_version: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub protocol_capabilities: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsMedia {
    #[serde(rename = "accountID", default, deserialize_with = "crate::de::opt_i64")]
    pub account_id: Option<i64>,
    #[serde(rename = "deviceID", default, deserialize_with = "crate::de::opt_i64")]
    pub device_id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub timespan: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub at: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub metadata_type: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub count: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub duration: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsResources {
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub timespan: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub at: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub host_cpu_utilization: Option<f64>,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub process_cpu_utilization: Option<f64>,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub host_memory_utilization: Option<f64>,
    #[serde(default, deserialize_with = "crate::de::opt_f64")]
    pub process_memory_utilization: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsBandwidth {
    #[serde(rename = "accountID", default, deserialize_with = "crate::de::opt_i64")]
    pub account_id: Option<i64>,
    #[serde(rename = "deviceID", default, deserialize_with = "crate::de::opt_i64")]
    pub device_id: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub timespan: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub at: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub lan: bool,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub bytes: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub key: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub version: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub added: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub fixed: Option<String>,
    #[serde(rename = "downloadURL", default, deserialize_with = "crate::de::opt_string")]
    pub download_url: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub state: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ButlerTask {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub title: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub description: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_i64")]
    pub interval: Option<i64>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub enabled: bool,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub schedule_randomized: bool,
}

/// `/identity` response.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub machine_identifier: Option<String>,
    #[serde(default, deserialize_with = "crate::de::opt_string")]
    pub version: Option<String>,
    #[serde(default, deserialize_with = "crate::de::boolean")]
    pub claimed: bool,
}
