//! Poster / thumbnail card for any metadata item.

use crate::prelude::*;
use std::rc::Rc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use patina_plex::Metadata;

use crate::actions;
use crate::art::art_of;
use crate::format::{self, Shape};
use crate::menu::item_menu;
use crate::nav::{self, Route};
use crate::store::store;

type Toggle = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Card {
    id: ElementId,
    item: Metadata,
    shape: Option<Shape>,
    use_art: bool,
    width: Option<f32>,
    selecting: bool,
    selected: bool,
    on_toggle: Option<Toggle>,
}

impl Card {
    pub fn new(id: impl Into<ElementId>, item: Metadata) -> Self {
        Self { id: id.into(), item, shape: None, use_art: false, width: None, selecting: false, selected: false, on_toggle: None }
    }

    /// Force a shape, so a row of mixed items lines up.
    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = Some(shape);
        self
    }

    /// Continue-watching style: backdrop art, show name as the title.
    pub fn use_art(mut self, on: bool) -> Self {
        self.use_art = on;
        self
    }

    /// A fixed width (inside horizontal rows) rather than filling a grid cell.
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }

    /// Selection mode: clicking toggles instead of opening.
    pub fn selecting(mut self, selecting: bool, selected: bool, on_toggle: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.selecting = selecting;
        self.selected = selected;
        self.on_toggle = Some(Rc::new(on_toggle));
        self
    }
}

impl RenderOnce for Card {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let item = self.item;
        let shape = self.shape.unwrap_or_else(|| format::shape_for(&item));
        let kind = item.kind().to_string();
        let (iw, ih) = match shape {
            Shape::Poster => (150., 225.),
            Shape::Landscape => (280., 158.),
            Shape::Square => (180., 180.),
        };
        let source = match shape {
            Shape::Landscape if self.use_art && kind != "episode" => item.backdrop().or(item.thumb.as_deref()),
            Shape::Landscape => item.thumb.as_deref().or(item.backdrop()),
            _ => item.poster(),
        };
        let image = art_of(source, iw, ih, item.server_id());
        let (line1, line2) = if kind == "episode" && self.use_art {
            (item.grandparent_title.clone().unwrap_or_default(), format!("{} · {}", item.episode_code(), item.title()))
        } else {
            (item.title().to_string(), format::subtitle(&item))
        };
        let unwatched = if kind == "show" || kind == "season" { item.unwatched_leaves().unwrap_or(0) } else { 0 };
        let watched = format::has_watch_state(&item) && unwatched == 0 && item.is_watched();
        let progress = item.progress();
        let spoiler = store(cx).read(cx).spoilers() && kind == "episode" && !item.is_watched();
        let tint = store(cx).read(cx).tint();
        let theme = cx.theme().clone();
        let ratio = match shape {
            Shape::Poster => 2. / 3.,
            Shape::Landscape => 16. / 9.,
            Shape::Square => 1.,
        };
        let group: SharedString = format!("card-{:?}", self.id).into();
        let selecting = self.selecting;
        let on_toggle = self.on_toggle.clone();
        let open_item = item.clone();
        let menu_item = item.clone();
        let play_item = item.clone();
        let more_item = item.clone();

        let art = div()
            .relative()
            .w_full()
            .aspect_ratio(ratio)
            .overflow_hidden()
            .rounded(px(10.))
            .bg(theme.colors.group_box)
            .border_1()
            .border_color(if selecting && self.selected { tint } else { theme.foreground.opacity(0.1) })
            .group_hover(group.clone(), |s| s.border_color(theme.foreground.opacity(0.25)))
            .child(match image {
                Some(img) => img
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .when(spoiler, |i| i.opacity(0.15))
                    .into_any_element(),
                None => div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_3()
                    .text_center()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(item.title().to_string())
                    .into_any_element(),
            })
            .when(unwatched > 0, |d| {
                d.child(
                    div()
                        .absolute()
                        .top_2()
                        .right_2()
                        .px_1p5()
                        .rounded(px(6.))
                        .bg(theme.background.opacity(0.8))
                        .text_xs()
                        .font_medium()
                        .child(unwatched.to_string()),
                )
            })
            .when(unwatched == 0 && watched, |d| {
                d.child(
                    div()
                        .absolute()
                        .top_2()
                        .right_2()
                        .size_5()
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(theme.background.opacity(0.8))
                        .text_color(tint)
                        .child(Icon::new(IconName::Check).size_3()),
                )
            })
            .when_some(progress, |d, p| {
                d.child(
                    div()
                        .absolute()
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .h_1()
                        .bg(hsla(0., 0., 0., 0.55))
                        .child(div().h_full().w(relative(p as f32)).bg(tint)),
                )
            })
            .when(selecting, |d| {
                let selected = self.selected;
                d.child(
                    div()
                        .absolute()
                        .top_2()
                        .left_2()
                        .size_5()
                        .rounded_full()
                        .border_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(selected, |d| d.border_color(tint).bg(tint).text_color(theme.background).child(Icon::new(IconName::Check).size_3()))
                        .when(!selected, |d| d.border_color(theme.foreground.opacity(0.4)).bg(theme.background.opacity(0.7))),
                )
            })
            .when(!selecting, |d| {
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .invisible()
                        .group_hover(group.clone(), |s| s.visible())
                        .bg(linear_gradient(0., linear_color_stop(hsla(0., 0., 0., 0.5), 0.), linear_color_stop(hsla(0., 0., 0., 0.), 0.6)))
                        .when(format::can_play(&item), |d| {
                            d.child(
                                div().absolute().bottom_3().left_2().child(
                                    Button::new("play")
                                        .primary()
                                        .icon(IconName::Play)
                                        .rounded_full()
                                        .tooltip("Play")
                                        .on_click(move |_, _, cx| {
                                            cx.stop_propagation();
                                            actions::play(&play_item, cx);
                                        }),
                                ),
                            )
                        })
                        .child(
                            div().absolute().top_2().left_2().child(
                                Button::new("more")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::Ellipsis)
                                    .bg(theme.background.opacity(0.7))
                                    .tooltip("More")
                                    .dropdown_menu(move |menu, window, cx| item_menu(menu, &more_item, window, cx)),
                            ),
                        ),
                )
            });

        div()
            .id(self.id)
            .group(group)
            .min_w_0()
            .cursor_pointer()
            .when_some(self.width, |d, w| d.w(px(w)).flex_none())
            .on_click(move |_, window, cx| {
                if selecting {
                    if let Some(t) = &on_toggle {
                        t(window, cx);
                    }
                } else {
                    nav::push(Route::for_item(&open_item), cx);
                }
            })
            .child(art)
            .child(div().mt_2().text_sm().font_medium().truncate().child(line1))
            .when(!line2.is_empty(), |d| d.child(div().text_xs().text_color(theme.muted_foreground).truncate().child(line2)))
            .context_menu(move |menu, window, cx| item_menu(menu, &menu_item, window, cx))
    }
}

/// Bits of metadata separated by dots, leaving out empty ones.
pub fn dots(bits: impl IntoIterator<Item = String>) -> String {
    bits.into_iter().filter(|b| !b.is_empty()).collect::<Vec<_>>().join(" · ")
}
