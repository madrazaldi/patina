//! Right-click / "…" menu for any item.

use crate::prelude::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::*;
use patina_plex::Metadata;

use crate::actions::{self, PlayOptions};
use crate::format;
use crate::nav::{self, Route};

fn entry(label: impl Into<SharedString>, icon: IconName, f: impl Fn(&mut Window, &mut App) + 'static) -> PopupMenuItem {
    PopupMenuItem::new(label).icon(icon).on_click(move |_, window, cx| f(window, cx))
}

/// Fill `menu` with what can be done to `item`.
pub fn item_menu(menu: PopupMenu, item: &Metadata, _window: &mut Window, _cx: &mut Context<PopupMenu>) -> PopupMenu {
    let kind = item.kind().to_string();
    let resume = item.progress().is_some();
    let watched = item.is_watched();
    let queueable = format::can_play(item) && !matches!(kind.as_str(), "photo" | "photoalbum");
    let foreign = item.server_id().map(str::to_string);

    let mut menu = menu.min_w(px(220.)).label(item.full_title());

    if format::can_play(item) {
        let it = item.clone();
        menu = menu.item(entry(if resume { "Resume" } else { "Play" }, IconName::Play, move |_, cx| actions::play(&it, cx)));
        if resume {
            let it = item.clone();
            menu = menu.item(entry("Play from beginning", IconName::RotateCcw, move |_, cx| {
                actions::play_with(&it, PlayOptions { resume: false, ..Default::default() }, cx)
            }));
        }
        if item.is_container() {
            let it = item.clone();
            menu = menu.item(entry("Shuffle", IconName::Shuffle, move |_, cx| {
                actions::play_with(&it, PlayOptions { shuffle: true, ..Default::default() }, cx)
            }));
        }
        if queueable {
            let it = item.clone();
            menu = menu.item(entry("Play next", IconName::ListStart, move |_, cx| actions::enqueue(&it, true, cx)));
            let it = item.clone();
            menu = menu.item(entry("Add to queue", IconName::ListEnd, move |_, cx| actions::enqueue(&it, false, cx)));
        }
        if matches!(kind.as_str(), "artist" | "album" | "track") {
            let it = item.clone();
            menu = menu.item(entry("Start radio", IconName::Radio, move |_, cx| {
                actions::play_with(&it, PlayOptions { radio: true, ..Default::default() }, cx)
            }));
        }
        menu = menu.separator();
    }

    if matches!(kind.as_str(), "episode" | "track")
        && let Some(show) = item.grandparent_rating_key.clone()
    {
        let server = foreign.clone();
        let (label, icon) = if kind == "track" { ("Go to artist", IconName::User) } else { ("Go to show", IconName::Tv) };
        menu = menu.item(entry(label, icon, move |_, cx| nav::push(Route::Item { rating_key: show.clone(), server: server.clone() }, cx)));
    }
    if matches!(kind.as_str(), "episode" | "track" | "season")
        && let Some(parent) = item.parent_rating_key.clone()
    {
        let server = foreign.clone();
        let label = match kind.as_str() {
            "episode" => "Go to season",
            "track" => "Go to album",
            _ => "Go to show",
        };
        menu = menu.item(entry(label, IconName::Layers, move |_, cx| nav::push(Route::Item { rating_key: parent.clone(), server: server.clone() }, cx)));
    }

    if format::has_watch_state(item) {
        let it = item.clone();
        menu = menu.item(entry(
            if watched { "Mark unwatched" } else { "Mark watched" },
            if watched { IconName::Circle } else { IconName::CircleCheck },
            move |_, cx| actions::set_watched(&it, !watched, cx),
        ));
    }

    if foreign.is_none() && (item.is_playable() || matches!(kind.as_str(), "show" | "season" | "album" | "artist" | "playlist")) {
        let it = item.clone();
        menu = menu.item(entry("Download", IconName::Download, move |_, cx| actions::download(&it, cx)));
    }

    menu
}
