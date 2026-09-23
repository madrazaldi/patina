use dioxus::prelude::*;
use patina_plex::{Metadata, Params};

use crate::fmt;
use crate::state::use_app;
use crate::ui::components::{Empty, ErrorView, MediaCard, Shape, Spinner};
use crate::ui::icons::Ic;
use crate::ui::use_scroll;

const PAGE: u32 = 100;

/// Any server listing by key: "See all" for hubs, tag filters (genre, actor, ...).
#[component]
pub fn BrowsePage(path: String, title: String) -> Element {
    let key = fmt::hex_decode(&path);
    let title = fmt::hex_decode(&title);
    rsx! {
        div { class: "page",
            div { class: "page-head", h1 { "{title}" } }
            Listing { key: "{key}", path: key.clone() }
        }
    }
}

#[component]
fn Listing(path: String) -> Element {
    let app = use_app();
    let mut items = use_signal(Vec::<Metadata>::new);
    let mut total = use_signal(|| None::<i64>);
    let mut loading = use_signal(|| true);
    let mut error = use_signal(|| None::<String>);
    let scroll = use_scroll();

    let mut load = move |path: String, start: u32| {
        loading.set(true);
        spawn(async move {
            let p = Params::new().set("X-Plex-Container-Start", start).set("X-Plex-Container-Size", PAGE);
            match app.server().fetch(&path, &p).await {
                Ok(mc) => {
                    total.set(mc.total_size.or(Some(start as i64 + mc.metadata.len() as i64)));
                    let got = mc.metadata.len();
                    items.write().extend(mc.metadata);
                    // Some endpoints ignore paging; stop when a page comes back short.
                    if (got as u32) < PAGE {
                        total.set(Some(items.peek().len() as i64));
                    }
                }
                Err(e) => error.set(Some(e.to_string())),
            }
            loading.set(false);
        });
    };

    let first = path.clone();
    use_hook(move || load(first, 0));
    let next = path.clone();
    use_effect(move || {
        let (top, height, client) = scroll();
        let have = items.peek().len() as i64;
        if !*loading.peek() && have < total.peek().unwrap_or(0) && height - top - client < 1400.0 {
            load(next.clone(), have as u32);
        }
    });

    if let Some(e) = error() {
        return rsx! { ErrorView { message: e } };
    }
    if items.read().is_empty() {
        return if loading() { rsx! { Spinner {} } } else { rsx! { Empty { icon: Ic::Search, title: "Nothing here" } } };
    }
    let landscape = items.read().first().map(|m| Shape::for_item(m) == Shape::Landscape).unwrap_or(false);
    rsx! {
        div { class: if landscape { "grid wide" } else { "grid" },
            for item in items.read().iter() {
                MediaCard { key: "{item.rating_key():?}{item.key:?}", item: item.clone() }
            }
        }
        if loading() { Spinner {} }
    }
}
