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
    pub ui: UiSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            client_id: uuid::Uuid::new_v4().to_string(),
            auth: AuthSettings::default(),
            server: ServerSettings::default(),
            player: PlayerSettings::default(),
            ui: UiSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthSettings {
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
        }
    }
}

impl PlayerSettings {
    pub fn mpv_program(&self) -> String {
        if self.mpv_path.trim().is_empty() { patina_mpv::default_program() } else { self.mpv_path.trim().to_string() }
    }

    pub fn quality(&self) -> &'static Quality {
        QUALITIES.iter().find(|q| q.id == self.quality).unwrap_or(&QUALITIES[0])
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiSettings {
    pub accent: String,
    /// Poster width in px.
    pub poster_size: u32,
    /// Blur thumbnails and summaries of unwatched episodes.
    pub hide_spoilers: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self { accent: "paper".into(), poster_size: 160, hide_spoilers: false }
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
        let path = config_file();
        let settings = match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
                tracing::warn!("ignoring unreadable {}: {e}", path.display());
                Settings::default()
            }),
            Err(_) => Settings::default(),
        };
        // Persist so the generated client id stays stable.
        if !path.exists() {
            settings.save();
        }
        settings
    }

    pub fn save(&self) {
        let path = config_file();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match toml::to_string_pretty(self) {
            Ok(text) => {
                if let Err(e) = write_private(&path, &text) {
                    tracing::error!("could not save settings: {e}");
                }
            }
            Err(e) => tracing::error!("could not encode settings: {e}"),
        }
    }
}

/// The file holds the auth token, so keep it readable only by the user.
fn write_private(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(tmp, path)
}
