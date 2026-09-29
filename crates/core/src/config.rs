//! Persistent settings, stored as TOML in `~/.config/patina/config.toml`
//! (or the platform config dir on Windows).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const PRODUCT: &str = "Patina";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Stable per-install identifier sent as `X-Plex-Client-Identifier`.
    pub client_id: String,
    pub auth: AuthSettings,
    pub server: ServerSettings,
    pub player: PlayerSettings,
    /// The GUI's appearance.
    pub ui: UiSettings,
    /// The terminal client's appearance and behaviour.
    pub tui: TuiSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            client_id: uuid::Uuid::new_v4().to_string(),
            auth: AuthSettings::default(),
            server: ServerSettings::default(),
            player: PlayerSettings::default(),
            ui: UiSettings::default(),
            tui: TuiSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthSettings {
    pub account_id: Option<String>,
    /// plex.tv account token (or the active Plex Home user's token).
    pub token: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerSettings {
    /// `clientIdentifier` of the preferred server.
    pub id: Option<String>,
    /// Last connection URI that worked, tried first on startup.
    pub last_uri: Option<String>,
    /// Manually configured server (no plex.tv account).
    pub manual_url: Option<String>,
    pub manual_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlayerSettings {
    /// mpv executable; auto-detected when empty.
    pub mpv_path: String,
    /// Extra mpv arguments, whitespace separated. Your mpv.conf still applies.
    pub extra_args: String,
    /// Quality preset id (see [`QUALITIES`]).
    pub quality: String,
    pub auto_skip_intro: bool,
    pub auto_skip_credits: bool,
    /// mpv key that skips the current intro/credits marker.
    pub skip_key: String,
    /// Apply the audio/subtitle tracks selected in Plex, and save changes made in mpv back to Plex.
    pub sync_streams: bool,
    /// Fraction of the runtime after which an item counts as watched.
    pub watched_threshold: f64,
    /// Autoplay following episodes.
    pub continuous: bool,
    /// Show what's playing in the OS media controls (Now Playing, MPRIS, SMTC).
    pub media_controls: bool,
    /// Show what's playing as Discord Rich Presence.
    pub discord: bool,
    /// Discord application id; its name is what Discord shows ("Watching Patina").
    pub discord_app_id: String,
    /// Where downloads for offline playback go; a default folder when empty.
    pub downloads_dir: String,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self {
            mpv_path: String::new(),
            extra_args: String::new(),
            quality: "original".into(),
            auto_skip_intro: false,
            auto_skip_credits: false,
            skip_key: "TAB".into(),
            sync_streams: true,
            watched_threshold: 0.9,
            continuous: true,
            media_controls: true,
            discord: false,
            discord_app_id: String::new(),
            downloads_dir: String::new(),
        }
    }
}

impl PlayerSettings {
    pub fn mpv_program(&self) -> String {
        if self.mpv_path.trim().is_empty() { patina_mpv::default_program() } else { self.mpv_path.trim().to_string() }
    }

    pub fn quality(&self) -> &'static Quality {
        quality(&self.quality).unwrap_or(&QUALITIES[0])
    }

    pub fn downloads_dir(&self) -> PathBuf {
        let custom = self.downloads_dir.trim();
        if !custom.is_empty() {
            return PathBuf::from(custom);
        }
        directories::UserDirs::new()
            .and_then(|d| d.video_dir().map(|v| v.join("Patina")))
            .unwrap_or_else(|| data_dir().join("downloads"))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    /// Named library queries and home layouts, scoped by account and server in the GUI.
    pub saved_views: Vec<serde_json::Value>,
    pub home_layouts: std::collections::BTreeMap<String, HomeLayout>,
    pub accent: String,
    /// Poster width in px.
    pub poster_size: u32,
    /// Blur thumbnails and summaries of unwatched episodes.
    pub hide_spoilers: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self { accent: "paper".into(), poster_size: 160, hide_spoilers: false, saved_views: Vec::new(), home_layouts: Default::default() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HomeLayout {
    pub order: Vec<String>,
    pub hidden: Vec<String>,
    /// Hubs that are hidden by default (the legacy On Deck) but turned on.
    pub shown: Vec<String>,
    pub compact_continue: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TuiSettings {
    /// Show the inspector pane next to the columns.
    pub show_inspector: bool,
    pub show_watch_status: bool,
    pub show_library_counts: bool,
    pub hide_watched: bool,
    /// Accent colour; empty means the theme's.
    pub accent: String,
}

impl Default for TuiSettings {
    fn default() -> Self {
        Self { show_inspector: true, show_watch_status: true, show_library_counts: true, hide_watched: false, accent: String::new() }
    }
}

pub struct Accent {
    pub id: &'static str,
    pub label: &'static str,
    pub color: &'static str,
}

/// Accents are used sparingly (progress, watched marks, focus), so they are
/// kept muted. Ids are stable; they are stored in the config file.
pub const ACCENTS: &[Accent] = &[
    Accent { id: "paper", label: "Paper", color: "#e6e1d6" },
    Accent { id: "amber", label: "Amber", color: "#d6a44c" },
    Accent { id: "copper", label: "Copper", color: "#c98a5e" },
    Accent { id: "verdigris", label: "Verdigris", color: "#7fb3a0" },
    Accent { id: "sky", label: "Sky", color: "#7ea6cc" },
    Accent { id: "iris", label: "Iris", color: "#9d98d4" },
    Accent { id: "rose", label: "Rose", color: "#cf8a98" },
];

#[derive(Debug, PartialEq)]
pub struct Quality {
    pub id: &'static str,
    pub label: &'static str,
    /// `None` means direct play.
    pub kbps: Option<u32>,
    pub resolution: &'static str,
}

pub const QUALITIES: &[Quality] = &[
    Quality { id: "original", label: "Original (direct play)", kbps: None, resolution: "" },
    Quality { id: "40m-4k", label: "4K · 40 Mbps", kbps: Some(40000), resolution: "3840x2160" },
    Quality { id: "20m-1080", label: "1080p · 20 Mbps", kbps: Some(20000), resolution: "1920x1080" },
    Quality { id: "12m-1080", label: "1080p · 12 Mbps", kbps: Some(12000), resolution: "1920x1080" },
    Quality { id: "8m-1080", label: "1080p · 8 Mbps", kbps: Some(8000), resolution: "1920x1080" },
    Quality { id: "4m-720", label: "720p · 4 Mbps", kbps: Some(4000), resolution: "1280x720" },
    Quality { id: "2m-720", label: "720p · 2 Mbps", kbps: Some(2000), resolution: "1280x720" },
    Quality { id: "1m-480", label: "480p · 1 Mbps", kbps: Some(1000), resolution: "854x480" },
];

/// A quality preset by id or label.
pub fn quality(name: &str) -> Option<&'static Quality> {
    QUALITIES.iter().find(|q| q.id == name || q.label == name)
}

pub fn config_dir() -> PathBuf {
    if cfg!(windows)
        && let Some(dirs) = directories::ProjectDirs::from("", "", "patina") {
            return dirs.config_dir().to_path_buf();
        }
    // Same convention as mpv on macOS/Linux: ~/.config (or $XDG_CONFIG_HOME).
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| directories::BaseDirs::new().map(|b| b.home_dir().join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("patina")
}

pub fn cache_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "patina")
        .map(|d| d.cache_dir().to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("patina"))
}

pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "patina")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| config_dir().join("data"))
}

/// The directory mpv reads its configuration from by default.
pub fn mpv_config_dir() -> PathBuf {
    if cfg!(windows)
        && let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("mpv");
        }
    std::env::var_os("MPV_HOME").map(PathBuf::from).unwrap_or_else(|| {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| directories::BaseDirs::new().map(|b| b.home_dir().join(".config")))
            .unwrap_or_default()
            .join("mpv")
    })
}

fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

impl Settings {
    pub fn load() -> Settings {
        let _lock = crate::storage::lock(&config_file()).ok();
        let (settings, persist) = match Self::read() {
            Some(l) => (l.settings, !l.client_id_saved),
            None => (Settings::default(), !config_file().exists()),
        };
        // Persist so the generated client id stays stable.
        if persist && let Err(e) = settings.save() { tracing::error!("could not save settings: {e:#}"); }
        settings
    }

    /// The file as saved now (another process may have changed it), keeping
    /// this client id if the file has none. `self` when it can't be read.
    pub fn reload(&self) -> Settings {
        match Self::read() {
            Some(mut s) => {
                if !s.client_id_saved {
                    s.settings.client_id = self.client_id.clone();
                }
                s.settings
            }
            None => self.clone(),
        }
    }

    fn read() -> Option<Loaded> {
        let path = config_file();
        let text = std::fs::read_to_string(&path).ok()?;
        let table: toml::Table = match toml::from_str(&text) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("ignoring unreadable {}: {e}", path.display());
                return None;
            }
        };
        let client_id_saved = table.contains_key("client_id");
        match table.try_into() {
            Ok(settings) => Some(Loaded { settings, client_id_saved }),
            Err(e) => {
                tracing::warn!("ignoring unreadable {}: {e}", path.display());
                None
            }
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        crate::storage::write(&config_file(), toml::to_string_pretty(self)?.as_bytes())
    }

    /// Hold an OS lock across the entire read–modify–write transaction.
    pub fn update(&self, f: impl FnOnce(&mut Self) -> anyhow::Result<()>) -> anyhow::Result<Self> {
        let _lock = crate::storage::lock(&config_file())?;
        // Never overwrite an existing unreadable file with defaults.
        let mut latest = if config_file().exists() {
            Self::read().ok_or_else(|| anyhow::anyhow!("Settings could not be read; the existing file was preserved."))?.settings
        } else { self.clone() };
        f(&mut latest)?;
        latest.save()?;
        Ok(latest)
    }

}

struct Loaded {
    settings: Settings,
    client_id_saved: bool,
}

/// Apply only fields actually changed by the front end, preserving other writers' fields.
pub fn patch<T: Serialize + serde::de::DeserializeOwned>(value: &mut T, patch: serde_json::Value) -> anyhow::Result<()> {
    let mut json = serde_json::to_value(&*value)?;
    let fields = patch.as_object().ok_or_else(|| anyhow::anyhow!("Settings patch must be an object"))?;
    let object = json.as_object_mut().ok_or_else(|| anyhow::anyhow!("Settings must be an object"))?;
    for (key, value) in fields {
        anyhow::ensure!(object.contains_key(key), "Unknown setting: {key}");
        object.insert(key.clone(), value.clone());
    }
    *value = serde_json::from_value(json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_settings_and_saved_views_round_trip() {
        let mut settings: Settings = toml::from_str("[ui]\nposter_size = 180\n").unwrap();
        assert!(settings.ui.saved_views.is_empty());
        settings.ui.saved_views.push(serde_json::json!({
            "id": "view-1", "name": "Unwatched drama", "scope": "alice:home", "library": "1",
            "mediaType": 0, "sort": "titleSort", "desc": false, "unwatched": true,
            "toggles": [], "picked": [{"filter": "genre", "title": "Genre", "value": "10", "label": "Drama"}]
        }));
        settings.ui.home_layouts.insert("alice:home".into(), HomeLayout { compact_continue: true, ..Default::default() });
        let text = toml::to_string_pretty(&settings).unwrap();
        assert_eq!(settings, toml::from_str::<Settings>(&text).unwrap());
    }
    #[test]
    fn partial_settings_update_preserves_other_fields() {
        let mut player = PlayerSettings { quality: "4m-720".into(), ..Default::default() };
        patch(&mut player, serde_json::json!({"continuous": false})).unwrap();
        assert_eq!(player.quality, "4m-720");
        assert!(!player.continuous);
        assert!(patch(&mut player, serde_json::json!({"continuous": "wrong"})).is_err());
        assert!(!player.continuous);
    }
}
