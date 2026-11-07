//! Home: a featured hero, then the server's hubs, which can be reordered and hidden.

use crate::prelude::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use patina_core::config::HomeLayout;
use patina_plex::{Hub, Metadata};

use crate::actions;
use crate::art::art_of;
use crate::core::{Load, bg, core};
use crate::format;
use crate::nav::{self, Route};
use crate::store::store;
use crate::theme::GUTTER;
use crate::widgets::{self, hub::HubRow};

/// Hubs hidden until turned on: Plex's legacy On Deck, which Continue Watching replaces.
const HIDDEN_BY_DEFAULT: &[&str] = &["home.ondeck"];

pub struct Home {
    hubs: Load<Vec<Hub>>,
    /// The last good list, shown while reloading or after a failed reload.
    stale: Option<Vec<Hub>>,
    revision: u64,
    customizing: bool,
    index: usize,
    _subs: Vec<Subscription>,
}

fn hub_key(h: &Hub) -> String {
    h.hub_identifier.clone().or_else(|| h.key.clone()).or_else(|| h.title.clone()).unwrap_or_default()
}

impl Home {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let store = store(cx);
        let subs = vec![cx.observe(&store, |this: &mut Self, store, cx| {
            if store.read(cx).revision != this.revision {
                this.load(cx);
            }
            cx.notify();
        })];
        let mut this = Self { hubs: Load::Loading, stale: None, revision: 0, customizing: false, index: 0, _subs: subs };
        this.load(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.revision = store(cx).read(cx).revision;
        let app = core(cx);
        cx.spawn(async move |this, cx| {
            let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.home_hubs(20).await?) }).await;
            this.update(cx, |h, cx| {
                h.hubs = Load::from_result(result);
                if let Some(list) = h.hubs.ready() {
                    h.stale = Some(list.clone());
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn layout(cx: &App) -> HomeLayout {
        let s = store(cx).read(cx);
        s.settings.ui.home_layouts.get(&s.scope()).cloned().unwrap_or_default()
    }

    fn save_layout(layout: HomeLayout, cx: &mut App) {
        let store = store(cx);
        let scope = store.read(cx).scope();
        store.update(cx, |s, cx| s.update_ui(cx, move |ui| {
            ui.home_layouts.insert(scope, layout);
        }));
    }

    fn is_hidden(layout: &HomeLayout, key: &str) -> bool {
        layout.hidden.iter().any(|k| k == key) || (HIDDEN_BY_DEFAULT.contains(&key) && !layout.shown.iter().any(|k| k == key))
    }

    fn set_shown(mut layout: HomeLayout, key: &str, on: bool, cx: &mut App) {
        layout.hidden.retain(|k| k != key);
        layout.shown.retain(|k| k != key);
        if HIDDEN_BY_DEFAULT.contains(&key) {
            if on {
                layout.shown.push(key.to_string());
            }
        } else if !on {
            layout.hidden.push(key.to_string());
        }
        Self::save_layout(layout, cx);
    }

    /// Featured items: recent or in-progress movies and shows with backdrop art.
    fn featured(rows: &[Hub]) -> Vec<(Metadata, &'static str)> {
        let mut out: Vec<(Metadata, &'static str)> = Vec::new();
        for hub in rows {
            let id = hub.hub_identifier.as_deref().unwrap_or("").to_lowercase();
            let label = if id.contains("continue") || id.contains("ondeck") {
                "Continue watching"
            } else if id.contains("recent") {
                "Recently added"
            } else {
                continue;
            };
            for m in &hub.metadata {
                if out.len() < 6 && m.art.is_some() && matches!(m.kind(), "movie" | "show") && !out.iter().any(|(x, _)| x.rating_key == m.rating_key) {
                    out.push((m.clone(), label));
                }
            }
        }
        out
    }

    fn hero(&self, item: Metadata, label: &'static str, count: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let meta: Vec<String> = [
            item.year.map(|y| y.to_string()),
            item.content_rating.clone(),
            if item.kind() == "show" { item.child_count.map(|n| format::plural(n, "season")) } else { item.duration.map(format::duration) },
            Some(item.genres.iter().take(2).filter_map(|g| g.tag.clone()).collect::<Vec<_>>().join(", ")),
        ]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect();
        let bg = theme.background;
        let play_item = item.clone();
        let open_item = item.clone();
        let resume = item.progress().is_some();
        div()
            .relative()
            .flex_none()
            .mx(GUTTER)
            .mt_2()
            .h(px(480.))
            .overflow_hidden()
            .rounded(px(12.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.colors.group_box)
            .children(art_of(item.art.as_deref(), 1280., 720., None).map(|i| i.absolute().inset_0().size_full().object_fit(ObjectFit::Cover)))
            .child(div().absolute().inset_0().bg(linear_gradient(90., linear_color_stop(bg.opacity(0.95), 0.), linear_color_stop(bg.opacity(0.), 0.75))))
            .child(div().absolute().inset_0().bg(linear_gradient(0., linear_color_stop(bg.opacity(0.7), 0.), linear_color_stop(bg.opacity(0.), 0.4))))
            .child(
                v_flex()
                    .absolute()
                    .bottom(px(36.))
                    .left(px(36.))
                    .max_w(px(576.))
                    .child(div().w_auto().flex().child(div().px_2().py_0p5().rounded(px(6.)).bg(theme.secondary).text_xs().font_medium().child(label)))
                    .child(div().mt_3().text_size(px(48.)).line_height(px(50.)).font_semibold().child(item.title().to_string()))
                    .child(div().mt_3().text_sm().text_color(theme.muted_foreground).child(meta.join("  ·  ")))
                    .when_some(item.summary.clone().filter(|s| !s.is_empty()), |d, s| {
                        d.child(div().mt_3p5().text_sm().line_clamp(3).text_color(theme.muted_foreground).child(s))
                    })
                    .child(
                        h_flex()
                            .mt_5()
                            .gap_2()
                            .child(Button::new("hero-play").primary().large().icon(IconName::Play).label(if resume { "Resume" } else { "Play" }).on_click(move |_, _, cx| actions::play(&play_item, cx)))
                            .child(Button::new("hero-details").large().label("Details").on_click(move |_, _, cx| nav::push(Route::for_item(&open_item), cx))),
                    ),
            )
            .when(count > 1, |d| {
                let i = self.index % count;
                d.child(
                    h_flex()
                        .absolute()
                        .right(px(24.))
                        .bottom(px(32.))
                        .gap_1p5()
                        .items_center()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(Button::new("hero-prev").outline().small().icon(IconName::ChevronLeft).tooltip("Previous").on_click(cx.listener(move |this, _, _, cx| {
                            this.index = (this.index + count - 1) % count;
                            cx.notify();
                        })))
                        .child(div().px_1().child(format!("{:02} / {:02}", i + 1, count)))
                        .child(Button::new("hero-next").outline().small().icon(IconName::ChevronRight).tooltip("Next").on_click(cx.listener(move |this, _, _, cx| {
                            this.index = (this.index + 1) % count;
                            cx.notify();
                        }))),
                )
            })
    }

    fn customize(&self, all: &[Hub], layout: &HomeLayout, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let n = all.len();
        let order: Vec<String> = all.iter().map(hub_key).collect();
        let compact = layout.compact_continue;
        let l = layout.clone();
        widgets::rows(cx)
            .mx(GUTTER)
            .mt_2()
            .mb_6()
            .child(h_flex().px_4().py_3().gap_3().child(Checkbox::new("home-compact").label("Compact Continue Watching").checked(compact).on_click(move |on, _, cx| {
                let mut l = l.clone();
                l.compact_continue = *on;
                Self::save_layout(l, cx);
            })))
            .children(all.iter().enumerate().map(|(i, hub)| {
                let key = hub_key(hub);
                let title = hub.title.clone().unwrap_or_default();
                let shown = !Self::is_hidden(layout, &key);
                let (l1, l2, l3) = (layout.clone(), layout.clone(), layout.clone());
                let (o1, o2) = (order.clone(), order.clone());
                let k = key.clone();
                let swap = |mut l: HomeLayout, mut order: Vec<String>, a: usize, b: usize, cx: &mut App| {
                    order.swap(a, b);
                    l.order = order;
                    Self::save_layout(l, cx);
                };
                h_flex()
                    .px_4()
                    .py_2()
                    .gap_3()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(div().flex_1().child(Checkbox::new(("home-hub", i)).label(title.clone()).checked(shown).on_click(move |on, _, cx| Self::set_shown(l1.clone(), &k, *on, cx))))
                    .child(Button::new(("up", i)).ghost().small().icon(IconName::ChevronUp).tooltip(format!("Move {title} up")).disabled(i == 0).on_click(move |_, _, cx| swap(l2.clone(), o1.clone(), i, i - 1, cx)))
                    .child(Button::new(("down", i)).ghost().small().icon(IconName::ChevronDown).tooltip(format!("Move {title} down")).disabled(i + 1 == n).on_click(move |_, _, cx| swap(l3.clone(), o2.clone(), i, i + 1, cx)))
            }))
            .child(
                h_flex()
                    .justify_end()
                    .border_t_1()
                    .border_color(theme.border)
                    .bg(theme.muted.opacity(0.3))
                    .px_4()
                    .py_2p5()
                    .child(Button::new("reset-layout").ghost().small().label("Reset layout").on_click(|_, _, cx| Self::save_layout(HomeLayout::default(), cx))),
            )
    }
}

impl Render for Home {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let layout = Self::layout(cx);
        let list = self.hubs.ready().cloned().or_else(|| self.stale.clone());
        let mut body = v_flex().id("home").size_full().overflow_y_scroll().pb(px(64.));
        body = body.child(
            h_flex()
                .justify_end()
                .gap_2()
                .px(GUTTER)
                .pt_4()
                .pb_1()
                .child(Button::new("pick").ghost().small().label("Pick something to watch").on_click(|_, _, cx| nav::push(Route::Pick, cx)))
                .child(Button::new("customize").ghost().small().label("Customize Home").selected(self.customizing).on_click(cx.listener(|this, _, _, cx| {
                    this.customizing = !this.customizing;
                    cx.notify();
                }))),
        );
        let Some(list) = list else {
            return body
                .child(match &self.hubs {
                    Load::Failed(e) => widgets::failed(e.clone(), cx).child(Button::new("retry").outline().small().mt_3().label("Try again").on_click(cx.listener(|this, _, _, cx| this.load(cx)))),
                    _ => widgets::loading(),
                })
                .into_any_element();
        };
        let mut all: Vec<Hub> = list.into_iter().filter(|h| !h.metadata.is_empty() || !h.directory.is_empty()).collect();
        all.sort_by_key(|h| layout.order.iter().position(|k| *k == hub_key(h)).unwrap_or(100_000));
        if self.customizing {
            body = body.child(self.customize(&all, &layout, cx));
        }
        if let Load::Failed(e) = &self.hubs {
            body = body.child(div().px(GUTTER).text_sm().text_color(theme.danger).child(format!("Could not refresh: {e}")));
        }
        let rows: Vec<Hub> = all.iter().filter(|h| !Self::is_hidden(&layout, &hub_key(h))).cloned().collect();
        if rows.is_empty() {
            let text = if all.is_empty() { "Your libraries are empty or still scanning." } else { "Your Home rows are hidden. Use Customize Home to show them." };
            return body.child(widgets::empty("Nothing here yet", Some(text.into()), cx)).into_any_element();
        }
        let picks = Self::featured(&rows);
        let count = picks.len();
        if let Some((item, label)) = picks.into_iter().nth(self.index % count.max(1)) {
            body = body.child(self.hero(item, label, count, cx));
        }
        body.child(
            v_flex().px(GUTTER).pt_1().children(rows.into_iter().enumerate().map(|(i, hub)| {
                let id = SharedString::from(format!("{}:{}:{i}", hub.hub_identifier.as_deref().unwrap_or(""), hub.key.as_deref().unwrap_or("")));
                HubRow::new(ElementId::Name(id), hub).compact(layout.compact_continue)
            })),
        )
        .into_any_element()
    }
}
