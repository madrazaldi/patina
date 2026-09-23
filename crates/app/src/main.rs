//! Patina — a Plex client that plays through mpv.

mod config;
mod fmt;
mod images;
mod player;
mod session;
mod state;
mod ui;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("PATINA_LOG")
                .unwrap_or_else(|_| "warn,patina=info,patina_plex=info,mpv=warn".into()),
        )
        .init();

    std::thread::spawn(images::prune_cache);

    let window = WindowBuilder::new()
        .with_title("Patina")
        .with_inner_size(LogicalSize::new(1440.0, 900.0))
        .with_min_inner_size(LogicalSize::new(960.0, 620.0));
    #[cfg(target_os = "macos")]
    let window = {
        use dioxus::desktop::tao::platform::macos::WindowBuilderExtMacOS;
        window.with_titlebar_transparent(true).with_fullsize_content_view(true).with_title_hidden(true)
    };

    let config = Config::new()
        .with_window(window)
        .with_menu(None)
        .with_background_color((9, 11, 12, 255))
        .with_custom_head(ui::head())
        .with_disable_context_menu(!cfg!(debug_assertions))
        .with_asynchronous_custom_protocol(images::SCHEME, |_id, request, responder| images::handle(request, responder));

    LaunchBuilder::desktop().with_cfg(config).launch(ui::App);
}
