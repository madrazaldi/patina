//! Patina's look: shadcn's neutral dark theme (the same tokens as the old
//! webview's `app.css`). Colour comes from the artwork; the interface stays
//! neutral. One tint (the user's accent, muted) marks progress, watched state
//! and selection. The app is dark-only.

use crate::prelude::*;
use gpui_kit::component::{Theme, ThemeMode};

/// Space at the sides of page content.
pub const GUTTER: Pixels = px(32.);

/// `#rrggbb` (or `#rrggbbaa`) as a colour; black when it does not parse.
pub fn hex(s: &str) -> Hsla {
    let s = s.trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).unwrap_or(0);
    match s.len() {
        8 => rgba(v).into(),
        _ => rgb(v).into(),
    }
}

/// Neutral grey of the given oklch lightness, as the old theme specified them.
fn grey(l: f32) -> Hsla {
    let y = l * l * l;
    let srgb = if y <= 0.003_130_8 { 12.92 * y } else { 1.055 * y.powf(1. / 2.4) - 0.055 };
    hsla(0., 0., srgb.clamp(0., 1.), 1.)
}

fn white(alpha: f32) -> Hsla {
    hsla(0., 0., 1., alpha)
}

/// Faint text (`--text-3`).
pub fn faint() -> Hsla {
    grey(0.556)
}

/// Raised surfaces between the background and cards (`--bg-2`).
pub fn bg2() -> Hsla {
    grey(0.165)
}

pub fn init(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |t| {
        let background = grey(0.145);
        let foreground = grey(0.985);
        let card = grey(0.185);
        let popover = grey(0.205);
        let primary = grey(0.922);
        let primary_fg = grey(0.205);
        let secondary = grey(0.269);
        let muted_fg = grey(0.708);
        let border = white(0.10);
        let input = white(0.15);
        let destructive = hex("#ff6467");

        t.radius = px(8.);
        t.radius_lg = px(10.);
        t.shadow = true;

        let c = &mut t.colors;
        c.background = background;
        c.foreground = foreground;
        c.border = border;
        c.input = input;
        c.ring = grey(0.556);
        c.popover = popover;
        c.popover_foreground = foreground;
        c.primary = primary;
        c.primary_hover = primary.opacity(0.9);
        c.primary_active = primary.opacity(0.8);
        c.primary_foreground = primary_fg;
        c.secondary = secondary;
        c.secondary_hover = secondary.opacity(0.8);
        c.secondary_active = secondary.opacity(0.7);
        c.secondary_foreground = foreground;
        c.muted = secondary;
        c.muted_foreground = muted_fg;
        c.accent = secondary;
        c.accent_foreground = foreground;
        c.danger = destructive;
        c.danger_hover = destructive.opacity(0.9);
        c.danger_active = destructive.opacity(0.8);
        c.danger_foreground = foreground;
        c.sidebar = grey(0.175);
        c.sidebar_foreground = foreground;
        c.sidebar_accent = secondary;
        c.sidebar_accent_foreground = foreground;
        c.sidebar_border = border;
        c.sidebar_primary = primary;
        c.sidebar_primary_foreground = primary_fg;
        c.list = background;
        c.list_hover = secondary.opacity(0.5);
        c.list_active = secondary;
        c.list_even = background;
        c.list_head = background;
        c.table = background;
        c.table_head = background;
        c.table_hover = secondary.opacity(0.5);
        c.table_even = background;
        c.table_row_border = border;
        c.title_bar = background;
        c.title_bar_border = border;
        c.tab_bar = background;
        c.tab = background;
        c.tab_active = background;
        c.tab_foreground = muted_fg;
        c.tab_active_foreground = foreground;
        c.skeleton = secondary.opacity(0.6);
        c.selection = hex("#e6e1d6").opacity(0.35);
        c.overlay = hsla(0., 0., 0., 0.6);
        c.group_box = card;
        c.group_box_foreground = foreground;
        c.window_border = border;
        c.scrollbar = hsla(0., 0., 0., 0.);
        c.scrollbar_thumb = white(0.08);
        c.scrollbar_thumb_hover = white(0.16);
        c.switch = secondary;
        c.slider_bar = foreground;
        c.slider_thumb = foreground;
        c.progress_bar = hex("#e6e1d6");
        c.caret = foreground;
        c.button_primary = primary;
        c.button_primary_hover = primary.opacity(0.9);
        c.button_primary_active = primary.opacity(0.8);
        c.button_primary_foreground = primary_fg;
    });
}

/// Use the user's accent for progress bars and selection.
pub fn set_tint(tint: Hsla, cx: &mut App) {
    if Theme::global(cx).colors.progress_bar == tint {
        return;
    }
    Theme::update(cx, |t| {
        t.colors.progress_bar = tint;
        t.colors.selection = tint.opacity(0.35);
        t.colors.slider_bar = tint;
    });
}
