//! A Plex hub (Continue Watching, Recently Added, ...) rendered as a row.

use crate::prelude::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme as _, Icon, h_flex};
use patina_plex::Hub;

use super::{CardRow, rows, section_head};
use crate::actions;
use crate::format::{self, Shape};
use crate::nav::Route;

#[derive(IntoElement)]
pub struct HubRow {
    id: ElementId,
    hub: Hub,
    compact: bool,
}

impl HubRow {
    pub fn new(id: impl Into<ElementId>, hub: Hub) -> Self {
        Self { id: id.into(), hub, compact: false }
    }

    /// Continue Watching as a list of rows instead of cards.
    pub fn compact(mut self, on: bool) -> Self {
        self.compact = on;
        self
    }
}

/// Continue Watching, On Deck and similar in-progress hubs.
pub fn is_continue(hub: &Hub) -> bool {
    let ident = hub.hub_identifier.as_deref().unwrap_or("").to_lowercase();
    ["continue", "ondeck", "inprogress"].iter().any(|s| ident.contains(s))
}

impl RenderOnce for HubRow {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let hub = self.hub;
        let continue_style = is_continue(&hub);
        let landscape = continue_style || matches!(hub.kind.as_deref(), Some("clip" | "episode"));
        let title = hub.title.clone().unwrap_or_default();
        if self.compact && continue_style {
            let theme = cx.theme().clone();
            return div()
                .id(self.id)
                .child(section_head(title, None))
                .child(rows(cx).children(hub.metadata.into_iter().enumerate().map(|(i, item)| {
                    let left = format::duration((item.duration.unwrap_or(0) - item.view_offset.unwrap_or(0)).max(0));
                    let name = match &item.grandparent_title {
                        Some(show) => format!("{show} · {}", item.title()),
                        None => item.title().to_string(),
                    };
                    h_flex()
                        .id(i)
                        .w_full()
                        .gap_5()
                        .px_4()
                        .py_2p5()
                        .text_sm()
                        .cursor_pointer()
                        .when(i > 0, |d| d.border_t_1().border_color(theme.border))
                        .hover(|s| s.bg(theme.muted.opacity(0.5)))
                        .on_click(move |_, _, cx| actions::play(&item, cx))
                        .child(div().flex_1().min_w_0().truncate().font_medium().child(name))
                        .child(div().text_xs().text_color(theme.muted_foreground).child(format!("{left} left")))
                        .child(
                            h_flex()
                                .gap_1()
                                .px_2()
                                .py_0p5()
                                .rounded(px(6.))
                                .bg(theme.secondary)
                                .text_xs()
                                .font_medium()
                                .child(Icon::new(IconName::Play).size_3())
                                .child("Resume"),
                        )
                })))
                .into_any_element();
        }
        let more = (hub.more && hub.key.is_some()).then(|| Route::browse(hub.key.clone().unwrap_or_default(), title.clone()));
        let mut row = CardRow::new(self.id, title, hub.metadata).use_art(continue_style).more(more);
        if landscape {
            row = row.shape(Shape::Landscape);
        }
        row.into_any_element()
    }
}
