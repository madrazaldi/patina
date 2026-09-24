//! Downloads for offline playback. Original files go to the downloads folder
//! (Settings → Player), their artwork to the data dir, and a manifest
//! (`downloads.json` next to the config) remembers them. One download runs
//! at a time; progress is pushed to the front end as `Downloads` events.
//!
//! Another Patina process may share the manifest, so it is re-read before
//! each write: entries this process changed win, the rest come from disk.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use futures::StreamExt;
use patina_plex::{Metadata, MetadataOptions, Params};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::config;
use crate::state::{AppState, CoreEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    Queued,
    Downloading,
    Done,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Download {
    /// `<server id>:<rating key>`.
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
}

static STORE: Mutex<Option<Vec<Download>>> = Mutex::new(None);
static RUNNING: AtomicBool = AtomicBool::new(false);
static CANCELLED: Mutex<Option<HashSet<String>>> = Mutex::new(None);
/// Ids this process added, changed or removed.
static OWNED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

fn manifest() -> PathBuf {
    config::config_dir().join("downloads.json")
}

fn artwork_dir() -> PathBuf {
    config::data_dir().join("artwork")
}

fn read_manifest() -> Option<Vec<Download>> {
    std::fs::read(manifest()).ok().and_then(|b| serde_json::from_slice(&b).ok())
}

fn own(id: &str) {
    OWNED.lock().unwrap().get_or_insert_with(HashSet::new).insert(id.to_string());
}

fn with<T>(f: impl FnOnce(&mut Vec<Download>) -> T) -> T {
    let mut store = STORE.lock().unwrap();
    let list = store.get_or_insert_with(|| {
        let mut list = read_manifest().unwrap_or_default();
        // Anything interrupted by quitting starts again.
        for d in &mut list {
            if d.state == State::Downloading {
                d.state = State::Queued;
                d.received = 0;
            }
        }
        list
    });
    f(list)
}

fn save() {
    let disk = read_manifest();
    let list = with(|list| {
        if let Some(disk) = disk {
            *list = merge(list, disk, OWNED.lock().unwrap().as_ref());
        }
        list.clone()
    });
    let path = manifest();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_vec_pretty(&list) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(tmp, path);
        }
    }
}

/// The manifest on disk with this process's own entries (or their removal) applied.
fn merge(mine: &[Download], disk: Vec<Download>, owned: Option<&HashSet<String>>) -> Vec<Download> {
    let owned = |id: &str| owned.is_some_and(|o| o.contains(id));
    let mut out: Vec<Download> = disk
        .into_iter()
        .filter_map(|d| if owned(&d.id) { mine.iter().find(|m| m.id == d.id).cloned() } else { Some(d) })
        .collect();
    for m in mine.iter().filter(|m| owned(&m.id)) {
        if !out.iter().any(|d| d.id == m.id) {
            out.push(m.clone());
        }
    }
    out
}

pub fn list() -> Vec<Download> {
    with(|l| l.clone())
}

fn emit(app: &AppState) {
    app.emit(CoreEvent::Downloads(list()));
}

/// Saved artwork of a download, for the image protocol.
pub fn poster(id: &str) -> Option<PathBuf> {
    with(|l| l.iter().find(|d| d.id == id).and_then(|d| d.poster.clone())).map(PathBuf::from)
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Queue an item, or every episode/track below it. Returns how many were added.
pub async fn enqueue(app: &AppState, rating_key: &str) -> Result<usize> {
    let server = app.server()?;
    let session = app.session().ok_or_else(|| anyhow!("Not connected."))?;
    let item = server.metadata(rating_key, MetadataOptions::default()).await?.ok_or_else(|| anyhow!("Item not found."))?;
    let items = if item.is_playable() { vec![item] } else { server.all_leaves(rating_key).await?.metadata };
    let added = with(|list| {
        let mut added = 0;
        for item in items.into_iter().filter(|m| m.is_playable() && !m.media.is_empty()) {
            let id = format!("{}:{}", session.server_id, item.rating_key());
            if list.iter().any(|d| d.id == id) {
                continue;
            }
            own(&id);
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
                added_at: now(),
            });
            added += 1;
        }
        added
    });
    save();
    emit(app);
    kick(app.clone());
    Ok(added)
}

/// Delete a download (and its file), cancelling it if it is running.
pub fn remove(app: &AppState, id: &str) {
    let removed = with(|list| {
        let i = list.iter().position(|d| d.id == id)?;
        Some(list.remove(i))
    });
    own(id);
    if let Some(d) = removed {
        if d.state == State::Downloading {
            CANCELLED.lock().unwrap().get_or_insert_with(HashSet::new).insert(d.id.clone());
        }
        for f in [d.file.as_deref(), d.poster.as_deref()].into_iter().flatten() {
            let _ = std::fs::remove_file(f);
            let _ = std::fs::remove_file(format!("{f}.part"));
        }
    }
    save();
    emit(app);
}

/// Try failed downloads again.
pub fn retry(app: &AppState, id: &str) {
    with(|list| {
        if let Some(d) = list.iter_mut().find(|d| d.id == id && d.state == State::Failed) {
            own(id);
            d.state = State::Queued;
            d.error = None;
            d.received = 0;
        }
    });
    save();
    emit(app);
    kick(app.clone());
}

/// Downloaded files, in the order asked for.
pub fn files(ids: &[String]) -> Vec<(Metadata, PathBuf)> {
    with(|list| {
        ids.iter()
            .filter_map(|id| list.iter().find(|d| &d.id == id && d.state == State::Done))
            .filter_map(|d| Some((d.item.clone(), PathBuf::from(d.file.as_ref()?))))
            .filter(|(_, p)| p.exists())
            .collect()
    })
}

/// Resume pending downloads (on startup, once connected).
pub fn resume(app: &AppState) {
    if with(|l| l.iter().any(|d| d.state == State::Queued)) {
        kick(app.clone());
    }
}

fn kick(app: AppState) {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    app.clone().spawn(async move {
        loop {
            let server_id = app.session().map(|s| s.server_id);
            let next = with(|l| {
                l.iter().find(|d| d.state == State::Queued && Some(&d.server_id) == server_id.as_ref()).map(|d| d.id.clone())
            });
            let Some(id) = next else { break };
            set(&id, |d| d.state = State::Downloading);
            emit(&app);
            let result = fetch(&app, &id).await;
            let cancelled = CANCELLED.lock().unwrap().as_mut().is_some_and(|c| c.remove(&id));
            if !cancelled {
                match result {
                    Ok(()) => set(&id, |d| d.state = State::Done),
                    Err(e) => set(&id, |d| {
                        d.state = State::Failed;
                        d.error = Some(format!("{e:#}"));
                    }),
                }
            }
            save();
            emit(&app);
        }
        RUNNING.store(false, Ordering::SeqCst);
    });
}

fn set(id: &str, f: impl FnOnce(&mut Download)) {
    with(|l| {
        if let Some(d) = l.iter_mut().find(|d| d.id == id) {
            f(d);
        }
    });
    own(id);
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
    folder.join(format!("{}.{ext}", clean(&name)))
}

async fn fetch(app: &AppState, id: &str) -> Result<()> {
    let server = app.server()?;
    let Some(d) = with(|l| l.iter().find(|d| d.id == id).cloned()) else { return Ok(()) };
    let item = server
        .metadata(d.item.rating_key(), MetadataOptions::default())
        .await?
        .ok_or_else(|| anyhow!("The item no longer exists on the server."))?;
    let part = item.media.first().and_then(|m| m.parts.first()).ok_or_else(|| anyhow!("No file to download."))?;
    let key = part.key.clone().ok_or_else(|| anyhow!("No file to download."))?;
    let ext = part
        .container
        .clone()
        .or_else(|| part.file.as_deref().and_then(|f| Path::new(f).extension()).map(|e| e.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "mkv".into());
    let dir = app.settings().player.downloads_dir();
    let path = target(&dir, &item, &ext);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.with_context(|| format!("creating {}", parent.display()))?;
    }

    // Artwork first, so the list looks right while the file downloads.
    let thumb = match item.kind() {
        "episode" => item.thumb.clone().or(item.grandparent_thumb.clone()),
        "track" => item.parent_thumb.clone().or(item.thumb.clone()),
        _ => item.thumb.clone(),
    };
    if let Some(thumb) = thumb
        && let Some(cached) = crate::images::cached_file(&thumb, 480, 720).await
    {
        let art = artwork_dir();
        let _ = tokio::fs::create_dir_all(&art).await;
        let dest = art.join(format!("{}.img", clean(id)));
        if tokio::fs::copy(&cached, &dest).await.is_ok() {
            set(id, |d| d.poster = Some(dest.display().to_string()));
        }
    }
    set(id, |d| {
        d.item = item.clone();
        d.file = Some(path.display().to_string());
    });
    emit(app);

    let resp = server.download(&key, &Params::new().set("download", 1)).await?;
    let total = resp.content_length().map(|n| n as i64).or(part.size).unwrap_or(0);
    set(id, |d| d.size = total);
    let partial = PathBuf::from(format!("{}.part", path.display()));
    let mut file = tokio::fs::File::create(&partial).await.with_context(|| format!("creating {}", partial.display()))?;
    let mut stream = resp.bytes_stream();
    let mut received: i64 = 0;
    let mut last = Instant::now();
    while let Some(chunk) = stream.next().await {
        if CANCELLED.lock().unwrap().as_ref().is_some_and(|c| c.contains(id)) {
            drop(file);
            let _ = tokio::fs::remove_file(&partial).await;
            return Ok(());
        }
        let chunk = chunk.context("downloading")?;
        file.write_all(&chunk).await.context("writing the file")?;
        received += chunk.len() as i64;
        if last.elapsed().as_millis() >= 400 {
            last = Instant::now();
            set(id, |d| d.received = received);
            emit(app);
        }
    }
    file.flush().await?;
    drop(file);
    tokio::fs::rename(&partial, &path).await?;
    set(id, |d| {
        d.received = received;
        d.size = d.size.max(received);
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn download(id: &str, state: State) -> Download {
        Download {
            id: id.into(),
            server_id: String::new(),
            server_name: String::new(),
            item: Metadata::default(),
            file: None,
            poster: None,
            size: 0,
            received: 0,
            state,
            error: None,
            added_at: 0,
        }
    }

    #[test]
    fn merges_with_other_process() {
        let mine = [download("a", State::Downloading), download("c", State::Queued), download("d", State::Queued)];
        let disk = vec![download("a", State::Queued), download("b", State::Done), download("d", State::Done), download("e", State::Done)];
        // Owned: a (changed), c (added), e (removed here). d is someone else's.
        let owned: HashSet<String> = ["a", "c", "e"].into_iter().map(String::from).collect();
        let out = merge(&mine, disk, Some(&owned));
        let got: Vec<(&str, State)> = out.iter().map(|d| (d.id.as_str(), d.state)).collect();
        assert_eq!(got, [("a", State::Downloading), ("b", State::Done), ("d", State::Done), ("c", State::Queued)]);
    }
}
