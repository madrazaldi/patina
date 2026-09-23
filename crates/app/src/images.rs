//! Artwork proxy. The webview loads `plximg://` URLs; we resolve them through
//! the current server's photo transcoder (so the token never reaches the DOM)
//! and cache the resized result on disk.

use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::{LazyLock, RwLock};

use dioxus::desktop::wry::http::{Request, Response};
use dioxus::desktop::wry::RequestAsyncResponder;
use patina_plex::PlexServer;
use sha2::{Digest, Sha256};

pub const SCHEME: &str = "plximg";

static SERVER: RwLock<Option<PlexServer>> = RwLock::new(None);
static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().unwrap_or_default()
});
static CACHE: LazyLock<PathBuf> = LazyLock::new(|| {
    let dir = crate::config::cache_dir().join("images");
    let _ = std::fs::create_dir_all(&dir);
    dir
});

pub fn set_server(server: Option<PlexServer>) {
    *SERVER.write().unwrap() = server;
}

fn base() -> &'static str {
    if cfg!(any(windows, target_os = "android")) { "http://plximg.localhost/" } else { "plximg://localhost/" }
}

/// URL for an image path (server-relative or absolute) resized to `w`×`h`.
pub fn url(source: &str, w: u32, h: u32) -> String {
    let q = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("u", source)
        .append_pair("w", &w.to_string())
        .append_pair("h", &h.to_string())
        .finish();
    format!("{}i?{q}", base())
}

pub fn opt(source: Option<&str>, w: u32, h: u32) -> Option<String> {
    source.filter(|s| !s.is_empty()).map(|s| url(s, w, h))
}

pub fn handle(request: Request<Vec<u8>>, responder: RequestAsyncResponder) {
    let query = request.uri().query().unwrap_or("").to_string();
    tokio::spawn(async move {
        let mut source = String::new();
        let (mut w, mut h) = (300u32, 450u32);
        for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
            match k.as_ref() {
                "u" => source = v.into_owned(),
                "w" => w = v.parse().unwrap_or(w),
                "h" => h = v.parse().unwrap_or(h),
                _ => {}
            }
        }
        let response = match fetch(&source, w, h).await {
            Some((bytes, ctype)) => Response::builder()
                .status(200)
                .header("Content-Type", ctype)
                .header("Cache-Control", "public, max-age=604800")
                .header("Access-Control-Allow-Origin", "*")
                .body(Cow::Owned(bytes)),
            None => Response::builder().status(404).body(Cow::Borrowed(&b""[..])),
        };
        responder.respond(response.unwrap());
    });
}

async fn fetch(source: &str, w: u32, h: u32) -> Option<(Vec<u8>, String)> {
    if source.is_empty() {
        return None;
    }
    let server = SERVER.read().unwrap().clone();
    let scope = server.as_ref().and_then(|s| s.machine_identifier().map(str::to_string)).unwrap_or_default();
    let key = hex(&Sha256::digest(format!("{scope}|{source}|{w}|{h}").as_bytes()));
    let path = CACHE.join(&key[..2]).join(&key);
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

fn sniff(path: &std::path::Path) -> &'static str {
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
