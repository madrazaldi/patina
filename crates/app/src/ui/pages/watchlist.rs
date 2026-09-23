use dioxus::prelude::*;
use patina_plex::Metadata;

use crate::fmt;
use crate::images;
use crate::state::use_app;
use crate::ui::Route;
use crate::ui::components::{Empty, ErrorView, Img, Spinner};
use crate::ui::icons::{Ic, Icon};

#[component]
pub fn WatchlistPage() -> Element {
    let app = use_app();
    let list = use_resource(move || {
        let _ = (app.revision)();
        async move {
            let Some(tv) = app.tv() else { return Ok(Vec::new()) };
            tv.watchlist(0, 300).await.map(|mc| mc.metadata)
        }
    });
    rsx! {
        div { class: "page",
            div { class: "page-head", div { h1 { "Watchlist" } div { class: "sub", "Synced with your Plex account — titles you don't have yet are shown dimmed." } } }
            match &*list.read() {
                None => rsx! { Spinner {} },
                Some(Err(e)) => rsx! { ErrorView { message: e.to_string() } },
                Some(Ok(l)) if l.is_empty() => rsx! { Empty { icon: Ic::Bookmark, title: "Your watchlist is empty", text: "Add movies and shows from their detail page." } },
                Some(Ok(l)) => rsx! {
                    div { class: "grid",
                        for m in l.iter() { WatchCard { key: "{m.rating_key():?}", item: m.clone() } }
                    }
                },
            }
        }
    }
}

/// A watchlist entry. Resolves to the local library copy (by guid) when clicked.
#[component]
fn WatchCard(item: Metadata) -> Element {
    let app = use_app();
    let nav = use_navigator();
    let guid = item.guid.clone();
    let local = use_resource(move || {
        let guid = guid.clone();
        async move {
            let guid = guid?;
            app.server().find_by_guid(&guid).await.ok()?.into_iter().next()
        }
    });
    let available = local.read().clone().flatten();
    let src = images::opt(item.thumb.as_deref(), 300, 450);
    let discover_id = item.rating_key().to_string();
    let sub = fmt::subtitle(&item);
    let title = item.title().to_string();
    rsx! {
        div { class: "card", style: if available.is_none() && local.read().is_some() { "opacity: .55" } else { "" },
            onclick: move |_| match &available {
                Some(m) => { nav.push(Route::ItemPage { rating_key: m.rating_key().to_string() }); }
                None => app.toast("Not available on this server"),
            },
            div { class: "card-art",
                Img { src }
                button { class: "card-more", title: "Remove from watchlist",
                    onclick: move |e| {
                        e.stop_propagation();
                        let id = discover_id.clone();
                        spawn(async move {
                            let Some(tv) = app.tv() else { return };
                            match tv.remove_from_watchlist(&id).await {
                                Ok(()) => { app.toast("Removed from watchlist"); app.bump(); }
                                Err(e) => app.error(e),
                            }
                        });
                    },
                    Icon { name: Ic::X, class: "sm" }
                }
            }
            div { class: "card-title", "{title}" }
            div { class: "card-sub", "{sub}" }
        }
    }
}
