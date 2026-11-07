//! A library section: the browsable, filterable grid of its titles, plus its
//! recommended hubs, collections and folders.

mod cache;
mod picker;

use crate::prelude::*;
use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::skeleton::Skeleton;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, h_flex, v_flex};
use patina_core::ops::library::{self as lib, FilterView, FolderView};
use patina_plex::{Directory, Hub, LibraryQuery, Metadata, Params};
use serde::{Deserialize, Serialize};

use self::cache::{Fetch, PAGE, Pages};
use self::picker::FilterPicker;
use crate::core::{Load, bg, core, message};
use crate::format::{self, Shape};
use crate::store::{self, store};
use crate::theme::GUTTER;
use crate::widgets::{self, Card, hub::HubRow};

const LETTERS: &str = "#ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const GAP: f32 = 20.;

/// The media types a section lists, for sections with more than one.
fn types_of(kind: &str) -> &'static [(u32, &'static str)] {
    match kind {
        "show" => &[(2, "Shows"), (3, "Seasons"), (4, "Episodes")],
        "artist" => &[(8, "Artists"), (9, "Albums"), (10, "Tracks")],
        _ => &[],
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tab_ {
    Browse,
    Recommended,
    Collections,
    Folders,
}

/// A value filter that is on ("Genre: Drama").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Picked {
    pub filter: String,
    pub title: String,
    pub value: String,
    pub label: String,
}

/// A saved filter/sort combination, stored in `ui.saved_views` (shared with the TUI).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedView {
    pub id: String,
    pub name: String,
    pub scope: String,
    pub library: String,
    #[serde(default)]
    pub media_type: u32,
    #[serde(default)]
    pub sort: String,
    #[serde(default)]
    pub desc: bool,
    #[serde(default)]
    pub unwatched: bool,
    #[serde(default)]
    pub toggles: Vec<String>,
    #[serde(default)]
    pub picked: Vec<Picked>,
}

/// The saved views of one library.
pub fn saved_views(values: &[serde_json::Value], scope: &str, library: &str) -> Vec<SavedView> {
    values
        .iter()
        .filter_map(|v| serde_json::from_value::<SavedView>(v.clone()).ok())
        .filter(|v| v.scope == scope && v.library == library)
        .collect()
}

/// The sort key to send: the sort's descending key when descending.
fn sort_value(sort: &str, desc: bool, sorts: &[Directory]) -> String {
    if !desc {
        return sort.to_string();
    }
    sorts.iter().find(|d| d.key.as_deref() == Some(sort)).and_then(|d| d.desc_key.clone()).unwrap_or_else(|| format!("{sort}:desc"))
}

/// Index of the first item under each letter present, from the section's first characters.
fn letter_starts(chars: &[Directory]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    let mut n = 0usize;
    for c in chars {
        let letter = c.title.clone().or_else(|| c.key.clone()).unwrap_or_default().to_uppercase();
        if !out.iter().any(|(l, _)| *l == letter) {
            out.push((letter, n));
        }
        n += c.size.unwrap_or(0).max(0) as usize;
    }
    out
}

/// Grid columns and row height for a width.
fn grid_metrics(width: f32, poster: f32, shape: Shape) -> (usize, f32) {
    let card = if shape == Shape::Landscape { poster * 1.75 } else { poster };
    let columns = (((width + GAP) / (card + GAP)).floor() as usize).max(1);
    let cell = (width - (columns as f32 - 1.) * GAP) / columns as f32;
    let factor = match shape {
        Shape::Poster => 1.5,
        Shape::Landscape => 9. / 16.,
        Shape::Square => 1.,
    };
    (columns, cell * factor + 64.)
}

pub struct Library {
    id: String,
    scope: String,
    tab: Tab_,
    media_type: Option<u32>,
    sort: String,
    desc: bool,
    unwatched: bool,
    toggles: Vec<String>,
    picked: Vec<Picked>,
    sorts: Vec<Directory>,
    filters: Vec<FilterView>,
    pages: Pages,
    chars: Vec<Directory>,
    hubs: Load<Vec<Hub>>,
    collections: Load<Vec<Metadata>>,
    trail: Vec<(Option<String>, String)>,
    folders: Load<FolderView>,
    revision: u64,
    selecting: bool,
    selection: Vec<Metadata>,
    bulk: Bulk,
    view_name: Entity<InputState>,
    picker: Option<Entity<FilterPicker>>,
    scroll: UniformListScrollHandle,
    width: Rc<Cell<f32>>,
    _subs: Vec<Subscription>,
}

#[derive(Default)]
struct Bulk {
    busy: bool,
    done: usize,
    failures: Vec<String>,
}

impl Library {
    pub fn new(id: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let shared = store(cx);
        let (scope, kind, revision) = {
            let s = shared.read(cx);
            (s.scope(), s.sections.iter().find(|d| d.key.as_deref() == Some(id.as_str())).and_then(|d| d.kind.clone()).unwrap_or_default(), s.revision)
        };
        let view_name = cx.new(|cx| InputState::new(window, cx).placeholder("Name this view"));
        let subs = vec![
            cx.observe(&shared, |this: &mut Self, store, cx| {
                let s = store.read(cx);
                if s.revision != this.revision {
                    let since = this.revision;
                    let affected = s.affects(&this.id, since);
                    this.revision = s.revision;
                    if affected {
                        this.reload(cx);
                    }
                }
                cx.notify();
            }),
            cx.subscribe_in(&view_name, window, |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::PressEnter { .. } => this.save_view(window, cx),
                InputEvent::Change => cx.notify(),
                _ => {}
            }),
        ];
        let mut this = Self {
            media_type: types_of(&kind).first().map(|t| t.0),
            id,
            scope,
            tab: Tab_::Browse,
            sort: "titleSort".into(),
            desc: false,
            unwatched: false,
            toggles: Vec::new(),
            picked: Vec::new(),
            sorts: Vec::new(),
            filters: Vec::new(),
            pages: Pages::default(),
            chars: Vec::new(),
            hubs: Load::Loading,
            collections: Load::Loading,
            trail: Vec::new(),
            folders: Load::Loading,
            revision,
            selecting: false,
            selection: Vec::new(),
            bulk: Bulk::default(),
            view_name,
            picker: None,
            scroll: UniformListScrollHandle::new(),
            width: Rc::new(Cell::new(0.)),
            _subs: subs,
        };
        this.load_sorts(cx);
        this.query_changed(cx);
        this
    }

    fn section<'a>(&self, cx: &'a App) -> Option<&'a Directory> {
        store(cx).read(cx).sections.iter().find(|d| d.key.as_deref() == Some(self.id.as_str()))
    }

    fn kind(&self, cx: &App) -> String {
        self.section(cx).and_then(|d| d.kind.clone()).unwrap_or_default()
    }

    fn filtered(&self) -> bool {
        self.unwatched || !self.toggles.is_empty() || !self.picked.is_empty()
    }

    fn rail_on(&self) -> bool {
        self.sort.starts_with("titleSort") && !self.desc && !self.filtered()
    }

    fn shape(&self) -> Option<Shape> {
        match self.media_type {
            Some(4) => Some(Shape::Landscape),
            Some(t) if t >= 8 => Some(Shape::Square),
            _ => None,
        }
    }

    fn query(&self) -> LibraryQuery {
        let mut filters = Params::new();
        for t in &self.toggles {
            filters.push(t.clone(), "1");
        }
        for p in &self.picked {
            filters.push(p.filter.clone(), p.value.clone());
        }
        LibraryQuery {
            kind: self.media_type,
            sort: Some(sort_value(&self.sort, self.desc, &self.sorts)),
            unwatched: self.unwatched,
            filters,
            start: 0,
            size: 0,
        }
    }

    fn query_key(&self) -> String {
        let q = self.query();
        format!("{:?}|{:?}|{}|{}", q.kind, q.sort, q.unwatched, q.filters.to_query_string())
    }

    /// The listing's query changed: start over, and refetch what depends on it.
    fn query_changed(&mut self, cx: &mut Context<Self>) {
        let key = self.query_key();
        if self.pages.key != key {
            if !self.pages.key.is_empty() {
                self.selection.clear();
            }
            self.pages.reset(key);
            self.scroll.scroll_to_item(0, ScrollStrategy::Top);
            let first = self.pages.ensure(0, PAGE);
            self.fetch(first, cx);
        }
        self.load_filters(cx);
        self.load_chars(cx);
        cx.notify();
    }

    /// Something on the server changed: refresh the open tab.
    fn reload(&mut self, cx: &mut Context<Self>) {
        match self.tab {
            Tab_::Browse => {
                let again = self.pages.refresh();
                self.fetch(again, cx);
            }
            Tab_::Recommended => self.load_hubs(cx),
            Tab_::Collections => self.load_collections(cx),
            Tab_::Folders => self.load_folders(cx),
        }
    }

    fn fetch(&mut self, list: Vec<Fetch>, cx: &mut Context<Self>) {
        for f in list {
            let app = core(cx);
            let id = self.id.clone();
            let mut q = self.query();
            q.start = f.offset as u32;
            q.size = PAGE as u32;
            cx.spawn(async move |this, cx| {
                let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.section_items(&id, &q).await?) }).await;
                this.update(cx, |l, cx| {
                    match result {
                        Ok(mc) => l.pages.loaded(f, mc.metadata, mc.total_size.map(|n| n.max(0) as usize)),
                        Err(e) => l.pages.failed(f, message(&e)),
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }

    fn load_sorts(&mut self, cx: &mut Context<Self>) {
        let (app, id) = (core(cx), self.id.clone());
        cx.spawn(async move |this, cx| {
            let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.section_sorts(&id).await?) }).await;
            if let Ok(sorts) = result {
                this.update(cx, |l, cx| {
                    l.sorts = sorts;
                    // A descending sort's key may differ from `<sort>:desc`.
                    if l.desc {
                        l.query_changed(cx);
                    }
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn load_filters(&mut self, cx: &mut Context<Self>) {
        let (app, id, kind) = (core(cx), self.id.clone(), self.media_type);
        cx.spawn(async move |this, cx| {
            if let Ok(filters) = bg(async move { lib::section_filters(&app, &id, kind).await }).await {
                this.update(cx, |l, cx| {
                    if l.media_type == kind {
                        l.filters = filters;
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn load_chars(&mut self, cx: &mut Context<Self>) {
        if !self.rail_on() {
            self.chars.clear();
            return;
        }
        let (app, id, kind) = (core(cx), self.id.clone(), self.media_type);
        cx.spawn(async move |this, cx| {
            if let Ok(chars) = bg(async move { Ok::<_, anyhow::Error>(app.server()?.section_first_characters(&id, kind).await?) }).await {
                this.update(cx, |l, cx| {
                    if l.media_type == kind && l.rail_on() {
                        l.chars = chars;
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn load_hubs(&mut self, cx: &mut Context<Self>) {
        let (app, id) = (core(cx), self.id.clone());
        cx.spawn(async move |this, cx| {
            let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.section_hubs(&id, 18).await?) }).await;
            this.update(cx, |l, cx| {
                l.hubs = keep_stale(std::mem::take(&mut l.hubs), result);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn load_collections(&mut self, cx: &mut Context<Self>) {
        let (app, id) = (core(cx), self.id.clone());
        cx.spawn(async move |this, cx| {
            let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.collections(&id).await?) }).await;
            this.update(cx, |l, cx| {
                l.collections = keep_stale(std::mem::take(&mut l.collections), result);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn load_folders(&mut self, cx: &mut Context<Self>) {
        let (app, id) = (core(cx), self.id.clone());
        let key = self.trail.last().and_then(|(k, _)| k.clone());
        let depth = self.trail.len();
        cx.spawn(async move |this, cx| {
            let result = bg(async move { lib::section_folders(&app, &id, key.as_deref()).await }).await;
            this.update(cx, |l, cx| {
                if l.trail.len() == depth {
                    l.folders = Load::from_result(result);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn set_tab(&mut self, tab: Tab_, cx: &mut Context<Self>) {
        if self.tab == tab {
            return;
        }
        self.tab = tab;
        match tab {
            Tab_::Browse => {}
            Tab_::Recommended => self.load_hubs(cx),
            Tab_::Collections => self.load_collections(cx),
            Tab_::Folders => {
                self.folders = Load::Loading;
                self.load_folders(cx);
            }
        }
        cx.notify();
    }

    fn set_type(&mut self, t: u32, cx: &mut Context<Self>) {
        self.media_type = Some(t);
        self.toggles.clear();
        self.picked.clear();
        self.filters.clear();
        self.query_changed(cx);
    }

    fn save_view(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.view_name.read(cx).value().trim().to_string();
        if name.is_empty() {
            return;
        }
        let view = SavedView {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            scope: self.scope.clone(),
            library: self.id.clone(),
            media_type: self.media_type.unwrap_or(0),
            sort: self.sort.clone(),
            desc: self.desc,
            unwatched: self.unwatched,
            toggles: self.toggles.clone(),
            picked: self.picked.clone(),
        };
        let Ok(value) = serde_json::to_value(&view) else { return };
        store(cx).update(cx, |s, cx| s.update_ui(cx, move |ui| ui.saved_views.push(value)));
        self.view_name.update(cx, |v, cx| v.set_value("", window, cx));
    }

    /// Show a saved view.
    pub fn use_view(&mut self, view: &SavedView, cx: &mut Context<Self>) {
        self.tab = Tab_::Browse;
        self.media_type = if view.media_type == 0 { types_of(&self.kind(cx)).first().map(|t| t.0) } else { Some(view.media_type) };
        self.sort = if view.sort.is_empty() { "titleSort".into() } else { view.sort.clone() };
        self.desc = view.desc;
        self.unwatched = view.unwatched;
        self.toggles = view.toggles.clone();
        self.picked = view.picked.clone();
        self.selection.clear();
        self.query_changed(cx);
    }

    fn remove_view(id: String, cx: &mut App) {
        store(cx).update(cx, |s, cx| {
            s.update_ui(cx, move |ui| ui.saved_views.retain(|v| v.get("id").and_then(|i| i.as_str()) != Some(id.as_str())))
        });
    }

    fn toggle_item(&mut self, item: &Metadata, cx: &mut Context<Self>) {
        let key = item.rating_key();
        if let Some(i) = self.selection.iter().position(|m| m.rating_key() == key) {
            self.selection.remove(i);
        } else {
            self.selection.push(item.clone());
        }
        cx.notify();
    }

    fn open_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let values: Vec<FilterView> = self.filters.iter().filter(|f| f.kind != "boolean").cloned().collect();
        let picked = self.picked.iter().map(|p| (p.filter.clone(), p.value.clone())).collect();
        let me = cx.entity().downgrade();
        let on_pick: picker::OnPick = Rc::new(move |f: &FilterView, value: String, label: String, _: &mut Window, cx: &mut App| {
            let f = f.clone();
            me.update(cx, |l, cx| {
                l.picker = None;
                if !l.picked.iter().any(|p| p.filter == f.filter && p.value == value) {
                    l.picked.push(Picked { filter: f.filter, title: f.title, value, label });
                    l.query_changed(cx);
                }
                cx.notify();
            })
            .ok();
        });
        let (id, kind) = (self.id.clone(), self.media_type);
        self.picker = Some(cx.new(|cx| FilterPicker::new(id, kind, values, picked, on_pick, window, cx)));
        cx.notify();
    }

    /// Queue, download or mark watched everything selected.
    fn run_bulk(&mut self, action: &'static str, cx: &mut Context<Self>) {
        let list = self.selection.clone();
        if list.is_empty() {
            return;
        }
        self.bulk = Bulk { busy: true, done: 0, failures: Vec::new() };
        cx.notify();
        let app = core(cx);
        let scope = self.scope.clone();
        cx.spawn(async move |this, cx| {
            match action {
                "watched" => {
                    let keys: Vec<String> = list.iter().map(|m| m.rating_key().to_string()).collect();
                    let result = bg(async move { patina_core::undo::set_watched(&app, None, &keys, true).await }).await;
                    this.update(cx, |l, cx| {
                        l.bulk.busy = false;
                        match result {
                            Ok(_) => l.selection.clear(),
                            Err(e) => l.bulk.failures.push(message(&e)),
                        }
                        cx.notify();
                    })
                    .ok();
                    return;
                }
                "queue" => {
                    let n = list.len();
                    let result = {
                        let _guard = patina_core::runtime().enter();
                        patina_core::player::queue_items(app.clone(), list.clone())
                    };
                    this.update(cx, |l, _| match result {
                        Ok(()) => l.bulk.done = n,
                        Err(e) => l.bulk.failures.push(message(&e)),
                    })
                    .ok();
                }
                _ => {
                    for item in list {
                        let current = this.update(cx, |_, cx| store(cx).read(cx).scope()).unwrap_or_default();
                        if current != scope {
                            this.update(cx, |l, _| l.bulk.failures.push("Stopped because the account or server changed.".into())).ok();
                            break;
                        }
                        let (app, key) = (app.clone(), item.rating_key().to_string());
                        let result = bg(async move { patina_core::downloads::enqueue(&app, &key).await }).await;
                        this.update(cx, |l, cx| {
                            match result {
                                Ok(_) => l.bulk.done += 1,
                                Err(e) => l.bulk.failures.push(format!("{}: {}", item.title(), message(&e))),
                            }
                            cx.notify();
                        })
                        .ok();
                    }
                }
            }
            this.update(cx, |l, cx| {
                l.bulk.busy = false;
                if l.bulk.failures.is_empty() {
                    store::toast(cx, format!("{} titles updated", l.bulk.done));
                    l.selection.clear();
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn add_to_playlist(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.selection.clone();
        if items.is_empty() {
            return;
        }
        let first = format::playlist_kind(&items[0]);
        if items.iter().any(|m| format::playlist_kind(m) != first) {
            store::error(cx, "Select only video or only music for a playlist.");
            return;
        }
        crate::dialogs::playlist::open(items, window, cx);
    }
}

/// Keep the previous data when a reload fails, so the page shows it with the error.
fn keep_stale<T>(old: Load<T>, result: anyhow::Result<T>) -> Load<T> {
    match (old, result) {
        (_, Ok(v)) => Load::Ready(v),
        (Load::Ready(v), Err(_)) => Load::Ready(v),
        (_, Err(e)) => Load::Failed(message(&e)),
    }
}

/// A chip-style toggle button (Unwatched, boolean filters).
fn chip(id: impl Into<ElementId>, label: impl Into<SharedString>, on: bool) -> Button {
    let b = Button::new(id).small().rounded_full().label(label);
    if on { b.primary() } else { b.outline() }
}

fn divider(cx: &App) -> Div {
    div().w_px().h_5().mx_1().bg(cx.theme().border)
}

/// A wrapping grid of cards (collections, folder items).
fn poster_grid(id: &str, items: Vec<Metadata>, cx: &App) -> Div {
    let w = store(cx).read(cx).poster_width();
    h_flex().flex_wrap().items_start().gap_5().children(items.into_iter().enumerate().map(|(i, item)| {
        Card::new(ElementId::Name(format!("{id}:{}:{i}", item.rating_key()).into()), item).width(w)
    }))
}

impl Library {
    fn head(&self, title: String, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let scan = store(cx).read(cx).activities.iter().find(|a| a.section_id.as_deref() == Some(self.id.as_str())).cloned();
        let owned = store(cx).read(cx).owned();
        let id = self.id.clone();
        let tabs = [Tab_::Browse, Tab_::Recommended, Tab_::Collections, Tab_::Folders];
        let selected = tabs.iter().position(|t| *t == self.tab).unwrap_or(0);
        h_flex()
            .mb_6()
            .items_end()
            .justify_between()
            .gap_6()
            .child(
                h_flex()
                    .min_w_0()
                    .items_center()
                    .gap_3p5()
                    .child(div().text_3xl().font_semibold().truncate().child(title))
                    .when_some(scan, |d, scan| {
                        d.child(
                            h_flex()
                                .gap_1p5()
                                .px_2()
                                .py_0p5()
                                .rounded(px(6.))
                                .border_1()
                                .border_color(theme.border)
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(Spinner::new().xsmall())
                                .child(format!("Scanning… {}%", scan.progress)),
                        )
                    }),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        TabBar::new("library-tabs")
                            .segmented()
                            .small()
                            .selected_index(selected)
                            .on_click(cx.listener(move |this, ix: &usize, _, cx| this.set_tab(tabs[*ix], cx)))
                            .child(Tab::new().label("Library"))
                            .child(Tab::new().label("Recommended"))
                            .child(Tab::new().label("Collections"))
                            .child(Tab::new().label("Folders")),
                    )
                    .when(owned, |d| {
                        d.child(Button::new("scan").outline().small().icon(IconName::RefreshCw).tooltip("Scan library files").on_click(move |_, _, cx| {
                            let (app, id) = (core(cx), id.clone());
                            store::act(cx, async move { Ok::<_, anyhow::Error>(app.server()?.refresh_section(&id, false, None).await?) }, Some("Scanning library…".into()), false);
                        }))
                    }),
            ).into_any_element()
    }

    fn toolbar(&self, kind: &str, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let types = types_of(kind);
        let watchable = kind != "artist" && kind != "photo";
        let booleans: Vec<FilterView> = self.filters.iter().filter(|f| f.kind == "boolean" && f.filter != "unwatched").cloned().collect();
        let has_values = self.filters.iter().any(|f| f.kind != "boolean");
        let noun = types.iter().find(|t| Some(t.0) == self.media_type).map(|t| t.1.to_lowercase().trim_end_matches('s').to_string()).unwrap_or_else(|| "item".into());
        let sorts: Vec<(String, String)> = if self.sorts.is_empty() {
            [("titleSort", "Title"), ("addedAt", "Date added"), ("originallyAvailableAt", "Release date"), ("rating", "Rating")].iter().map(|(k, t)| (k.to_string(), t.to_string())).collect()
        } else {
            self.sorts.iter().filter_map(|d| Some((d.key.clone()?, d.title.clone().unwrap_or_default()))).collect()
        };
        let sort_label = sorts.iter().find(|(k, _)| *k == self.sort).map(|(_, t)| t.clone()).unwrap_or_else(|| "Title".into());
        let me = cx.entity().downgrade();
        let saved = saved_views(&store(cx).read(cx).settings.ui.saved_views, &self.scope, &self.id);
        let name_empty = self.view_name.read(cx).value().trim().is_empty();

        let mut first = h_flex().flex_wrap().items_center().gap_2();
        if types.len() > 1 {
            let selected = types.iter().position(|t| Some(t.0) == self.media_type).unwrap_or(0);
            first = first
                .child(
                    TabBar::new("types")
                        .segmented()
                        .small()
                        .selected_index(selected)
                        .on_click(cx.listener(move |this, ix: &usize, _, cx| this.set_type(types[*ix].0, cx)))
                        .children(types.iter().map(|(_, label)| Tab::new().label(*label))),
                )
                .child(divider(cx));
        }
        first = first
            .child(
                Button::new("sort")
                    .outline()
                    .small()
                    .min_w(px(144.))
                    .label(sort_label)
                    .icon(Icon::new(IconName::ChevronDown).size_3())
                    .dropdown_menu(move |mut menu, _, _| {
                        for (key, title) in sorts.clone() {
                            let me = me.clone();
                            menu = menu.item(PopupMenuItem::new(title).on_click(move |_, _, cx| {
                                let key = key.clone();
                                me.update(cx, |l, cx| {
                                    l.sort = key;
                                    l.query_changed(cx);
                                })
                                .ok();
                            }));
                        }
                        menu
                    }),
            )
            .child(
                Button::new("direction")
                    .outline()
                    .small()
                    .icon(if self.desc { IconName::ArrowDownWideNarrow } else { IconName::ArrowUpNarrowWide })
                    .tooltip(if self.desc { "Descending" } else { "Ascending" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.desc = !this.desc;
                        this.query_changed(cx);
                    })),
            );
        if has_values {
            let picker = self.picker.clone();
            first = first.child(
                Popover::new("filter-picker")
                    .anchor(Anchor::TopLeft)
                    .open(picker.is_some())
                    .on_open_change(cx.listener(|this, open: &bool, window, cx| {
                        if *open {
                            this.open_picker(window, cx);
                        } else {
                            this.picker = None;
                            cx.notify();
                        }
                    }))
                    .trigger(Button::new("filter").outline().small().icon(IconName::ListFilter).label("Filter"))
                    .content(move |_, _, _| match &picker {
                        Some(p) => p.clone().into_any_element(),
                        None => div().into_any_element(),
                    }),
            );
        }
        if watchable || !booleans.is_empty() {
            first = first.child(divider(cx));
        }
        if watchable {
            first = first.child(chip("unwatched", "Unwatched", self.unwatched).on_click(cx.listener(|this, _, _, cx| {
                this.unwatched = !this.unwatched;
                this.query_changed(cx);
            })));
        }
        for (i, f) in booleans.into_iter().enumerate() {
            let on = self.toggles.contains(&f.filter);
            first = first.child(chip(("bool", i), f.title.clone(), on).on_click(cx.listener(move |this, _, _, cx| {
                if let Some(at) = this.toggles.iter().position(|t| *t == f.filter) {
                    this.toggles.remove(at);
                } else {
                    this.toggles.push(f.filter.clone());
                }
                this.query_changed(cx);
            })));
        }
        let selecting = self.selecting;
        first = first
            .child(
                Button::new("select")
                    .outline()
                    .small()
                    .label("Select titles")
                    .when(selecting, |b| b.text_color(store(cx).read(cx).tint()).border_color(store(cx).read(cx).tint().opacity(0.5)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.selecting = !this.selecting;
                        if !this.selecting {
                            this.selection.clear();
                        }
                        cx.notify();
                    })),
            )
            .child(div().flex_1())
            .when_some(self.pages.total, |d, total| d.child(div().text_sm().text_color(theme.muted_foreground).child(format::plural(total as i64, &noun))));

        let picked = (!self.picked.is_empty()).then(|| {
            h_flex()
                .flex_wrap()
                .items_center()
                .gap_2()
                .children(self.picked.iter().enumerate().map(|(i, p)| {
                    Button::new(("picked", i))
                        .secondary()
                        .small()
                        .rounded_full()
                        .tooltip("Remove filter")
                        .child(h_flex().gap_1().child(div().text_color(theme.muted_foreground).child(format!("{}:", p.title))).child(p.label.clone()).child(Icon::new(IconName::X).size_3().text_color(theme.muted_foreground)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if i < this.picked.len() {
                                this.picked.remove(i);
                            }
                            this.query_changed(cx);
                        }))
                }))
                .child(Button::new("clear-picked").ghost().small().label("Clear").on_click(cx.listener(|this, _, _, cx| {
                    this.picked.clear();
                    this.query_changed(cx);
                })))
        });

        let views = h_flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(div().w(px(208.)).child(Input::new(&self.view_name).small()))
            .child(Button::new("save-view").outline().small().label("Save view").disabled(name_empty).on_click(cx.listener(|this, _, window, cx| this.save_view(window, cx))))
            .children(saved.into_iter().enumerate().map(|(i, view)| {
                let (v, id, name) = (view.clone(), view.id.clone(), view.name.clone());
                h_flex()
                    .child(Button::new(("view", i)).secondary().small().label(view.name.clone()).on_click(cx.listener(move |this, _, _, cx| this.use_view(&v, cx))))
                    .child(Button::new(("view-x", i)).secondary().small().icon(IconName::X).tooltip(format!("Remove saved view {name}")).on_click(move |_, _, cx| Self::remove_view(id.clone(), cx)))
            }));
        let _ = window;
        v_flex().flex_none().mb_6().gap_3().pb_4().border_b_1().border_color(theme.border).child(first).children(picked).child(views).into_any_element()
    }

    fn bulk_bar(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let (busy, n) = (self.bulk.busy, self.selection.len());
        let off = busy || n == 0;
        v_flex()
            .flex_none()
            .child(
                h_flex()
                    .mb_4()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .rounded(px(8.))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.colors.group_box)
                    .px_3()
                    .py_2()
                    .when(busy, |d| d.child(Spinner::new().small()))
                    .child(div().mr_2().text_sm().font_medium().child(if busy { format!("{n} selected · {} complete", self.bulk.done) } else { format!("{n} selected") }))
                    .child(Button::new("bulk-queue").outline().small().label("Add to queue").disabled(off).on_click(cx.listener(|this, _, _, cx| this.run_bulk("queue", cx))))
                    .child(Button::new("bulk-download").outline().small().label("Download").disabled(off).on_click(cx.listener(|this, _, _, cx| this.run_bulk("download", cx))))
                    .child(Button::new("bulk-watched").outline().small().label("Mark watched").disabled(off).on_click(cx.listener(|this, _, _, cx| this.run_bulk("watched", cx))))
                    .child(Button::new("bulk-playlist").outline().small().label("Add to playlist").disabled(off).on_click(cx.listener(|this, _, window, cx| this.add_to_playlist(window, cx))))
                    .child(div().flex_1())
                    .child(Button::new("bulk-clear").ghost().small().label("Clear selection").disabled(off).on_click(cx.listener(|this, _, _, cx| {
                        this.selection.clear();
                        cx.notify();
                    }))),
            )
            .when(!self.bulk.failures.is_empty(), |d| {
                d.child(gpui_kit::component::alert::Alert::error("bulk-failures", format!("{} completed. {}", self.bulk.done, self.bulk.failures.join("\n"))).mb_4())
            }).into_any_element()
    }

    fn grid(&self, cx: &mut Context<Self>) -> AnyElement {
        let total = self.pages.total.unwrap_or(0);
        let shape = self.shape().unwrap_or(Shape::Poster);
        let poster = store(cx).read(cx).poster_width();
        let width = self.width.get();
        let (columns, row_h) = grid_metrics(width.max(1.), poster, shape);
        let rows = if width > 0. { total.div_ceil(columns) } else { 0 };
        let me = cx.entity();
        let measured = self.width.clone();
        let view = cx.entity_id();
        let list = uniform_list("library-grid", rows, move |range, _, cx| {
            me.update(cx, |l, cx| {
                let (start, end) = (range.start * columns, (range.end * columns).min(total));
                let need = l.pages.ensure(start, end);
                l.fetch(need, cx);
                let selecting = l.selecting;
                range
                    .map(|r| {
                        h_flex()
                            .w_full()
                            .h(px(row_h))
                            .gap(px(GAP))
                            .items_start()
                            .children((0..columns).map(|c| {
                                let index = r * columns + c;
                                let cell = div().flex_1().min_w_0().h_full();
                                if index >= total {
                                    return cell.into_any_element();
                                }
                                match l.pages.at(index).cloned() {
                                    Some(item) => {
                                        let selected = l.selection.iter().any(|m| m.rating_key() == item.rating_key());
                                        let me = cx.entity().downgrade();
                                        let toggle = item.clone();
                                        let mut card = Card::new(ElementId::Name(format!("grid:{index}:{}", item.rating_key()).into()), item).selecting(selecting, selected, move |_, cx| {
                                            me.update(cx, |l, cx| l.toggle_item(&toggle, cx)).ok();
                                        });
                                        if let Some(s) = l.shape() {
                                            card = card.shape(s);
                                        }
                                        cell.child(card).into_any_element()
                                    }
                                    None => cell.child(Skeleton::new().w_full().h(px(row_h - 64.)).rounded(px(10.))).into_any_element(),
                                }
                            }))
                            .into_any_element()
                    })
                    .collect::<Vec<_>>()
            })
        })
        .track_scroll(&self.scroll)
        .size_full();
        div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                canvas(
                    move |bounds, _, cx| {
                        let w: f32 = bounds.size.width.into();
                        if (measured.get() - w).abs() > 0.5 {
                            measured.set(w);
                            cx.notify(view);
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(list)
            .into_any_element()
    }

    fn rail(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let starts = letter_starts(&self.chars);
        let columns = grid_metrics(self.width.get().max(1.), store(cx).read(cx).poster_width(), self.shape().unwrap_or(Shape::Poster)).0;
        v_flex().flex_none().ml_3p5().children(LETTERS.chars().map(|l| {
            let at = starts.iter().find(|(s, _)| s.as_str() == l.to_string()).map(|(_, i)| *i);
            div()
                .id(SharedString::from(format!("letter-{l}")))
                .h(px(17.))
                .w(px(18.))
                .text_center()
                .text_size(px(10.5))
                .line_height(px(17.))
                .font_family("Menlo")
                .text_color(theme.muted_foreground)
                .when(at.is_none(), |d| d.opacity(0.3))
                .when_some(at, |d, at| {
                    d.cursor_pointer().hover(|s| s.text_color(theme.foreground)).on_click(cx.listener(move |this, _, _, cx| {
                        this.scroll.scroll_to_item(at / columns.max(1), ScrollStrategy::Top);
                        cx.notify();
                    }))
                })
                .child(l.to_string())
        })).into_any_element()
    }

    fn browse(&mut self, kind: &str, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let mut top = v_flex().flex_none().child(self.toolbar(kind, window, cx));
        if self.selecting {
            top = top.child(self.bulk_bar(cx));
        }
        if let Some(e) = self.pages.error.clone() {
            let stale = self.pages.total.unwrap_or(0) > 0;
            top = top.child(status_error(e, stale, cx).child(Button::new("retry").outline().small().mt_2().label("Retry").on_click(cx.listener(|this, _, _, cx| {
                let again = this.pages.refresh();
                this.fetch(again, cx);
                cx.notify();
            }))));
        }
        let body: AnyElement = match self.pages.total {
            Some(0) if !self.pages.loading() => widgets::empty("No matches", Some("Try a different filter.".into()), cx).into_any_element(),
            None if self.pages.error.is_none() => widgets::loading().into_any_element(),
            None => div().into_any_element(),
            Some(_) => {
                let rail = if self.rail_on() && !self.chars.is_empty() { Some(self.rail(cx)) } else { None };
                h_flex().flex_1().min_h_0().items_start().child(self.grid(cx)).children(rail).into_any_element()
            }
        };
        v_flex().size_full().px(GUTTER).child(top).child(div().flex_1().min_h_0().flex().child(body))
    }

    fn folders_view(&self, title: String, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let mut nav = h_flex().ml(px(-10.)).mb_5().flex_wrap().items_center().gap_1().child(
            Button::new("trail-root").ghost().small().label(title).text_color(if self.trail.is_empty() { theme.foreground } else { theme.muted_foreground }).on_click(cx.listener(|this, _, _, cx| {
                this.trail.clear();
                this.folders = Load::Loading;
                this.load_folders(cx);
                cx.notify();
            })),
        );
        let last = self.trail.len().saturating_sub(1);
        for (i, (_, step)) in self.trail.iter().enumerate() {
            nav = nav.child(Icon::new(IconName::ChevronRight).size_3().text_color(theme.muted_foreground)).child(
                Button::new(("trail", i)).ghost().small().label(step.clone()).text_color(if i == last { theme.foreground } else { theme.muted_foreground }).on_click(cx.listener(move |this, _, _, cx| {
                    this.trail.truncate(i + 1);
                    this.folders = Load::Loading;
                    this.load_folders(cx);
                    cx.notify();
                })),
            );
        }
        let body = match &self.folders {
            Load::Loading => widgets::loading().into_any_element(),
            Load::Failed(e) => widgets::failed(e.clone(), cx).into_any_element(),
            Load::Ready(f) if f.directories.is_empty() && f.items.is_empty() => widgets::empty("Empty folder", None, cx).into_any_element(),
            Load::Ready(f) => v_flex()
                .when(!f.directories.is_empty(), |d| {
                    d.child(widgets::rows(cx).mb_8().children(f.directories.iter().enumerate().map(|(i, dir)| {
                        let step = (dir.key.clone(), dir.title.clone().unwrap_or_else(|| "Folder".into()));
                        widgets::list_row(i == 0, cx)
                            .id(("folder", i))
                            .cursor_pointer()
                            .hover(|s| s.bg(theme.muted.opacity(0.5)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.trail.push(step.clone());
                                this.folders = Load::Loading;
                                this.load_folders(cx);
                                cx.notify();
                            }))
                            .child(Icon::new(IconName::Folder).size_4().text_color(theme.muted_foreground))
                            .child(div().flex_1().text_sm().truncate().child(dir.title.clone().unwrap_or_default()))
                            .child(Icon::new(IconName::ChevronRight).size_3p5().text_color(theme.muted_foreground))
                    })))
                })
                .when(!f.items.is_empty(), |d| d.child(poster_grid("folder", f.items.clone(), cx)))
                .into_any_element(),
        };
        v_flex().child(nav).child(body).into_any_element()
    }
}

/// An error above content: "Showing saved results" when older data stays on screen.
fn status_error(error: String, stale: bool, cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .mb_4()
        .p_4()
        .rounded(px(8.))
        .border_1()
        .border_color(if stale { theme.border } else { theme.danger.opacity(0.5) })
        .when(stale, |d| d.child(div().text_sm().font_medium().child("Showing saved results")))
        .child(div().text_sm().text_color(if stale { theme.muted_foreground } else { theme.danger }).child(error))
}

impl Render for Library {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(section) = self.section(cx).cloned() else {
            return widgets::page("library").child(widgets::empty("Library not found", None, cx)).into_any_element();
        };
        let title = section.title.clone().unwrap_or_default();
        let kind = section.kind.clone().unwrap_or_default();
        if self.tab == Tab_::Browse {
            let head = self.head(title, cx);
            let browse = self.browse(&kind, window, cx);
            return v_flex().size_full().child(div().flex_none().px(GUTTER).pt(px(28.)).child(head)).child(div().flex_1().min_h_0().child(browse)).into_any_element();
        }
        let head = self.head(title.clone(), cx);
        let body: AnyElement = match self.tab {
            Tab_::Recommended => match &self.hubs {
                Load::Loading => widgets::loading().into_any_element(),
                Load::Failed(e) => widgets::failed(e.clone(), cx).into_any_element(),
                Load::Ready(hubs) if !hubs.iter().any(|h| !h.metadata.is_empty()) => widgets::empty("No recommendations yet", None, cx).into_any_element(),
                Load::Ready(hubs) => v_flex()
                    .children(hubs.iter().filter(|h| !h.metadata.is_empty()).enumerate().map(|(i, h)| {
                        HubRow::new(ElementId::Name(format!("{}:{i}", h.hub_identifier.as_deref().unwrap_or("")).into()), h.clone())
                    }))
                    .into_any_element(),
            },
            Tab_::Collections => match &self.collections {
                Load::Loading => widgets::loading().into_any_element(),
                Load::Failed(e) => widgets::failed(e.clone(), cx).into_any_element(),
                Load::Ready(list) if list.is_empty() => widgets::empty("No collections", Some("Collections you create in Plex show up here.".into()), cx).into_any_element(),
                Load::Ready(list) => poster_grid("collection", list.clone(), cx).into_any_element(),
            },
            _ => self.folders_view(title, cx),
        };
        widgets::page("library").child(head).child(body).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    fn dir(key: &str, title: &str, size: i64) -> Directory {
        Directory { key: Some(key.into()), title: Some(title.into()), size: Some(size), ..Default::default() }
    }

    #[test]
    fn descending_sort_uses_the_server_key() {
        let sorts = vec![Directory { key: Some("titleSort".into()), desc_key: Some("titleSort:desc".into()), ..Default::default() }, dir("addedAt", "Date added", 0)];
        assert_eq!(sort_value("titleSort", false, &sorts), "titleSort");
        assert_eq!(sort_value("titleSort", true, &sorts), "titleSort:desc");
        assert_eq!(sort_value("year", true, &sorts), "year:desc");
    }

    #[test]
    fn letters_start_where_the_previous_ones_end() {
        let chars = vec![dir("#", "#", 3), dir("A", "A", 10), dir("C", "c", 2)];
        assert_eq!(letter_starts(&chars), vec![("#".into(), 0), ("A".into(), 3), ("C".into(), 13)]);
    }

    #[test]
    fn grid_fits_columns_to_the_width() {
        let (cols, h) = grid_metrics(900., 160., Shape::Poster);
        assert_eq!(cols, 5);
        assert!((h - ((900. - 80.) / 5. * 1.5 + 64.)).abs() < 0.01);
        assert_eq!(grid_metrics(100., 160., Shape::Poster).0, 1);
        assert_eq!(grid_metrics(900., 160., Shape::Landscape).0, 3);
    }

    #[test]
    fn saved_views_round_trip_and_filter_by_library() {
        let raw: serde_json::Value = serde_json::from_str(
            r#"{"id": "view-1", "name": "Unwatched drama", "scope": "alice:home", "library": "1",
                "mediaType": 0, "sort": "titleSort", "desc": false, "unwatched": true,
                "toggles": [], "picked": [{"filter": "genre", "title": "Genre", "value": "10", "label": "Drama"}]}"#,
        )
        .unwrap();
        let other = serde_json::json!({ "id": "v2", "name": "x", "scope": "alice:home", "library": "2" });
        let views = saved_views(&[raw.clone(), other, serde_json::json!("junk")], "alice:home", "1");
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].picked[0].label, "Drama");
        assert_eq!(serde_json::to_value(&views[0]).unwrap(), raw);
    }
}
