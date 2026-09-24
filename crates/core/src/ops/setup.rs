//! About the install: versions, paths, choices and the mpv check.

use anyhow::{Result, anyhow};
use serde::Serialize;

use crate::config;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub id: &'static str,
    pub label: &'static str,
    pub color: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub version: &'static str,
    pub config_path: String,
    pub mpv_config_dir: String,
    pub mpv_conf_exists: bool,
    pub qualities: Vec<Choice>,
    pub accents: Vec<Choice>,
    /// Debug builds: `PATINA_ROUTE=/item/100` opens a page on startup.
    pub debug_route: Option<String>,
    /// Debug builds: `PATINA_PLAY=<ratingKey>` starts playback on startup.
    pub debug_play: Option<String>,
}

pub fn info() -> Info {
    let mpv_dir = config::mpv_config_dir();
    let debug = |k: &str| if cfg!(debug_assertions) { std::env::var(k).ok() } else { None };
    Info {
        version: config::VERSION,
        config_path: config::config_dir().join("config.toml").display().to_string(),
        mpv_config_dir: mpv_dir.display().to_string(),
        mpv_conf_exists: mpv_dir.join("mpv.conf").exists(),
        qualities: config::QUALITIES.iter().map(|q| Choice { id: q.id, label: q.label, color: None }).collect(),
        accents: config::ACCENTS.iter().map(|a| Choice { id: a.id, label: a.label, color: Some(a.color) }).collect(),
        debug_route: debug("PATINA_ROUTE"),
        debug_play: debug("PATINA_PLAY"),
    }
}

/// "mpv v0.41.0 — mpv" for the configured (or auto-detected) mpv.
pub async fn probe_mpv(app: &AppState) -> Result<String> {
    let program = app.settings().player.mpv_program();
    let version = patina_mpv::probe(&program).await.map_err(|e| anyhow!("Could not run {program}: {e}"))?;
    // "mpv v0.41.0 Copyright © …" → "mpv v0.41.0"
    let short: String = version.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
    Ok(format!("{short} — {program}"))
}
