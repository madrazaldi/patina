//! Artwork: images are resolved through the current server's photo
//! transcoder (so front ends never see the token) and the resized result is
//! cached on disk.
//!
//! The cache is built for a grid of hundreds of posters scrolling past at
//! once: simultaneous requests for one image share a single fetch, fetches
//! are limited to [`MAX_FETCHES`] at a time, entries are written atomically
//! (a reader never sees a half-written file), and the folder is kept under a
//! size budget as well as an age limit.

use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::{Duration, SystemTime};

use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use patina_plex::PlexServer;
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;

/// Images fetched from the server or the web at the same time.
pub const MAX_FETCHES: usize = 8;
/// The most disk the cache keeps; the least recently used entries go first.
pub const BUDGET_BYTES: u64 = 512 * 1024 * 1024;
/// Entries unused for this long are dropped.
pub const MAX_AGE: Duration = Duration::from_secs(60 * 60 * 24 * 30);
/// Written bytes after which the budget is checked again.
const PRUNE_EVERY_BYTES: u64 = 32 * 1024 * 1024;
/// A hit older than this has its timestamp refreshed, so use keeps entries alive.
const TOUCH_AFTER: Duration = Duration::from_secs(60 * 60);

static SERVER: RwLock<Option<PlexServer>> = RwLock::new(None);
static HTTP: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder().timeout(Duration::from_secs(30)).build().unwrap_or_default()
});
static CACHE: LazyLock<PathBuf> = LazyLock::new(|| {
    let dir = crate::config::cache_dir().join("images");
    let _ = std::fs::create_dir_all(&dir);
    dir
});
static PERMITS: Semaphore = Semaphore::const_new(MAX_FETCHES);
static FLIGHTS: LazyLock<Arc<SingleFlight<Option<Fetched>>>> = LazyLock::new(|| Arc::new(SingleFlight::default()));
static WRITTEN: AtomicU64 = AtomicU64::new(0);
static PRUNING: AtomicBool = AtomicBool::new(false);

type Fetched = Arc<(Vec<u8>, String)>;

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
    if let Some(hit) = read_cached(&path).await {
        return Some(hit);
    }
    let source = source.to_string();
    let job = {
        let path = path.clone();
        move || async move {
            // Another flight may have stored it between the miss and now.
            if let Some((bytes, ctype)) = read_cached(&path).await {
                return Some(Arc::new((bytes, ctype)));
            }
            let _permit = PERMITS.acquire().await.ok()?;
            let (bytes, ctype) = download(server.as_ref(), &source, w, h).await?;
            // The content type is re-sniffed from the bytes on cache hits.
            if write_atomic(&path, &bytes).await.is_ok() {
                note_written(bytes.len() as u64);
            }
            Some(Arc::new((bytes, ctype)))
        }
    };
    let shared = FLIGHTS.run(path, job).await??;
    Some((shared.0.clone(), shared.1.clone()))
}

/// The cached bytes of `path`, if any (an empty file counts as a miss).
async fn read_cached(path: &Path) -> Option<(Vec<u8>, String)> {
    let bytes = tokio::fs::read(path).await.ok().filter(|b| !b.is_empty())?;
    touch_if_stale(path);
    Some((bytes, sniff(path).into()))
}

/// Mark an old entry as used just now, off the request path.
fn touch_if_stale(path: &Path) {
    let old = std::fs::metadata(path).and_then(|m| m.modified()).is_ok_and(|t| t.elapsed().is_ok_and(|age| age > TOUCH_AFTER));
    if old {
        let path = path.to_path_buf();
        crate::runtime().spawn_blocking(move || {
            if let Ok(f) = std::fs::OpenOptions::new().write(true).open(&path) {
                let _ = f.set_modified(SystemTime::now());
            }
        });
    }
}

/// Ask the server (or the web) for the image.
async fn download(server: Option<&PlexServer>, source: &str, w: u32, h: u32) -> Option<(Vec<u8>, String)> {
    let is_absolute = source.starts_with("http://") || source.starts_with("https://");
    let (bytes, ctype) = match (server, is_absolute) {
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
    if bytes.is_empty() {
        return None;
    }
    let ctype = if ctype.starts_with("image/") { ctype } else { "image/jpeg".to_string() };
    Some((bytes, ctype))
}

async fn direct(url: &str) -> Option<(Vec<u8>, String)> {
    let resp = HTTP.get(url).send().await.ok()?.error_for_status().ok()?;
    let ctype = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    Some((resp.bytes().await.ok()?.to_vec(), ctype))
}

/// Write to a temporary sibling, then rename over `path`, so readers see
/// either the whole file or none of it.
async fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    tokio::fs::create_dir_all(dir).await?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!("{name}{TMP_MARK}{}", uuid::Uuid::new_v4().simple()));
    let result = async {
        tokio::fs::write(&tmp, bytes).await?;
        tokio::fs::rename(&tmp, path).await
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&tmp).await;
    }
    result
}

/// Temporary files carry this between the entry's name and a random suffix.
const TMP_MARK: &str = ".tmp-";

/// Count written bytes and check the budget in the background once enough accumulate.
fn note_written(bytes: u64) {
    if WRITTEN.fetch_add(bytes, Ordering::Relaxed) + bytes < PRUNE_EVERY_BYTES || PRUNING.swap(true, Ordering::SeqCst) {
        return;
    }
    WRITTEN.store(0, Ordering::Relaxed);
    crate::runtime().spawn_blocking(|| {
        prune_dir(&CACHE, SystemTime::now(), MAX_AGE, BUDGET_BYTES);
        PRUNING.store(false, Ordering::SeqCst);
    });
}

/// The content type of an image file, from its first bytes.
pub fn sniff(path: &Path) -> &'static str {
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

/// Drop cached images not used for 30 days, then the least recently used
/// ones until the cache fits its size budget.
pub fn prune_cache() {
    prune_dir(&CACHE, SystemTime::now(), MAX_AGE, BUDGET_BYTES);
}

/// What a prune removed.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pruned {
    pub files: usize,
    pub bytes: u64,
}

/// Prune the two-level cache folder `dir`: leftover temporary files, entries
/// last used before `now - max_age`, then oldest-used entries until the total
/// is at most 90% of `budget` (so a full cache is not trimmed on every write).
/// An entry's age is the later of its modification and access times.
pub fn prune_dir(dir: &Path, now: SystemTime, max_age: Duration, budget: u64) -> Pruned {
    let mut pruned = Pruned::default();
    let mut live: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
    let mut total = 0u64;
    let Ok(dirs) = std::fs::read_dir(dir) else { return pruned };
    for sub in dirs.flatten() {
        let Ok(files) = std::fs::read_dir(sub.path()) else { continue };
        for f in files.flatten() {
            let Ok(meta) = f.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let used = [meta.accessed().ok(), meta.modified().ok()].into_iter().flatten().max().unwrap_or(now);
            let age = now.duration_since(used).unwrap_or_default();
            let temporary = f.file_name().to_string_lossy().contains(TMP_MARK);
            // A temporary file this old belongs to a write that died.
            let expired = if temporary { age > Duration::from_secs(3600) } else { age > max_age };
            if expired {
                if std::fs::remove_file(f.path()).is_ok() {
                    pruned.files += 1;
                    pruned.bytes += meta.len();
                }
            } else if !temporary {
                total += meta.len();
                live.push((used, meta.len(), f.path()));
            }
        }
    }
    if total > budget {
        let target = budget / 10 * 9;
        live.sort_by_key(|(used, ..)| *used);
        for (_, len, path) in live {
            if total <= target {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total -= len;
                pruned.files += 1;
                pruned.bytes += len;
            }
        }
    }
    pruned
}

/// Runs one task per key and lets every caller for that key wait on it.
///
/// The task is spawned, not driven by its callers: if all of them give up the
/// work still finishes (and releases whatever it holds), and the key is
/// forgotten the moment it does.
struct SingleFlight<V> {
    flights: Mutex<HashMap<PathBuf, Shared<BoxFuture<'static, Option<V>>>>>,
}

impl<V> Default for SingleFlight<V> {
    fn default() -> Self {
        Self { flights: Mutex::new(HashMap::new()) }
    }
}

impl<V: Clone + Send + Sync + 'static> SingleFlight<V> {
    /// The result of the task for `key`, starting it with `make` unless one is
    /// already running. `None` if the task panicked.
    fn run<F, Fut>(self: &Arc<Self>, key: PathBuf, make: F) -> impl Future<Output = Option<V>> + use<V, F, Fut>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = V> + Send + 'static,
    {
        let mut flights = self.flights.lock().unwrap();
        if let Some(flight) = flights.get(&key) {
            return flight.clone();
        }
        let forget = Forget { flights: self.clone(), key: key.clone() };
        let task = make();
        // The lock is held across the spawn, so the task cannot remove its entry before it exists.
        let handle = crate::runtime().spawn(async move {
            let _forget = forget;
            task.await
        });
        let flight = async move { handle.await.ok() }.boxed().shared();
        flights.insert(key, flight.clone());
        flight
    }

    #[cfg(test)]
    fn running(&self) -> usize {
        self.flights.lock().unwrap().len()
    }
}

/// Removes a finished (or panicked) task's key.
struct Forget<V> {
    flights: Arc<SingleFlight<V>>,
    key: PathBuf,
}

impl<V> Drop for Forget<V> {
    fn drop(&mut self) {
        if let Ok(mut flights) = self.flights.flights.lock() {
            flights.remove(&self.key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[tokio::test]
    async fn simultaneous_requests_share_one_fetch() {
        let flights = Arc::new(SingleFlight::<u32>::default());
        let started = Arc::new(AtomicUsize::new(0));
        let key = PathBuf::from("poster");
        let calls: Vec<_> = (0..20)
            .map(|_| {
                let started = started.clone();
                flights.run(key.clone(), move || async move {
                    started.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    7
                })
            })
            .collect();
        let results = futures::future::join_all(calls).await;
        assert!(results.iter().all(|r| *r == Some(7)));
        assert_eq!(started.load(Ordering::SeqCst), 1);
        assert_eq!(flights.running(), 0, "a finished key must be forgotten");
        // A later request starts again.
        assert_eq!(flights.run(key, || async { 8 }).await, Some(8));
    }

    #[tokio::test]
    async fn different_keys_run_independently_and_work_survives_abandonment() {
        let flights = Arc::new(SingleFlight::<u32>::default());
        let done = Arc::new(AtomicUsize::new(0));
        // Nobody waits for "a": dropping the future must not cancel the work.
        let d = done.clone();
        drop(flights.run("a".into(), move || async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            d.fetch_add(1, Ordering::SeqCst);
            1
        }));
        let b = flights.run("b".into(), || async { 2 }).await;
        assert_eq!(b, Some(2));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(done.load(Ordering::SeqCst), 1);
        assert_eq!(flights.running(), 0);
    }

    #[tokio::test]
    async fn a_panicking_task_is_reported_and_forgotten() {
        let flights = Arc::new(SingleFlight::<u32>::default());
        let r = flights.run("x".into(), || async { panic!("boom") }).await;
        assert_eq!(r, None);
        assert_eq!(flights.running(), 0, "a panic must not wedge the key");
        assert_eq!(flights.run("x".into(), || async { 3 }).await, Some(3));
    }

    #[tokio::test]
    async fn fetches_are_bounded() {
        let (running, peak) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let permits = Arc::new(Semaphore::new(3));
        let flights = Arc::new(SingleFlight::<()>::default());
        let calls: Vec<_> = (0..12)
            .map(|i| {
                let (running, peak, permits) = (running.clone(), peak.clone(), permits.clone());
                flights.run(PathBuf::from(format!("k{i}")), move || async move {
                    let _p = permits.acquire().await.unwrap();
                    let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    running.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();
        futures::future::join_all(calls).await;
        assert_eq!(peak.load(Ordering::SeqCst), 3);
    }

    fn entry(dir: &Path, name: &str, bytes: usize, used: SystemTime) {
        let sub = dir.join(&name[..2]);
        std::fs::create_dir_all(&sub).unwrap();
        let path = sub.join(name);
        std::fs::write(&path, vec![1u8; bytes]).unwrap();
        // A new file's access time is now; set both so the entry looks last used at `used`.
        let f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        f.set_times(std::fs::FileTimes::new().set_accessed(used).set_modified(used)).unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .flat_map(|d| std::fs::read_dir(d.path()).unwrap().flatten().map(|f| f.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>())
            .collect();
        out.sort();
        out
    }

    #[test]
    fn prune_drops_old_entries_and_dead_temporary_files() {
        let tmp = tempfile::tempdir().unwrap();
        let now = SystemTime::now();
        let day = Duration::from_secs(86_400);
        entry(tmp.path(), "aa-fresh", 10, now - day);
        entry(tmp.path(), "bb-stale", 10, now - 40 * day);
        entry(tmp.path(), "cc-dead.tmp-1", 10, now - 2 * day);
        entry(tmp.path(), "dd-writing.tmp-2", 10, now);
        let pruned = prune_dir(tmp.path(), now, 30 * day, u64::MAX);
        assert_eq!(pruned, Pruned { files: 2, bytes: 20 });
        assert_eq!(names(tmp.path()), ["aa-fresh", "dd-writing.tmp-2"]);
    }

    #[test]
    fn prune_enforces_the_size_budget_oldest_first() {
        let tmp = tempfile::tempdir().unwrap();
        let now = SystemTime::now();
        let hour = Duration::from_secs(3600);
        for i in 0..10u64 {
            // e0 is the most recently used, e9 the least.
            entry(tmp.path(), &format!("e{i}"), 100, now - hour * (i as u32 + 1));
        }
        // 1000 bytes against a 500 budget trims to 90% of it (450): the six least recently used go.
        let pruned = prune_dir(tmp.path(), now, Duration::from_secs(86_400 * 30), 500);
        assert_eq!(pruned.bytes, 600);
        assert_eq!(names(tmp.path()), ["e0", "e1", "e2", "e3"]);
        // Under budget: nothing changes.
        assert_eq!(prune_dir(tmp.path(), now, Duration::from_secs(86_400 * 30), 500), Pruned::default());
    }

    #[tokio::test]
    async fn atomic_writes_leave_no_temporary_files() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("ab").join("abcdef");
        write_atomic(&path, b"first").await.unwrap();
        write_atomic(&path, b"second").await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        assert_eq!(names(tmp.path()), ["abcdef"]);
        // An unwritable destination cleans up after itself.
        let blocked = tmp.path().join("ab").join("abcdef").join("child");
        assert!(write_atomic(&blocked, b"x").await.is_err());
        assert_eq!(names(tmp.path()), ["abcdef"]);
    }

    /// A server answering every request with a small PNG after a delay, counting
    /// requests and the most it served at once.
    async fn slow_image_server(delay: Duration) -> (String, Arc<AtomicUsize>, Arc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (requests, peak, active) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let (r, p) = (requests.clone(), peak.clone());
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else { return };
                let (r, p, active) = (r.clone(), p.clone(), active.clone());
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut buf = [0u8; 4096];
                    let _ = socket.read(&mut buf).await;
                    r.fetch_add(1, Ordering::SeqCst);
                    p.fetch_max(active.fetch_add(1, Ordering::SeqCst) + 1, Ordering::SeqCst);
                    tokio::time::sleep(delay).await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    let body = [0x89, b'P', b'N', b'G', 1, 2, 3, 4];
                    let head = format!("HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(&body).await;
                });
            }
        });
        (url, requests, peak)
    }

    #[tokio::test]
    async fn fetch_shares_requests_bounds_concurrency_and_caches() {
        let (url, requests, peak) = slow_image_server(Duration::from_millis(150)).await;
        let client = patina_plex::ClientInfo::new("test", "Patina", "test");
        set_server(Some(PlexServer::new(client, &url, "token").unwrap()));
        let run = uuid::Uuid::new_v4().simple().to_string();

        // Thirty simultaneous requests for one image make one request.
        let same = format!("/library/metadata/1/thumb/{run}");
        let results = futures::future::join_all((0..30).map(|_| fetch(&same, 300, 450))).await;
        assert!(results.iter().all(|r| r.as_ref().is_some_and(|(b, c)| b.len() == 8 && c == "image/png")));
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        // And it is cached, so asking again makes none.
        assert!(fetch(&same, 300, 450).await.is_some());
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        assert!(cached_file(&same, 300, 450).await.is_some_and(|p| p.is_file()));

        // A grid's worth of different images never has more than MAX_FETCHES in flight.
        let many: Vec<String> = (0..4 * MAX_FETCHES).map(|i| format!("/library/metadata/{}/thumb/{run}", 100 + i)).collect();
        let results = futures::future::join_all(many.iter().map(|s| fetch(s, 300, 450))).await;
        assert!(results.iter().all(Option::is_some));
        assert_eq!(requests.load(Ordering::SeqCst), 1 + many.len());
        assert!(peak.load(Ordering::SeqCst) <= MAX_FETCHES, "peak {}", peak.load(Ordering::SeqCst));
        assert!(peak.load(Ordering::SeqCst) > 1, "fetches should still overlap");

        let server = SERVER.read().unwrap().clone();
        for s in many.iter().chain([&same]) {
            let _ = std::fs::remove_file(cache_path(server.as_ref(), s, 300, 450));
        }
        set_server(None);
    }

    #[tokio::test]
    async fn empty_cache_files_are_misses() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("empty");
        std::fs::write(&path, b"").unwrap();
        assert!(read_cached(&path).await.is_none());
        std::fs::write(&path, [0x89, b'P', b'N', b'G', 0, 0]).unwrap();
        let (bytes, ctype) = read_cached(&path).await.unwrap();
        assert_eq!((bytes.len(), ctype.as_str()), (6, "image/png"));
    }
}
