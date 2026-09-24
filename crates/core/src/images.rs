//! Artwork: images are resolved through the current server's photo
//! transcoder (so front ends never see the token) and the resized result is
//! cached on disk.

use std::path::PathBuf;
use std::sync::{LazyLock, RwLock};

use patina_plex::PlexServer;
use sha2::{Digest, Sha256};

static SERVER: RwLock<Option<PlexServer>> = RwLock::new(None);
static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().unwrap_or_default()
});
static CACHE: LazyLock<PathBuf> = LazyLock::new(|| {
    let dir = crate::config::cache_dir().join("images");
    let _ = std::fs::create_dir_all(&dir);
    dir
});

/// The server images are fetched through; `None` when signed out.
pub fn set_server(server: Option<PlexServer>) {
    *SERVER.write().unwrap() = server;
}

/// Saved artwork of a download and its content type; works offline.
pub fn download_poster(id: &str) -> Option<(Vec<u8>, &'static str)> {
    let path = crate::downloads::poster(id)?;
    let bytes = std::fs::read(&path).ok()?;
    Some((bytes, sniff(&path)))
}

fn cache_path(server: Option<&PlexServer>, source: &str, w: u32, h: u32) -> PathBuf {
    let scope = server.and_then(|s| s.machine_identifier().map(str::to_string)).unwrap_or_default();
    let key = hex(&Sha256::digest(format!("{scope}|{source}|{w}|{h}").as_bytes()));
    CACHE.join(&key[..2]).join(&key)
}

/// A resized image as a file on disk, for things outside the front end
/// (OS media controls, downloads).
pub async fn cached_file(source: &str, w: u32, h: u32) -> Option<PathBuf> {
    fetch(source, w, h).await?;
    let server = SERVER.read().unwrap().clone();
    Some(cache_path(server.as_ref(), source, w, h))
}

/// A server path or absolute URL resized to fit `w`×`h`, with its content type.
pub async fn fetch(source: &str, w: u32, h: u32) -> Option<(Vec<u8>, String)> {
    if source.is_empty() {
        return None;
    }
    let server = SERVER.read().unwrap().clone();
    let path = cache_path(server.as_ref(), source, w, h);
    if let Ok(bytes) = tokio::fs::read(&path).await {
        return Some((bytes, sniff(&path).into()));
    }

    let is_absolute = source.starts_with("http://") || source.starts_with("https://");
    let result = match (&server, is_absolute) {
        (Some(server), false) => server.image(source, w, h).await.ok().map(|(b, c)| (b, c.unwrap_or_default())),
        (Some(server), true) if source.contains("plex.tv") || source.contains("plex.direct") => {
            // Let the server resize external Plex artwork too, falling back to a direct fetch.
            match server.image(source, w, h).await {
                Ok((b, c)) => Some((b, c.unwrap_or_default())),
                Err(_) => direct(source).await,
            }
        }
        (_, true) => direct(source).await,
        (None, false) => None,
    }?;
    let (bytes, ctype) = result;
    if bytes.is_empty() {
        return None;
    }
    let ctype = if ctype.starts_with("image/") { ctype } else { "image/jpeg".to_string() };
    if let Some(dir) = path.parent() {
        let _ = tokio::fs::create_dir_all(dir).await;
    }
    // The content type is re-sniffed from the bytes on cache hits.
    let _ = tokio::fs::write(&path, &bytes).await;
    Some((bytes, ctype))
}

async fn direct(url: &str) -> Option<(Vec<u8>, String)> {
    let resp = HTTP.get(url).send().await.ok()?.error_for_status().ok()?;
    let ctype = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    Some((resp.bytes().await.ok()?.to_vec(), ctype))
}

/// The content type of an image file, from its first bytes.
pub fn sniff(path: &std::path::Path) -> &'static str {
    let mut head = [0u8; 8];
    if let Ok(mut f) = std::fs::File::open(path) {
        use std::io::Read;
        let _ = f.read(&mut head);
    }
    match head {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [b'R', b'I', b'F', b'F', ..] => "image/webp",
        [b'G', b'I', b'F', ..] => "image/gif",
        _ => "image/jpeg",
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Drop cached images not used for 30 days.
pub fn prune_cache() {
    let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(60 * 60 * 24 * 30);
    let Ok(dirs) = std::fs::read_dir(&*CACHE) else { return };
    for dir in dirs.flatten() {
        let Ok(files) = std::fs::read_dir(dir.path()) else { continue };
        for f in files.flatten() {
            let stale = f.metadata().and_then(|m| m.accessed().or_else(|_| m.modified())).map(|t| t < cutoff).unwrap_or(false);
            if stale {
                let _ = std::fs::remove_file(f.path());
            }
        }
    }
}
