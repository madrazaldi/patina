use dioxus::prelude::*;
use patina_plex::{Directory, LibraryQuery, Metadata, Params};

use crate::state::use_app;
use crate::ui::components::{Empty, ErrorView, HubRow, MediaCard, Shape, Spinner};
use crate::ui::icons::{Ic, Icon, section_icon};
use crate::ui::use_scroll;

const PAGE: u32 = 120;

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Browse,
    Recommended,
    Collections,
}

#[component]
pub fn LibraryPage(id: String) -> Element {
    let app = use_app();
    let mut tab = use_signal(|| Tab::Browse);
    use_effect(use_reactive((&id,), move |_| tab.set(Tab::Browse)));
    let Some(section) = app.sections.read().iter().find(|s| s.key.as_deref() == Some(id.as_str())).cloned() else {
        return rsx! { div { class: "page", Empty { icon: Ic::Folder, title: "Library not found" } } };
    };
    let kind = section.kind.clone().unwrap_or_default();
    let owned = app.session.read().as_ref().map(|s| s.owned).unwrap_or(false);
    let scan_id = id.clone();

    rsx! {
        div { class: "page",
            div { class: "page-head",
                div {
                    h1 { class: "flex", Icon { name: section_icon(&kind), class: "lg" } "{section.title()}" }
                }
                div { class: "toolbar",
                    div { class: "tabs",
                        for (t, label) in [(Tab::Browse, "Library"), (Tab::Recommended, "Recommended"), (Tab::Collections, "Collections")] {
                            div { class: if tab() == t { "tab active" } else { "tab" }, onclick: move |_| tab.set(t), "{label}" }
                        }
                    }
                    if owned {
                        button { class: "icon-btn", title: "Scan library files",
                            onclick: move |_| {
                                let id = scan_id.clone();
                                spawn(async move {
                                    match app.server().refresh_section(&id, false, None).await {
                                        Ok(()) => app.toast("Scanning library…"),
                                        Err(e) => app.error(e),
                                    }
                                });
                            },
                            Icon { name: Ic::Refresh, class: "sm" }
                        }
                    }
                }
            }
            match tab() {
                Tab::Browse => rsx! { Browse { key: "{id}", section: section.clone() } },
                Tab::Recommended => rsx! { Recommended { key: "{id}", id: id.clone() } },
                Tab::Collections => rsx! { Collections { key: "{id}", id: id.clone() } },
            }
        }
    }
}

/// Media types a library can be browsed by.
fn types_for(kind: &str) -> Vec<(u32, &'static str)> {
    match kind {
        "show" => vec![(2, "Shows"), (3, "Seasons"), (4, "Episodes")],
        "artist" => vec![(8, "Artists"), (9, "Albums"), (10, "Tracks")],
        "photo" => vec![],
        _ => vec![],
    }
}

#[component]
fn Browse(section: Directory) -> Element {
    let app = use_app();
    let id = section.key.clone().unwrap_or_default();
    let kind = section.kind.clone().unwrap_or_default();
    let types = types_for(&kind);

    let mut sort = use_signal(|| "titleSort".to_string());
    let mut desc = use_signal(|| false);
    let mut unwatched = use_signal(|| false);
    let mut genre = use_signal(|| None::<String>);
    let first_type = types.first().map(|t| t.0);
    let mut media_type = use_signal(move || first_type);
    let mut items = use_signal(Vec::<Metadata>::new);
    let mut total = use_signal(|| None::<i64>);
    let mut loading = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let mut generation = use_signal(|| 0u64);
    let scroll = use_scroll();

    let sid = id.clone();
    let sorts = use_resource(move || {
        let id = sid.clone();
        async move { app.server().section_sorts(&id).await.unwrap_or_default() }
    });
    let gid = id.clone();
    let genres = use_resource(move || {
        let id = gid.clone();
        let t = media_type();
        async move { app.server().section_filter_values(&id, "genre", t).await.unwrap_or_default() }
    });

    let query = move || {
        let sort_key = sort();
        let sort_value = if desc() {
            sorts
                .read()
                .as_ref()
                .and_then(|s| s.iter().find(|d| d.key.as_deref() == Some(sort_key.as_str())))
                .and_then(|d| d.desc_key.clone())
                .unwrap_or_else(|| format!("{sort_key}:desc"))
        } else {
            sort_key
        };
        LibraryQuery {
            kind: media_type(),
            sort: Some(sort_value),
            unwatched: unwatched(),
            filters: genre().map(|g| Params::new().set("genre", g)).unwrap_or_default(),
            start: 0,
            size: PAGE,
        }
    };

    let lid = id.clone();
    let load = move |start: u32, gen_at: u64, q: LibraryQuery| {
        let id = lid.clone();
        loading.set(true);
        spawn(async move {
            let q = LibraryQuery { start, ..q };
            let result = app.server().section_items(&id, &q).await;
            if *generation.peek() != gen_at {
                return;
            }
            loading.set(false);
            match result {
                Ok(mc) => {
                    total.set(mc.total_size.or(mc.size));
                    items.write().extend(mc.metadata);
                }
                Err(e) => error.set(Some(e.to_string())),
            }
        });
    };

    // Reload from the top whenever the query (or watch state) changes.
    let mut reload = load.clone();
    use_effect(move || {
        let q = query();
        let _ = (app.revision)();
        let gen_at = *generation.peek() + 1;
        generation.set(gen_at);
        items.set(Vec::new());
        total.set(None);
        error.set(None);
        reload(0, gen_at, q);
    });

    // Infinite scroll.
    let mut more = load.clone();
    use_effect(move || {
        let (top, height, client) = scroll();
        let have = items.peek().len() as i64;
        let all = total.peek().unwrap_or(0);
        if !*loading.peek() && have > 0 && have < all && height - top - client < 1400.0 {
            more(have as u32, *generation.peek(), query());
        }
    });

    let count_label = total().map(|n| {
        let noun = types
            .iter()
            .find(|t| Some(t.0) == media_type())
            .map(|t| t.1.to_lowercase())
            .unwrap_or_else(|| "items".into());
        format!("{} {noun}", group_digits(n))
    });
    let shape = match media_type() {
        Some(4) => Some(Shape::Landscape),
        Some(8..=10) => Some(Shape::Square),
        _ => None,
    };
    let wide = shape == Some(Shape::Landscape);

    rsx! {
        div { class: "toolbar", style: "margin-bottom: 22px",
            if types.len() > 1 {
                div { class: "tabs",
                    for (t, label) in types.clone() {
                        div { class: if media_type() == Some(t) { "tab active" } else { "tab" },
                            onclick: move |_| { media_type.set(Some(t)); genre.set(None); }, "{label}" }
                    }
                }
            }
            select { class: "select pill", value: "{sort}",
                onchange: move |e| sort.set(e.value()),
                match &*sorts.read() {
                    Some(list) if !list.is_empty() => rsx! {
                        for s in list.clone() {
                            option { key: "{s.key:?}", value: "{s.key.clone().unwrap_or_default()}", "{s.title()}" }
                        }
                    },
                    _ => rsx! {
                        option { value: "titleSort", "Title" }
                        option { value: "addedAt", "Date added" }
                        option { value: "originallyAvailableAt", "Release date" }
                        option { value: "rating", "Rating" }
                    },
                }
            }
            button { class: "icon-btn", title: if desc() { "Descending" } else { "Ascending" }, onclick: move |_| desc.toggle(),
                Icon { name: Ic::Sort, class: "sm" }
            }
            select { class: "select pill",
                value: "{genre().unwrap_or_default()}",
                onchange: move |e| { let v = e.value(); genre.set((!v.is_empty()).then_some(v)); },
                option { value: "", "All genres" }
                if let Some(list) = &*genres.read() {
                    for g in list.clone() {
                        option { key: "{g.key:?}", value: "{g.key.clone().unwrap_or_default()}", "{g.title()}" }
                    }
                }
            }
            if kind != "artist" && kind != "photo" {
                div { class: if unwatched() { "chip active" } else { "chip" }, onclick: move |_| unwatched.toggle(),
                    Icon { name: Ic::Eye, class: "sm" } "Unwatched"
                }
            }
            div { class: "grow" }
            if let Some(label) = count_label { span { class: "muted", "{label}" } }
        }
        if let Some(e) = error() {
            ErrorView { message: e }
        } else if items.read().is_empty() && !loading() {
            Empty { icon: Ic::Search, title: "No matches", text: "Try a different filter." }
        } else {
            div { class: if wide { "grid wide" } else { "grid" },
                for item in items.read().iter() {
                    MediaCard { key: "{item.rating_key():?}", item: item.clone(), shape }
                }
            }
            if loading() { Spinner {} }
        }
    }
}

fn group_digits(n: i64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[component]
fn Recommended(id: String) -> Element {
    let app = use_app();
    let hubs = use_resource(move || {
        let id = id.clone();
        let _ = (app.revision)();
        async move { app.server().section_hubs(&id, 18).await }
    });
    match &*hubs.read() {
        None => rsx! { Spinner {} },
        Some(Err(e)) => rsx! { ErrorView { message: e.to_string() } },
        Some(Ok(h)) if h.iter().all(|h| h.is_empty()) => rsx! { Empty { icon: Ic::Sparkles, title: "No recommendations yet" } },
        Some(Ok(h)) => rsx! {
            for hub in h.iter().filter(|h| !h.is_empty()) {
                HubRow { key: "{hub.hub_identifier:?}", hub: hub.clone() }
            }
        },
    }
}

#[component]
fn Collections(id: String) -> Element {
    let app = use_app();
    let cols = use_resource(move || {
        let id = id.clone();
        async move { app.server().collections(&id).await }
    });
    match &*cols.read() {
        None => rsx! { Spinner {} },
        Some(Err(e)) => rsx! { ErrorView { message: e.to_string() } },
        Some(Ok(c)) if c.is_empty() => rsx! { Empty { icon: Ic::Layers, title: "No collections", text: "Collections you create in Plex show up here." } },
        Some(Ok(c)) => rsx! {
            div { class: "grid",
                for item in c.iter() {
                    MediaCard { key: "{item.rating_key():?}", item: item.clone() }
                }
            }
        },
    }
}
