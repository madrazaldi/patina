//! Patina's desktop app: a native GPUI front end on `patina-core`.
//!
//! The core runs on a tokio runtime and reports every state change as a
//! [`CoreEvent`](patina_core::CoreEvent); [`store::Store`] mirrors those for the
//! views, which call the core directly (see [`core::bg`]) for everything else.

mod actions;
mod art;
mod core;
mod dialogs;
mod format;
mod gate;
mod menu;
mod nav;
mod pages;
mod palette;
mod player;
mod prelude;
mod shell;
mod store;
mod theme;
mod widgets;

use std::sync::Arc;

use gpui_kit::*;
use patina_core::AppState;

fn main() {
    let filter = std::env::var("PATINA_LOG").unwrap_or_else(|_| "warn".into());
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .init();

    // Background work (network, mpv, downloads) runs on tokio; the UI on GPUI's own executor.
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("patina-core").build().expect("tokio runtime");
    let handle = runtime.handle().clone();
    // Kept alive for the life of the process.
    std::mem::forget(runtime);

    let (sink, events) = core::GuiSink::new();
    let app = AppState::new(Arc::new(sink), handle.clone());
    handle.spawn(patina_core::session::restore(app.clone()));

    gpui_kit::application().with_assets(gpui_kit::assets::AllAssets).run(move |cx| {
        gpui_kit::init(cx);
        theme::init(cx);
        cx.set_global(core::Core(app.clone()));
        store::init(events, cx);
        nav::init(cx);
        shell::bind_keys(cx);

        // Close mpv and let the last playback reports reach Plex. GPUI gives quit futures
        // only a moment, so this blocks here, as the TUI does on exit.
        cx.on_app_quit(|_| {
            patina_core::player::shutdown();
            async {}
        })
        .detach();

        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
        // `PATINA_BACKGROUND=1` (debug builds) opens the window without taking focus, for
        // scripted runs and screenshots while you work in another app.
        let background = cfg!(debug_assertions) && std::env::var_os("PATINA_BACKGROUND").is_some_and(|v| v == "1");
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Patina".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(16.), px(18.))),
            }),
            window_min_size: Some(size(px(900.), px(560.))),
            app_id: Some("patina".into()),
            focus: !background,
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| shell::Main::new(window, cx))).expect("open the window");
        if !background {
            cx.activate(true);
        }
    });
}
