//! Spawn and drive an external mpv process over its JSON IPC socket.
//!
//! mpv is started without `--no-config` or `--config-dir`, so it loads the
//! user's normal configuration directory (`~/.config/mpv` on macOS/Linux,
//! `%APPDATA%\mpv` on Windows), including scripts, input.conf and mpv.conf.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, mpsc, oneshot};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not start mpv ({program}): {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("mpv IPC socket did not come up")]
    NoSocket,
    #[error("mpv exited")]
    Closed,
    #[error("mpv error: {0}")]
    Command(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// How to launch mpv.
#[derive(Debug, Clone)]
pub struct SpawnOptions {
    /// mpv executable (name on PATH or absolute path).
    pub program: String,
    /// Extra command-line arguments appended after ours.
    pub extra_args: Vec<String>,
    /// HTTP headers sent with every request mpv makes.
    pub http_headers: Vec<(String, String)>,
}

impl Default for SpawnOptions {
    fn default() -> Self {
        Self { program: default_program(), extra_args: Vec::new(), http_headers: Vec::new() }
    }
}

/// Best guess at the mpv binary: PATH, then common install locations.
pub fn default_program() -> String {
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &["/opt/homebrew/bin/mpv", "/usr/local/bin/mpv", "/Applications/mpv.app/Contents/MacOS/mpv"]
    } else if cfg!(windows) {
        &[]
    } else {
        &["/usr/bin/mpv", "/usr/local/bin/mpv"]
    };
    if let Some(found) = which("mpv") {
        return found;
    }
    candidates
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(|p| p.to_string())
        .unwrap_or_else(|| "mpv".into())
}

fn which(name: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    let exe = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    std::env::split_paths(&path).map(|dir| dir.join(&exe)).find(|p| p.is_file()).map(|p| p.display().to_string())
}

/// Check that mpv can be executed and return its version line.
pub async fn probe(program: &str) -> Result<String> {
    let out = Command::new(program)
        .arg("--version")
        .output()
        .await
        .map_err(|source| Error::Spawn { program: program.into(), source })?;
    Ok(String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or("mpv").to_string())
}

/// Events from mpv.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    PropertyChange { id: u64, name: String, data: Value },
    StartFile { playlist_entry_id: Option<i64> },
    FileLoaded,
    EndFile { reason: String, playlist_entry_id: Option<i64>, error: Option<String> },
    Seek,
    PlaybackRestart,
    Idle,
    /// `script-message` broadcasts (including from our own key bindings).
    ClientMessage { args: Vec<String> },
    Shutdown,
    Other { name: String, raw: Value },
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<std::result::Result<Value, String>>>>>;

/// A running mpv instance.
pub struct Mpv {
    writer: Mutex<Box<dyn tokio::io::AsyncWrite + Send + Unpin>>,
    pending: Pending,
    next_id: AtomicU64,
    child: Mutex<Child>,
    socket: PathBuf,
}

impl Mpv {
    /// Launch mpv idle, connect to its IPC socket, and return it with its event stream.
    pub async fn spawn(opts: SpawnOptions) -> Result<(Arc<Mpv>, mpsc::UnboundedReceiver<Event>)> {
        let socket = socket_path();
        let mut cmd = Command::new(&opts.program);
        cmd.arg("--idle=yes")
            .arg(format!("--input-ipc-server={}", socket.display()))
            .arg("--msg-level=all=warn")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if !opts.http_headers.is_empty() {
            let headers: Vec<String> = opts
                .http_headers
                .iter()
                .map(|(k, v)| format!("{k}: {v}").replace(',', "\\,"))
                .collect();
            cmd.arg(format!("--http-header-fields={}", headers.join(",")));
        }
        cmd.args(&opts.extra_args);
        let mut child = cmd.spawn().map_err(|source| Error::Spawn { program: opts.program.clone(), source })?;

        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::info!(target: "mpv", "{line}");
                }
            });
        }

        let (reader, writer) = connect(&socket, &mut child).await?;
        let pending: Pending = Arc::default();
        let (tx, rx) = mpsc::unbounded_channel();

        let pending_r = pending.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                if let Some(id) = v.get("request_id").and_then(Value::as_u64) {
                    if let Some(sender) = pending_r.lock().await.remove(&id) {
                        let res = match v.get("error").and_then(Value::as_str) {
                            Some("success") | None => Ok(v.get("data").cloned().unwrap_or(Value::Null)),
                            Some(err) => Err(err.to_string()),
                        };
                        let _ = sender.send(res);
                    }
                    continue;
                }
                if let Some(event) = parse_event(&v) {
                    let _ = tx.send(event);
                }
            }
            let _ = tx.send(Event::Shutdown);
            // Fail anything still waiting.
            pending_r.lock().await.clear();
        });

        let mpv = Arc::new(Mpv {
            writer: Mutex::new(writer),
            pending,
            next_id: AtomicU64::new(1),
            child: Mutex::new(child),
            socket,
        });
        Ok((mpv, rx))
    }

    /// Run an mpv command (`["loadfile", url, "replace"]`, ...).
    pub async fn command(&self, args: Vec<Value>) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        let mut line = serde_json::to_string(&json!({ "command": args, "request_id": id })).unwrap();
        line.push('\n');
        {
            let mut w = self.writer.lock().await;
            if w.write_all(line.as_bytes()).await.is_err() || w.flush().await.is_err() {
                self.pending.lock().await.remove(&id);
                return Err(Error::Closed);
            }
        }
        match tokio::time::timeout(Duration::from_secs(15), rx).await {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err(e))) => Err(Error::Command(e)),
            Ok(Err(_)) => Err(Error::Closed),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(Error::Command("timeout".into()))
            }
        }
    }

    pub async fn get_property(&self, name: &str) -> Result<Value> {
        self.command(vec![json!("get_property"), json!(name)]).await
    }

    pub async fn set_property(&self, name: &str, value: impl Into<Value>) -> Result<()> {
        self.command(vec![json!("set_property"), json!(name), value.into()]).await.map(|_| ())
    }

    pub async fn observe_property(&self, id: u64, name: &str) -> Result<()> {
        self.command(vec![json!("observe_property"), json!(id), json!(name)]).await.map(|_| ())
    }

    /// `loadfile` with per-file options (`start`, `force-media-title`, ...).
    /// `mode` is `replace`, `append`, `append-play`, `insert-next`, ...
    pub async fn loadfile(&self, url: &str, mode: &str, options: &[(&str, String)]) -> Result<Value> {
        let opts = options
            .iter()
            .map(|(k, v)| format!("{k}={}", quote_option(v)))
            .collect::<Vec<_>>()
            .join(",");
        self.command(vec![json!("loadfile"), json!(url), json!(mode), json!(-1), json!(opts)]).await
    }

    /// Add an external subtitle track. `flags`: `select`, `auto` or `cached`.
    pub async fn sub_add(&self, url: &str, flags: &str, title: &str, lang: &str) -> Result<()> {
        self.command(vec![json!("sub-add"), json!(url), json!(flags), json!(title), json!(lang)]).await.map(|_| ())
    }

    pub async fn seek_absolute(&self, secs: f64) -> Result<()> {
        self.command(vec![json!("seek"), json!(secs), json!("absolute")]).await.map(|_| ())
    }

    pub async fn show_text(&self, text: &str, duration_ms: u32) -> Result<()> {
        self.command(vec![json!("show-text"), json!(text), json!(duration_ms)]).await.map(|_| ())
    }

    /// Bind a key to broadcast `script-message <message>`; arrives as [`Event::ClientMessage`].
    pub async fn bind_message(&self, key: &str, message: &str) -> Result<()> {
        self.command(vec![json!("keybind"), json!(key), json!(format!("script-message {message}"))]).await.map(|_| ())
    }

    pub async fn playlist_next(&self) -> Result<()> {
        self.command(vec![json!("playlist-next"), json!("force")]).await.map(|_| ())
    }

    pub async fn playlist_prev(&self) -> Result<()> {
        self.command(vec![json!("playlist-prev")]).await.map(|_| ())
    }

    pub async fn set_pause(&self, paused: bool) -> Result<()> {
        self.set_property("pause", paused).await
    }

    pub async fn quit(&self) -> Result<()> {
        let r = self.command(vec![json!("quit")]).await;
        match r {
            Ok(_) | Err(Error::Closed) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Wait for the process to exit (killing it after `grace`).
    pub async fn wait(&self, grace: Duration) {
        let mut child = self.child.lock().await;
        if tokio::time::timeout(grace, child.wait()).await.is_err() {
            let _ = child.kill().await;
        }
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// mpv's `%len%value` quoting for option lists, so commas and quotes are safe.
pub fn quote_option(v: &str) -> String {
    format!("%{}%{}", v.len(), v)
}

fn parse_event(v: &Value) -> Option<Event> {
    let name = v.get("event")?.as_str()?;
    let entry = || v.get("playlist_entry_id").and_then(Value::as_i64);
    Some(match name {
        "property-change" => Event::PropertyChange {
            id: v.get("id").and_then(Value::as_u64).unwrap_or(0),
            name: v.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
            data: v.get("data").cloned().unwrap_or(Value::Null),
        },
        "start-file" => Event::StartFile { playlist_entry_id: entry() },
        "file-loaded" => Event::FileLoaded,
        "end-file" => Event::EndFile {
            reason: v.get("reason").and_then(Value::as_str).unwrap_or("").to_string(),
            playlist_entry_id: entry(),
            error: v.get("file_error").and_then(Value::as_str).map(str::to_string),
        },
        "seek" => Event::Seek,
        "playback-restart" => Event::PlaybackRestart,
        "idle" => Event::Idle,
        "client-message" => Event::ClientMessage {
            args: v
                .get("args")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
        },
        "shutdown" => Event::Shutdown,
        other => Event::Other { name: other.to_string(), raw: v.clone() },
    })
}

fn socket_path() -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let name = format!("patina-mpv-{}-{nonce}", std::process::id());
    if cfg!(windows) {
        PathBuf::from(format!(r"\\.\pipe\{name}"))
    } else {
        std::env::temp_dir().join(format!("{name}.sock"))
    }
}

type Reader = Box<dyn tokio::io::AsyncRead + Send + Unpin>;
type Writer = Box<dyn tokio::io::AsyncWrite + Send + Unpin>;

async fn connect(socket: &PathBuf, child: &mut Child) -> Result<(Reader, Writer)> {
    for _ in 0..100 {
        if let Ok(Some(_)) = child.try_wait() {
            return Err(Error::Closed);
        }
        #[cfg(unix)]
        if let Ok(stream) = tokio::net::UnixStream::connect(socket).await {
            let (r, w) = stream.into_split();
            return Ok((Box::new(r), Box::new(w)));
        }
        #[cfg(windows)]
        if let Ok(pipe) = tokio::net::windows::named_pipe::ClientOptions::new().open(socket) {
            let (r, w) = tokio::io::split(pipe);
            return Ok((Box::new(r), Box::new(w)));
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err(Error::NoSocket)
}
