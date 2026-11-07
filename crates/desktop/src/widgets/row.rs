//! A titled, horizontally scrolling row of cards.

use crate::prelude::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::h_flex;
use patina_plex::Metadata;

use super::{Card, section_head};
use crate::format::{self, Shape};
use crate::nav::{self, Route};
use crate::store::store;
use crate::theme::GUTTER;

#[derive(IntoElement)]
pub struct CardRow {
    id: ElementId,
    title: SharedString,
    items: Vec<Metadata>,
    shape: Option<Shape>,
    use_art: bool,
    more: Option<Route>,
}

impl CardRow {
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>, items: Vec<Metadata>) -> Self {
        Self { id: id.into(), title: title.into(), items, shape: None, use_art: false, more: None }
    }

    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = Some(shape);
        self
    }

    /// Continue-watching style cards.
    pub fn use_art(mut self, on: bool) -> Self {
        self.use_art = on;
        self
    }

    /// A "See all" link.
    pub fn more(mut self, route: Option<Route>) -> Self {
        self.more = route;
        self
    }
}

impl RenderOnce for CardRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.items.is_empty() {
            return div().into_any_element();
        }
        let shape = self.shape.unwrap_or_else(|| format::common_shape(&self.items));
        let poster = store(cx).read(cx).poster_width();
        let width = if shape == Shape::Landscape { poster * 1.75 } else { poster };
        let scroll = window.use_keyed_state(self.id.clone(), cx, |_, _| ScrollHandle::new()).read(cx).clone();
        let offset = -scroll.offset().x;
        let max = scroll.max_offset().x;
        let (at_start, at_end) = (offset <= px(2.), offset >= max - px(2.));
        let page = |dir: f32| {
            let scroll = scroll.clone();
            move |_: &ClickEvent, window: &mut Window, _: &mut App| {
                let step = scroll.bounds().size.width * 0.8;
                let next = (-scroll.offset().x + step * dir).clamp(px(0.), scroll.max_offset().x);
                scroll.set_offset(point(-next, scroll.offset().y));
                window.refresh();
            }
        };
        let controls = h_flex()
            .gap_1p5()
            .items_center()
            .when_some(self.more, |d, route| {
                d.child(Button::new("more").ghost().small().label("See all").text_color(cx.theme().muted_foreground).on_click(move |_, _, cx| nav::push(route.clone(), cx)))
            })
            .when(!(at_start && at_end) && max > px(0.), |d| {
                d.child(Button::new("left").outline().small().icon(IconName::ChevronLeft).tooltip("Scroll left").disabled(at_start).on_click(page(-1.)))
                    .child(Button::new("right").outline().small().icon(IconName::ChevronRight).tooltip("Scroll right").disabled(at_end).on_click(page(1.)))
            });
        let use_art = self.use_art;
        div()
            .id(self.id)
            .child(section_head(self.title, Some(controls.into_any_element())))
            .child(
                super::h_scroll("cards")
                    .mx(-GUTTER)
                    .px(GUTTER)
                    .pb_1()
                    .gap_5()
                    .items_start()
                    .track_scroll(&scroll)
                    .children(self.items.into_iter().enumerate().map(|(i, item)| {
                        let key = format!("{}:{}:{i}", item.rating_key(), item.key.as_deref().unwrap_or(""));
                        Card::new(ElementId::Name(key.into()), item).shape(shape).use_art(use_art).width(width)
                    })),
            )
            .into_any_element()
    }
}
