//! Item actions shared by cards, menus and detail pages.

use crate::prelude::*;
use gpui_kit::App;
use patina_core::player::{self, PlayRequest, QueueCommand};
use patina_core::{downloads, undo};
use patina_plex::Metadata;

use crate::core::core;
use crate::store::{act, act_then, toast};

/// Options for [`play_with`].
#[derive(Clone, Debug)]
pub struct PlayOptions {
    pub resume: bool,
    pub shuffle: bool,
    pub media_index: Option<usize>,
    /// Play the item's radio station (music).
    pub radio: bool,
}

impl Default for PlayOptions {
    fn default() -> Self {
        Self { resume: true, shuffle: false, media_index: None, radio: false }
    }
}

/// Hand an item (or container) to mpv.
pub fn play(item: &Metadata, cx: &mut App) {
    play_with(item, PlayOptions::default(), cx);
}

pub fn play_with(item: &Metadata, opts: PlayOptions, cx: &mut App) {
    let app = core(cx);
    let req = PlayRequest {
        item: item.clone(),
        resume: opts.resume,
        shuffle: opts.shuffle,
        media_index: opts.media_index,
        radio: opts.radio,
        ..Default::default()
    };
    let _guard = patina_core::runtime().enter();
    player::play(app, req);
}

/// Play several items as one queue, in order.
pub fn play_selection(items: Vec<Metadata>, shuffle: bool, cx: &mut App) {
    let Some(first) = items.first().cloned() else { return };
    let app = core(cx);
    let _guard = patina_core::runtime().enter();
    player::play(app, PlayRequest { item: first, selection: items, shuffle, resume: false, ..Default::default() });
}

/// Mark watched or unwatched. Core announces the change with a toast that offers Undo, and refreshes the views.
pub fn set_watched(item: &Metadata, watched: bool, cx: &mut App) {
    let app = core(cx);
    let server = item.server_id().map(str::to_string);
    let keys = vec![item.rating_key().to_string()];
    act(cx, async move { undo::set_watched(&app, server.as_deref(), &keys, watched).await }, None, false);
}

/// Mark many items (of the current server) watched or unwatched at once.
pub fn set_watched_many(keys: Vec<String>, watched: bool, cx: &mut App) {
    let app = core(cx);
    act(cx, async move { undo::set_watched(&app, None, &keys, watched).await }, None, false);
}

/// Queue an item (or everything in a container) next, or at the end. Starts playback if idle.
pub fn enqueue(item: &Metadata, next: bool, cx: &mut App) {
    let app = core(cx);
    let item = Box::new(item.clone());
    let _guard = patina_core::runtime().enter();
    player::queue(app, QueueCommand::Add { item, next });
}

/// Download an item, or every episode/track below it, for offline playback.
pub fn download(item: &Metadata, cx: &mut App) {
    let app = core(cx);
    let key = item.rating_key().to_string();
    act_then(cx, async move { downloads::enqueue(&app, &key).await }, |n, cx| {
        toast(
            cx,
            match n {
                0 => "Already downloaded".to_string(),
                1 => "Downloading…".to_string(),
                n => format!("Downloading {n} items…"),
            },
        )
    });
}

/// Reverse a reversible action: a toast's Undo button passes its entry, Cmd/Ctrl+Z the latest.
/// Core announces the result.
pub fn undo(entry: Option<u64>, cx: &mut App) {
    let app = core(cx);
    act(cx, async move { undo::undo(&app, entry).await }, None, false);
}

/// Cmd/Ctrl+Z.
pub fn undo_latest(cx: &mut App) {
    let app = core(cx);
    match undo::last(&app) {
        Some((id, _)) => undo(Some(id), cx),
        None => toast(cx, "Nothing to undo"),
    }
}

/// Open a URL in the browser.
pub fn open_url(url: &str, cx: &mut App) {
    cx.open_url(url);
}
