//! A C interface to Patina's backend, used by the native macOS app
//! (`macos/`). It mirrors the Tauri app's IPC: commands are called by name
//! with a JSON object of camelCase arguments and answer with JSON, and core
//! events arrive by name with the same payloads the webview receives.
//!
//! Callbacks run on backend threads. Strings and bytes handed to a callback
//! are only valid for the duration of that call; copy them.
//! `include/patina.h` declares these functions.

mod dispatch;

use std::ffi::{CStr, CString, c_char, c_void};
use std::sync::{Arc, OnceLock};

use patina_core::{AppState, CoreEvent, EventSink, images, player, presence, session};
use serde_json::Value;

pub type EventCallback = extern "C" fn(ctx: *mut c_void, name: *const c_char, payload: *const c_char);
pub type ReplyCallback = extern "C" fn(ctx: *mut c_void, ok: bool, json: *const c_char);
pub type BytesCallback = extern "C" fn(ctx: *mut c_void, data: *const u8, len: usize);

/// An opaque pointer the caller owns; handed back to its callback untouched.
#[derive(Clone, Copy)]
struct Ctx(*mut c_void);

// The caller promises its context may be used from any thread.
unsafe impl Send for Ctx {}
unsafe impl Sync for Ctx {}

impl Ctx {
    // A method, so closures capture the whole `Ctx` rather than its raw pointer.
    fn ptr(self) -> *mut c_void {
        self.0
    }
}

struct Bridge {
    runtime: tokio::runtime::Runtime,
    app: AppState,
}

static BRIDGE: OnceLock<Bridge> = OnceLock::new();
static EVENTS: OnceLock<(Ctx, EventCallback)> = OnceLock::new();
static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

/// Forwards core events to the app, and what's playing to Discord.
/// The app shows the OS media controls itself.
struct Native {
    ctx: Ctx,
    callback: EventCallback,
}

impl EventSink for Native {
    fn emit(&self, event: CoreEvent) {
        if let CoreEvent::NowPlaying(np) = &event
            && let Some(bridge) = BRIDGE.get()
        {
            presence::update(np.as_deref(), &bridge.app.settings().player);
        }
        send_event(self.ctx, self.callback, event.name(), &event.payload());
    }
}

/// Send an event that isn't a [`CoreEvent`], such as command progress.
pub(crate) fn emit(name: &str, payload: &Value) {
    if let Some(&(ctx, callback)) = EVENTS.get() {
        send_event(ctx, callback, name, payload);
    }
}

fn send_event(ctx: Ctx, callback: EventCallback, name: &str, payload: &Value) {
    let (Ok(name), Ok(payload)) = (CString::new(name), CString::new(payload.to_string())) else { return };
    callback(ctx.ptr(), name.as_ptr(), payload.as_ptr());
}

fn init_logging() {
    let dir = patina_core::config::data_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let Ok(file) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("patina.log")) else { return };
    let filter = tracing_subscriber::EnvFilter::try_from_env("PATINA_LOG")
        .unwrap_or_else(|_| "warn,patina=info,patina_core=info,patina_plex=info,mpv=warn".into());
    let (writer, guard) = tracing_appender::non_blocking(file);
    if tracing_subscriber::fmt().with_writer(writer).with_ansi(false).with_env_filter(filter).try_init().is_ok() {
        let _ = LOG_GUARD.set(guard);
    }
}

fn bridge() -> &'static Bridge {
    BRIDGE.get().expect("patina_start must be called first")
}

unsafe fn str_arg<'a>(p: *const c_char) -> &'a str {
    if p.is_null() { "" } else { unsafe { CStr::from_ptr(p) }.to_str().unwrap_or("") }
}

/// Start the backend and restore the saved session. Every core event is sent
/// to `callback` with its name and JSON payload. Call once, before anything else.
#[unsafe(no_mangle)]
pub extern "C" fn patina_start(ctx: *mut c_void, callback: EventCallback) {
    let ctx = Ctx(ctx);
    let _ = EVENTS.set((ctx, callback));
    BRIDGE.get_or_init(|| {
        init_logging();
        tracing::info!(version = env!("CARGO_PKG_VERSION"), "patina starting");
        let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("tokio runtime");
        let app = AppState::new(Arc::new(Native { ctx, callback }), runtime.handle().clone());
        std::thread::spawn(images::prune_cache);
        app.spawn(session::restore(app.clone()));
        Bridge { runtime, app }
    });
}

/// Call the command `name` with `args` (a JSON object, or null). `callback`
/// receives `ok` and the JSON result, or the error message as a JSON string.
///
/// # Safety
/// String arguments must be null or valid NUL-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_call(name: *const c_char, args: *const c_char, ctx: *mut c_void, callback: ReplyCallback) {
    let name = unsafe { str_arg(name) }.to_owned();
    let args = unsafe { str_arg(args) }.to_owned();
    let ctx = Ctx(ctx);
    let bridge = bridge();
    let app = bridge.app.clone();
    bridge.runtime.spawn(async move {
        let args = if args.is_empty() { Ok(Value::Null) } else { serde_json::from_str(&args) };
        // A separate task, so a panic still answers the callback (the caller waits on it).
        let result = match args {
            Ok(args) => {
                let task = {
                    let name = name.clone();
                    tokio::spawn(async move { dispatch::call(&app, &name, args).await })
                };
                task.await.unwrap_or_else(|e| Err(anyhow::anyhow!("{name} failed unexpectedly: {e}")))
            }
            Err(e) => Err(e.into()),
        };
        let (ok, value) = match result {
            Ok(v) => (true, v),
            Err(e) => {
                tracing::debug!("{name} failed: {e:#}");
                (false, Value::String(format!("{e:#}")))
            }
        };
        let json = CString::new(value.to_string()).unwrap_or_default();
        callback(ctx.ptr(), ok, json.as_ptr());
    });
}

/// Artwork through core's cache, so the token never leaves the backend.
/// `source` is a Plex image path or URL. `callback` gets no bytes (null, 0)
/// when the image can't be loaded.
///
/// # Safety
/// String arguments must be null or valid NUL-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_image(source: *const c_char, width: u32, height: u32, ctx: *mut c_void, callback: BytesCallback) {
    let source = unsafe { str_arg(source) }.to_owned();
    let ctx = Ctx(ctx);
    bridge().runtime.spawn(async move {
        // A separate task, so a panic still answers the callback.
        match tokio::spawn(async move { images::fetch(&source, width, height).await }).await.ok().flatten() {
            Some((bytes, _)) => callback(ctx.ptr(), bytes.as_ptr(), bytes.len()),
            None => callback(ctx.ptr(), std::ptr::null(), 0),
        }
    });
}

/// The saved artwork of a download, which works offline.
///
/// # Safety
/// String arguments must be null or valid NUL-terminated C strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn patina_download_image(id: *const c_char, ctx: *mut c_void, callback: BytesCallback) {
    let id = unsafe { str_arg(id) }.to_owned();
    let ctx = Ctx(ctx);
    let load = move || std::panic::catch_unwind(|| images::download_poster(&id)).ok().flatten();
    bridge().runtime.spawn_blocking(move || match load() {
        Some((bytes, _)) => callback(ctx.ptr(), bytes.as_ptr(), bytes.len()),
        None => callback(ctx.ptr(), std::ptr::null(), 0),
    });
}

/// Stop mpv. Call when the app quits.
#[unsafe(no_mangle)]
pub extern "C" fn patina_shutdown() {
    player::shutdown();
}
