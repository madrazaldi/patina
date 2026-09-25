//! Durable downloads shared by both front ends. A process lease owns the worker;
//! manifest transactions coordinate pause, remove and progress across processes.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, ensure};
use futures::StreamExt;
use patina_plex::{Metadata, MetadataOptions, PlexServer};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::state::{AppState, CoreEvent};
use crate::{config, storage};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    Queued,
    Downloading,
    Paused,
    Done,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    pub id: String,
    pub server_id: String,
    pub server_name: String,
    pub item: Metadata,
    pub file: Option<String>,
    pub poster: Option<String>,
    pub size: i64,
    pub received: i64,
    pub state: State,
    pub error: Option<String>,
    pub added_at: i64,
    /// Account that queued this file. Legacy downloads remain playable but never sync to an inferred user.
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub validator: Option<String>,
    #[serde(default)]
    pub source_key: Option<String>,
}

static RUNNING: AtomicBool = AtomicBool::new(false);
fn manifest() -> PathBuf {
    config::config_dir().join("downloads.json")
}
fn artwork_dir() -> PathBuf {
    config::data_dir().join("artwork")
}
fn update<R>(f: impl FnOnce(&mut Vec<Download>) -> R) -> Result<R> {
    storage::update_json(&manifest(), f)
}

pub fn list() -> Vec<Download> {
    storage::read_json(&manifest()).unwrap_or_else(|e| {
        tracing::error!("downloads: {e:#}");
        Vec::new()
    })
}
fn entry(id: &str) -> Option<Download> {
    list().into_iter().find(|d| d.id == id)
}
fn emit(app: &AppState) {
    app.emit(CoreEvent::Downloads(list()));
}
fn change(app: &AppState, f: impl FnOnce(&mut Vec<Download>)) {
    if let Err(e) = update(f) {
        app.error(format!("Could not save downloads: {e:#}"));
    }
    emit(app);
}
pub fn poster(id: &str) -> Option<PathBuf> {
    entry(id)?.poster.map(PathBuf::from)
}

pub async fn enqueue(app: &AppState, rating_key: &str) -> Result<usize> {
    let session = app.session().ok_or_else(|| anyhow!("Not connected."))?;
    let server = &session.server;
    let item = server.metadata(rating_key, MetadataOptions::default()).await?.ok_or_else(|| anyhow!("Item not found."))?;
    let items = if item.is_playable() { vec![item] } else { server.all_leaves(rating_key).await?.metadata };
    let account = crate::offline::account(&session);
    let added = update(|list| {
        let mut added = 0;
        for item in items.into_iter().filter(|m| m.is_playable() && !m.media.is_empty()) {
            let id = format!("{}:{}:{}", session.server_id, account, item.rating_key());
            if list.iter().any(|d| d.id == id) {
                continue;
            }
            list.push(Download {
                id,
                server_id: session.server_id.clone(),
                server_name: session.server_name.clone(),
                size: item.media.first().and_then(|m| m.parts.first()).and_then(|p| p.size).unwrap_or(0),
                item,
                file: None,
                poster: None,
                received: 0,
                state: State::Queued,
                error: None,
                added_at: crate::offline::now(),
                account: account.clone(),
                validator: None,
                source_key: None,
            });
            added += 1;
        }
        added
    })?;
    emit(app);
    resume(app);
    Ok(added)
}

pub fn remove(app: &AppState, id: &str) {
    let result = update(|list| list.iter().position(|d| d.id == id).map(|i| list.remove(i)));
    match result {
        Ok(Some(d)) => cleanup(&d),
        Ok(None) => {}
        Err(e) => app.error(format!("Could not remove download: {e:#}")),
    }
    emit(app);
}
fn cleanup(d: &Download) {
    for f in [d.file.as_deref(), d.poster.as_deref()].into_iter().flatten() {
        let _ = std::fs::remove_file(f);
        let _ = std::fs::remove_file(format!("{f}.part"));
    }
}
pub fn pause(app: &AppState, id: &str) {
    change(app, |list| {
        if let Some(d) = list.iter_mut().find(|d| d.id == id && matches!(d.state, State::Queued | State::Downloading)) {
            d.state = State::Paused;
        }
    });
}
pub fn retry(app: &AppState, id: &str) {
    change(app, |list| {
        if let Some(d) = list.iter_mut().find(|d| d.id == id && matches!(d.state, State::Failed | State::Paused)) {
            d.state = State::Queued;
            d.error = None;
        }
    });
    resume(app);
}

/// Preserve server/account identity all the way into offline playback.
pub fn files(ids: &[String]) -> Vec<Download> {
    let list = list();
    ids.iter()
        .filter_map(|id| list.iter().find(|d| &d.id == id && d.state == State::Done))
        .filter(|d| d.file.as_ref().is_some_and(|p| Path::new(p).exists()))
        .cloned()
        .collect()
}

pub fn resume(app: &AppState) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    app.clone().spawn(async move {
        if let Err(e) = worker(&app).await {
            app.error(format!("Downloads: {e:#}"));
        }
        RUNNING.store(false, Ordering::SeqCst);
    });
}

async fn worker(app: &AppState) -> Result<()> {
    // This lease is held across awaits; another GUI/TUI process cannot start a second writer.
    let Some(_lease) = storage::try_lock(&config::config_dir().join("download-worker"))? else { return Ok(()) };
    // With the lease held, any downloading entries were interrupted by a previous process.
    update(|list| {
        for d in list {
            if d.state == State::Downloading {
                d.state = State::Queued;
            }
        }
    })?;
    loop {
        let Some(session) = app.session() else { break };
        let account = crate::offline::account(&session);
        let next = update(|list| {
            let d = list
                .iter_mut()
                .find(|d| d.state == State::Queued && d.server_id == session.server_id && (d.account.is_empty() || d.account == account))?;
            d.state = State::Downloading;
            d.error = None;
            Some(d.clone())
        })?;
        let Some(d) = next else { break };
        emit(app);
        let mut result = Ok(false);
        for attempt in 0..3 {
            result = fetch(app, &session.server, &d.id).await;
            if result.is_ok() || !is_active(&d.id) || !result.as_ref().err().is_some_and(retryable) {
                break;
            }
            if attempt < 2 {
                tokio::time::sleep(Duration::from_secs(1 << attempt)).await;
            }
        }
        update(|list| {
            if let Some(current) = list.iter_mut().find(|x| x.id == d.id && x.state == State::Downloading) {
                match &result {
                    Ok(true) => {
                        current.state = State::Done;
                        current.error = None;
                    }
                    Ok(false) => current.state = State::Paused,
                    Err(e) => {
                        current.state = State::Failed;
                        current.error = Some(format!("{e:#}"));
                    }
                }
            }
        })?;
        if entry(&d.id).is_none() {
            cleanup(&d);
        }
        emit(app);
    }
    Ok(())
}
fn retryable(error: &anyhow::Error) -> bool {
    error.chain().any(|e| e.downcast_ref::<reqwest::Error>().is_some_and(|e| e.is_timeout() || e.is_connect() || e.is_body()))
        || error.chain().any(|e| {
            matches!(e.downcast_ref::<patina_plex::Error>(), Some(patina_plex::Error::Status { status: 429 | 500 | 502 | 503 | 504, .. }))
        })
}
fn is_active(id: &str) -> bool {
    entry(id).is_some_and(|d| d.state == State::Downloading)
}

fn clean(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { ' ' } else { c })
        .collect();
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    let s = s.trim_matches('.').to_string();
    if s.is_empty() { "Untitled".into() } else { s }
}

fn target(dir: &Path, item: &Metadata, ext: &str) -> PathBuf {
    let (folder, name) = match item.kind() {
        "episode" => (
            dir.join(clean(item.grandparent_title.as_deref().unwrap_or("Shows")))
                .join(clean(item.parent_title.as_deref().unwrap_or("Season"))),
            format!("{} {}", item.episode_code(), item.title()),
        ),
        "track" => (
            dir.join(clean(item.grandparent_title.as_deref().unwrap_or("Music")))
                .join(clean(item.parent_title.as_deref().unwrap_or("Album"))),
            format!("{:02} {}", item.index.unwrap_or(0), item.title()),
        ),
        _ => (dir.to_path_buf(), item.full_title()),
    };
    folder.join(format!("{} [{}].{ext}", clean(&name), clean(item.rating_key())))
}

/// Validate a resumed response before appending even one byte.
fn response_offset(status: u16, range: Option<&str>, requested: u64) -> Result<(u64, Option<u64>)> {
    if status == 200 {
        return Ok((0, None));
    }
    ensure!(status == 206, "Unexpected download response: {status}");
    let range = range.and_then(|s| s.strip_prefix("bytes ")).ok_or_else(|| anyhow!("Missing Content-Range"))?;
    let (span, total) = range.split_once('/').ok_or_else(|| anyhow!("Invalid Content-Range"))?;
    let (start, end) = span.split_once('-').ok_or_else(|| anyhow!("Invalid byte range"))?;
    let (start, end, total) = (start.parse::<u64>()?, end.parse::<u64>()?, total.parse::<u64>()?);
    ensure!(start == requested && end >= start && end < total, "Server returned the wrong byte range");
    Ok((start, Some(total)))
}

/// Only a strong ETag or modification date can identify bytes in a saved partial file.
fn response_validator(resp: &reqwest::Response) -> Option<String> {
    resp.headers().get("ETag").and_then(|v| v.to_str().ok()).filter(|v| !v.starts_with("W/"))
        .or_else(|| resp.headers().get("Last-Modified").and_then(|v| v.to_str().ok()))
        .map(str::to_string)
}

async fn fetch(app: &AppState, server: &PlexServer, id: &str) -> Result<bool> {
    let Some(d) = entry(id) else { return Ok(false) };
    let item = server
        .metadata(d.item.rating_key(), MetadataOptions::default())
        .await?
        .ok_or_else(|| anyhow!("The item no longer exists on the server."))?;
    let part = item.media.first().and_then(|m| m.parts.first()).ok_or_else(|| anyhow!("No file to download."))?;
    let key = part.key.as_deref().ok_or_else(|| anyhow!("No file to download."))?;
    let ext = part.container.as_deref().filter(|ext| ext.len() <= 12 && ext.chars().all(|c| c.is_ascii_alphanumeric())).unwrap_or("mkv");
    // Account/server ids separate files with identical titles and versions.
    let scope = crate::offline::digest(&format!("{}:{}", d.server_id, d.account));
    let dir = app.settings().player.downloads_dir().join(format!("{} - {}", clean(&d.server_name), &scope[..8]));
    let path = d.file.as_ref().map(PathBuf::from).unwrap_or_else(|| target(&dir, &item, ext));
    let parent = path.parent().ok_or_else(|| anyhow!("No download directory"))?;
    tokio::fs::create_dir_all(parent).await?;
    let partial = PathBuf::from(format!("{}.part", path.display()));
    let existing = tokio::fs::metadata(&partial).await.map(|m| m.len()).unwrap_or(0);
    // Without a validator a partial file cannot safely be matched to a changed source.
    let mut requested = if d.source_key.as_deref() == Some(key) && d.validator.is_some() { existing } else { 0 };
    let mut resp = match server.download_range(key, requested, d.validator.as_deref()).await {
        Err(patina_plex::Error::Status { status: 416, .. }) => {
            requested = 0;
            server.download_range(key, 0, None).await?
        }
        other => other?,
    };
    // A correctly aligned range alone cannot prove that the source is unchanged.
    if requested > 0 && resp.status().as_u16() == 206 && response_validator(&resp) != d.validator {
        drop(resp);
        requested = 0;
        resp = server.download_range(key, 0, None).await?;
    }
    let (offset, range_total) =
        response_offset(resp.status().as_u16(), resp.headers().get("Content-Range").and_then(|v| v.to_str().ok()), requested)?;
    let total = range_total.or_else(|| resp.content_length().map(|n| offset + n)).or(part.size.and_then(|n| u64::try_from(n).ok()));
    let validator = response_validator(&resp);
    if let Some(total) = total {
        ensure!(fs2::available_space(parent)? >= total.saturating_sub(offset), "Not enough free disk space for this download.");
    }
    let thumb = item.thumb.as_ref().or(item.parent_thumb.as_ref()).or(item.grandparent_thumb.as_ref());
    let mut poster = d.poster.clone();
    if poster.is_none()
        && let Some(thumb) = thumb
        && let Some(cached) = crate::images::cached_file(thumb, 480, 720).await
    {
        let art = artwork_dir();
        tokio::fs::create_dir_all(&art).await?;
        let dest = art.join(format!("{}.img", crate::offline::digest(id)));
        if tokio::fs::copy(cached, &dest).await.is_ok() {
            poster = Some(dest.display().to_string());
        }
    }
    update(|list| {
        if let Some(d) = list.iter_mut().find(|d| d.id == id) {
            d.item = item.clone();
            d.file = Some(path.display().to_string());
            d.poster = poster;
            d.size = total.unwrap_or(0) as i64;
            d.received = offset as i64;
            d.validator = validator;
            d.source_key = Some(key.to_string());
        }
    })?;
    if !is_active(id) {
        return Ok(false);
    }
    let mut file = tokio::fs::OpenOptions::new().create(true).write(true).truncate(offset == 0).append(offset > 0).open(&partial).await?;
    let mut stream = resp.bytes_stream();
    let mut received = offset;
    let mut last = Instant::now();
    let mut last_chunk = Instant::now();
    let mut last_control = Instant::now() - Duration::from_secs(1);
    loop {
        ensure!(last_chunk.elapsed() < Duration::from_secs(60), "The download stalled. Resume to try again.");
        let check_control = last_control.elapsed() >= Duration::from_millis(250);
        if check_control {
            last_control = Instant::now();
        }
        if check_control && !is_active(id) {
            file.flush().await?;
            drop(file);
            if entry(id).is_none() {
                let _ = tokio::fs::remove_file(&partial).await;
                let _ = tokio::fs::remove_file(&path).await;
            }
            return Ok(false);
        }
        let chunk = tokio::select! {
            chunk = stream.next() => chunk,
            _ = tokio::time::sleep(Duration::from_millis(250)) => continue,
        };
        let Some(chunk) = chunk else { break };
        let chunk = chunk.context("downloading")?;
        last_chunk = Instant::now();
        file.write_all(&chunk).await.context("writing the file")?;
        received += chunk.len() as u64;
        if last.elapsed() >= Duration::from_millis(500) {
            last = Instant::now();
            update(|list| {
                if let Some(d) = list.iter_mut().find(|d| d.id == id) {
                    d.received = received as i64;
                }
            })?;
            emit(app);
        }
    }
    ensure!(total.is_none_or(|n| n == received), "Download ended before the complete file arrived.");
    file.flush().await?;
    file.sync_all().await?;
    drop(file);
    let _lock = storage::lock(&manifest())?;
    if !is_active(id) {
        return Ok(false);
    }
    tokio::fs::rename(&partial, &path).await?;
    let mut list: Vec<Download> = storage::read_json(&manifest())?;
    if let Some(d) = list.iter_mut().find(|d| d.id == id) {
        d.received = received as i64;
        d.size = received as i64;
        d.state = State::Done;
    }
    storage::write(&manifest(), &serde_json::to_vec_pretty(&list)?)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranges_never_append_a_full_or_misaligned_response() {
        assert_eq!(response_offset(200, None, 500).unwrap(), (0, None));
        assert_eq!(response_offset(206, Some("bytes 500-999/1000"), 500).unwrap(), (500, Some(1000)));
        assert!(response_offset(206, Some("bytes 0-499/1000"), 500).is_err());
        assert!(response_offset(206, None, 500).is_err());
        assert!(response_offset(206, Some("bytes 500-1000/1000"), 500).is_err());
    }
}
