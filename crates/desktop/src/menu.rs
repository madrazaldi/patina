//! Right-click / "…" menu for any item.

use gpui_kit::assets::IconName;
use gpui_kit::component::menu::{PopupMenu, PopupMenuItem};
use gpui_kit::*;
use patina_core::downloads::{self, Download, State};
use patina_core::undo;
use patina_plex::Metadata;

use crate::actions::{self, PlayOptions};
use crate::core::core;
use crate::dialogs;
use crate::format;
use crate::nav::{self, Route};
use crate::store::{self, Store, store};
use crate::widgets;

fn entry(label: impl Into<SharedString>, icon: IconName, f: impl Fn(&mut Window, &mut App) + 'static) -> PopupMenuItem {
    PopupMenuItem::new(label).icon(icon).on_click(move |_, window, cx| f(window, cx))
}

/// Kinds that live in a library section (and can be edited or deleted).
const LIBRARY: &[&str] = &["movie", "show", "season", "episode", "artist", "album", "track", "collection"];

/// Kinds that can be downloaded.
pub const DOWNLOADABLE: &[&str] = &["movie", "episode", "track", "show", "season", "album", "artist"];

/// The download of an item on the connected server and account, in any state.
pub fn saved(s: &Store, item: &Metadata) -> Option<Download> {
    let session = s.session.as_ref()?;
    s.downloads
        .iter()
        .find(|d| d.server_id == session.server_id && (d.account.is_empty() || d.account == session.account_id) && d.item.rating_key == item.rating_key)
        .cloned()
}

/// The item's page on Plex Web.
fn plex_web(machine: &str, rating_key: &str) -> String {
    format!("https://app.plex.tv/desktop#!/server/{machine}/details?key=%2Flibrary%2Fmetadata%2F{rating_key}")
}

/// Run a server operation on an item, toasting `ok` and refreshing the views.
fn server_op<F>(cx: &mut App, ok: Option<&str>, refresh: bool, f: impl FnOnce(patina_plex::PlexServer) -> F + Send + 'static)
where
    F: Future<Output = Result<(), patina_plex::Error>> + Send + 'static,
{
    let app = core(cx);
    store::act(cx, async move { Ok::<_, anyhow::Error>(f(app.server()?).await?) }, ok.map(str::to_string), refresh);
}

/// Ask, then delete an item's media (or a collection) from the server.
pub fn delete(item: &Metadata, window: &mut Window, cx: &mut App) {
    let rk = item.rating_key().to_string();
    let collection = item.kind() == "collection";
    let scope = store(cx).read(cx).scope();
    let text = if collection { "The collection is removed; its items stay in the library." } else { "This deletes the media files from disk. It can't be undone." };
    widgets::confirm(format!("Delete “{}”?", item.title()), Some(text.into()), "Delete", true, window, cx, move |_, cx| {
        // The rating key belongs to the server the menu opened on.
        if store(cx).read(cx).scope() != scope {
            return;
        }
        let on_page = nav::is(&Route::Item { rating_key: rk.clone(), server: None }, cx);
        let app = core(cx);
        let rk = rk.clone();
        store::act_then(
            cx,
            async move {
                let server = app.server()?;
                if collection { server.delete_collection(&rk).await? } else { server.delete_metadata(&rk).await? }
                Ok(())
            },
            move |(), cx| {
                store::toast(cx, "Deleted");
                store(cx).update(cx, |s, cx| s.refresh(cx));
                if on_page {
                    nav::back(cx);
                }
            },
        );
    });
}

/// Fill `menu` with what can be done to `item`.
pub fn item_menu(menu: PopupMenu, item: &Metadata, _window: &mut Window, cx: &mut Context<PopupMenu>) -> PopupMenu {
    let kind = item.kind().to_string();
    let rk = item.rating_key().to_string();
    let resume = item.progress().is_some();
    let watched = item.is_watched();
    let queueable = format::can_play(item) && !matches!(kind.as_str(), "photo" | "photoalbum");
    let foreign = item.server_id().map(str::to_string);
    let s = store(cx).read(cx);
    let owned = s.owned();
    let discover = if s.has_tv() { item.discover_id().map(str::to_string) } else { None };
    let machine = s.session.as_ref().and_then(|x| x.machine_identifier.clone());
    let saved = saved(s, item);
    let place = foreign.as_ref().map(|id| s.session.as_ref().and_then(|x| x.servers.iter().find(|v| v.id == *id)).map_or("another server".to_string(), |v| v.name.clone()));
    let in_library = LIBRARY.contains(&kind.as_str());

    let mut menu = menu.min_w(px(220.)).label(item.full_title());
    let sub = format::subtitle(item);
    if !sub.is_empty() {
        menu = menu.label(sub);
    }
    if let Some(place) = place {
        menu = menu.label(format!("On {place}"));
    }

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
            if watched { "Mark as unwatched" } else { "Mark as watched" },
            if watched { IconName::Circle } else { IconName::CircleCheck },
            move |_, cx| actions::set_watched(&it, !watched, cx),
        ));
    }

    // What follows edits the connected server, so it is not offered for an item found on another one.
    if foreign.is_some() {
        return menu;
    }
    if resume {
        let rk = rk.clone();
        menu = menu.item(entry("Clear progress", IconName::Eraser, move |_, cx| {
            let (app, rk) = (core(cx), rk.clone());
            store::act(cx, async move { undo::clear_progress(&app, &rk).await }, None, false);
        }));
    }
    if kind == "episode" || (kind == "movie" && resume) {
        let rk = rk.clone();
        menu = menu.item(entry("Remove from Continue Watching", IconName::EyeOff, move |_, cx| {
            let (app, rk) = (core(cx), rk.clone());
            store::act(cx, async move { undo::remove_from_continue_watching(&app, &rk).await }, None, false);
        }));
    }
    if kind != "playlist" && kind != "collection" {
        let it = item.clone();
        menu = menu.item(entry("Add to playlist…", IconName::ListPlus, move |window, cx| dialogs::playlist::open(vec![it.clone()], window, cx)));
    }
    if owned && matches!(kind.as_str(), "movie" | "show" | "artist" | "album") {
        let it = item.clone();
        menu = menu.item(entry("Add to collection…", IconName::FolderPlus, move |window, cx| dialogs::collection::open(it.clone(), window, cx)));
    }
    if format::supports_tracks(item) {
        let it = item.clone();
        menu = menu.item(entry("Audio & subtitles…", IconName::Captions, move |window, cx| dialogs::tracks::open(it.clone(), window, cx)));
        let it = item.clone();
        menu = menu.item(entry("Playback preferences…", IconName::SlidersHorizontal, move |window, cx| dialogs::show_prefs::open(it.clone(), window, cx)));
    }
    if let Some(id) = discover {
        menu = menu.item(entry("Add to watchlist", IconName::Bookmark, move |_, cx| {
            let (app, id) = (core(cx), id.clone());
            store::act(cx, async move { Ok::<_, anyhow::Error>(app.plex_tv()?.add_to_watchlist(&id).await?) }, Some("Added to watchlist".into()), false);
        }));
    }
    if DOWNLOADABLE.contains(&kind.as_str()) {
        menu = match &saved {
            Some(d) if matches!(d.state, State::Done) => menu.item(PopupMenuItem::new("Downloaded").icon(IconName::ArrowDownToLine).disabled(true)),
            Some(d) if matches!(d.state, State::Paused | State::Failed) => {
                let id = d.id.clone();
                menu.item(entry("Resume / retry download", IconName::Download, move |_, cx| downloads::retry(&core(cx), &id)))
            }
            Some(_) => menu.item(PopupMenuItem::new("Downloading…").icon(IconName::Download).disabled(true)),
            None => {
                let it = item.clone();
                menu.item(entry("Download", IconName::Download, move |_, cx| actions::download(&it, cx)))
            }
        };
    }
    if matches!(kind.as_str(), "show" | "season" | "playlist") {
        let it = item.clone();
        menu = menu.item(entry("Keep next unwatched downloaded…", IconName::CloudDownload, move |window, cx| dialogs::keep::open(it.clone(), window, cx)));
    }

    if owned && kind != "playlist" {
        menu = menu.separator();
        if in_library {
            let it = item.clone();
            menu = menu.item(entry("Edit…", IconName::Pencil, move |window, cx| dialogs::edit::open(it.clone(), window, cx)));
            if kind != "track" {
                let it = item.clone();
                menu = menu.item(entry("Choose poster…", IconName::Image, move |window, cx| {
                    dialogs::artwork::open(it.clone(), dialogs::artwork::Kind::Poster, window, cx)
                }));
            }
            if matches!(kind.as_str(), "movie" | "show" | "artist" | "album" | "collection") {
                let it = item.clone();
                menu = menu.item(entry("Choose background…", IconName::Wallpaper, move |window, cx| {
                    dialogs::artwork::open(it.clone(), dialogs::artwork::Kind::Background, window, cx)
                }));
            }
            if matches!(kind.as_str(), "movie" | "show" | "artist" | "album") {
                let it = item.clone();
                menu = menu.item(entry("Fix match…", IconName::Crosshair, move |window, cx| dialogs::matching::open(it.clone(), window, cx)));
            }
        }
        let k = rk.clone();
        menu = menu.item(entry("Refresh metadata", IconName::RefreshCw, move |_, cx| {
            let k = k.clone();
            server_op(cx, Some("Refreshing metadata…"), false, move |s| async move { s.refresh_metadata(&k, true).await });
        }));
        let k = rk.clone();
        menu = menu.item(entry("Analyze", IconName::ScanLine, move |_, cx| {
            let k = k.clone();
            server_op(cx, Some("Analyzing…"), false, move |s| async move { s.analyze_metadata(&k).await });
        }));
        if in_library {
            let it = item.clone();
            menu = menu.item(entry("Delete…", IconName::Trash, move |window, cx| delete(&it, window, cx)));
        }
    }
    if let Some(machine) = machine {
        let url = plex_web(&machine, &rk);
        menu = menu.item(entry("Open in Plex Web", IconName::ExternalLink, move |_, cx| actions::open_url(&url, cx)));
    }
    menu
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn plex_web_link_encodes_the_key() {
        assert_eq!(plex_web("abc", "100"), "https://app.plex.tv/desktop#!/server/abc/details?key=%2Flibrary%2Fmetadata%2F100");
    }
}
