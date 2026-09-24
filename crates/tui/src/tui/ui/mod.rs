//! Rendering: chrome (header, footer) here; `columns`, `inspector`,
//! `search`, `modals` and the sign-in `stage` in their own modules.

mod columns;
mod inspector;
mod modals;
mod player;
mod search;
mod stage;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    Wrap,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use patina_core::config::ACCENTS;
use patina_core::state::NowPlaying;
use patina_plex::Metadata;
use patina_core::state::Stage;
use patina_plex::stream_type::{AUDIO as STREAM_AUDIO, SUBTITLE as STREAM_SUBTITLE};

use super::app::{App, Level, PLAYLISTS, active_download};
use super::column::{Column, SortKey};
use super::modal::*;
use super::rows::{Act, Tone};
use super::source::{Group, Source};
use crate::format;
use crate::item::{Item, Kind, extra_label};

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub struct Theme {
    pub accent: Color,
    pub dim: Color,
    pub text: Color,
    pub error: Color,
    pub success: Color,
    /// Background for chips and the unfocused selection.
    pub surface: Color,
    pub star: Color,
}

impl Theme {
    /// `accent` is an accent id from the shared settings or a hex colour.
    pub fn new(accent: &str) -> Self {
        let hex = ACCENTS.iter().find(|a| a.id == accent).map_or(accent, |a| a.color);
        Self {
            accent: parse_hex(hex).unwrap_or(Color::Rgb(0xe5, 0xa0, 0x0d)),
            dim: Color::DarkGray,
            text: Color::Reset,
            error: Color::Red,
            success: Color::Green,
            surface: Color::Indexed(237),
            star: Color::Yellow,
        }
    }
}

fn parse_hex(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(Color::Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

pub fn draw(f: &mut Frame, app: &mut App, theme: &Theme) {
    let [header, body, bar, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(player::bar_height(app)),
        Constraint::Length(1),
    ])
    .areas(f.area());

    use columns::draw_columns;
    use inspector::draw_inspector;
    use modals::draw_modal;
    use search::draw_search;
    use stage::draw_stage;

    draw_header(f, app, theme, header);
    if !app.ready() {
        draw_stage(f, app, theme, body);
        draw_footer(f, app, theme, footer);
        if app.modal.is_some() {
            draw_modal(f, app, theme);
        }
        return;
    }

    let (cols_area, insp_area) = if app.show_inspector && body.width >= 90 {
        let [a, b] = Layout::horizontal([Constraint::Percentage(62), Constraint::Percentage(38)])
            .areas(body);
        (a, Some(b))
    } else {
        (body, None)
    };
    draw_columns(f, app, theme, cols_area);
    if let Some(area) = insp_area {
        draw_inspector(f, app, theme, area);
    }
    player::draw_now_playing(f, app, theme, bar);
    draw_footer(f, app, theme, footer);

    if app.search.is_some() {
        draw_search(f, app, theme);
    }
    if app.modal.is_some() {
        draw_modal(f, app, theme);
    }
}

// ---- text helpers ----------------------------------------------------------

fn truncate(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = c.width().unwrap_or(0);
        if w + cw > max.saturating_sub(1) {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out
}

fn spinner(app: &App) -> &'static str {
    SPINNER[app.spinner % SPINNER.len()]
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(4)).max(10);
    let h = height.min(area.height.saturating_sub(2)).max(3);
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}

fn modal_block<'a>(title: impl Into<Line<'a>>, theme: &Theme) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent))
        .title(title.into())
}

/// A vertical scrollbar drawn over the right border of `area`, if content overflows.
fn scrollbar(f: &mut Frame, area: Rect, total: usize, visible: usize, pos: usize, color: Color) {
    if total <= visible || area.height < 3 {
        return;
    }
    let mut state = ScrollbarState::new(total.saturating_sub(visible))
        .position(pos)
        .viewport_content_length(visible);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(None)
            .thumb_symbol("┃")
            .thumb_style(Style::default().fg(color)),
        Rect {
            y: area.y + 1,
            height: area.height - 2,
            ..area
        },
        &mut state,
    );
}

/// Paints `style` over every span of a row and pads it to `width`.
fn highlight(mut line: Line<'static>, width: usize, style: Style) -> Line<'static> {
    let pad = width.saturating_sub(line.width());
    if pad > 0 {
        line.spans.push(Span::raw(" ".repeat(pad)));
    }
    for s in line.spans.iter_mut() {
        s.style = s.style.patch(style);
    }
    line
}

fn selected_style(theme: &Theme) -> Style {
    Style::default().bg(theme.accent).fg(Color::Black)
}

// ---- header / footer -------------------------------------------------------

fn draw_header(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let mut spans = vec![
        Span::styled(
            " Patina ",
            Style::default().bg(theme.accent).fg(Color::Black).bold(),
        ),
        Span::raw(" "),
    ];
    let n = app.columns.len();
    for (i, col) in app.columns.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" › ", Style::default().fg(theme.dim)));
        }
        let style = if i + 1 == n {
            Style::default().bold()
        } else {
            Style::default().fg(theme.dim)
        };
        spans.push(Span::styled(truncate(&col.title, 32), style));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);

    let mut right = Vec::new();
    if let Some(np) = app.now_playing.as_ref().filter(|_| !app.ready()) {
        right.push(Span::styled(format!("{} ", now_playing(np)), Style::default().fg(theme.accent)));
    }
    if let Some(name) = app.username() {
        right.push(Span::styled(format!("{name} "), Style::default().fg(theme.dim)));
    }
    f.render_widget(
        Paragraph::new(Line::from(right)).alignment(Alignment::Right),
        area,
    );
}

fn draw_footer(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let left: Line = if let Some(n) = &app.notice {
        let color = match n.level {
            Level::Info => theme.text,
            Level::Success => theme.success,
            Level::Error => theme.error,
        };
        Line::from(Span::styled(
            format!(" {}", n.text),
            Style::default().fg(color),
        ))
    } else if let Some(col) = app.columns.last().filter(|c| c.filtering) {
        Line::from(vec![
            Span::styled(" / ", Style::default().fg(theme.accent).bold()),
            Span::raw(col.filter.clone()),
            Span::styled("█", Style::default().fg(theme.accent)),
            Span::styled("   enter keep · esc clear", Style::default().fg(theme.dim)),
        ])
    } else if let Some(item) = app.selected().filter(|i| i.row.is_some()) {
        let mut hints = row_hints(item);
        if item.link.is_some() && !hints.iter().any(|h| h.0 == "l") {
            hints.push(("l", "open"));
        }
        hints.extend([("h", "back"), ("/", "filter"), ("m", "menu"), ("?", "help")]);
        let mut line = key_hints(&hints, theme);
        line.spans.insert(0, Span::raw(" "));
        line
    } else {
        let playable = app.selected().is_some_and(Item::is_playable);
        let mut hints = vec![("enter", if playable { "play" } else { "open" })];
        if playable {
            hints.push(("o", "options"));
        }
        if app.columns.len() > 1 {
            hints.push(("h", "back"));
        }
        if app.columns.last().is_some_and(|c| c.library().is_some()) {
            hints.push(("F", "filters"));
        }
        hints.extend([
            ("f", "search"),
            ("/", "filter"),
            ("s", "sort"),
            ("m", "menu"),
            ("?", "help"),
        ]);
        let mut spans = vec![Span::raw(" ")];
        for (i, (k, d)) in hints.into_iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("  ", Style::default().fg(theme.dim)));
            }
            spans.push(Span::styled(k, Style::default().fg(theme.accent).bold()));
            spans.push(Span::styled(
                format!(" {d}"),
                Style::default().fg(theme.dim),
            ));
        }
        Line::from(spans)
    };
    f.render_widget(Paragraph::new(left), area);

    let mut right: Vec<Span> = Vec::new();
    if let Some((title, received, size)) = active_download(&app.downloads) {
        let pct = if size > 0 { format!(" {}%", received * 100 / size) } else { format!(" {}", format::bytes(received)) };
        right.push(Span::styled(format!("↓ {}{pct}  ", truncate(&title, 20)), Style::default().fg(theme.accent)));
    }
    if let Some(a) = app.activities.first() {
        let pct = if a.progress >= 0 { format!(" {}%", a.progress) } else { String::new() };
        let more = if app.activities.len() > 1 {
            format!(" +{}", app.activities.len() - 1)
        } else {
            String::new()
        };
        let what = match &a.subtitle {
            Some(sub) => format!("{} · {sub}", a.title),
            None => a.title.clone(),
        };
        right.push(Span::styled(
            format!("⟳ {}{pct}{more}  ", truncate(&what, 36)),
            Style::default().fg(theme.dim),
        ));
    }
    if app.is_busy() {
        let what = if app.pending > 0 { "working" } else { "loading" };
        right.push(Span::styled(
            format!("{} {what} ", spinner(app)),
            Style::default().fg(theme.dim),
        ));
    }
    f.render_widget(
        Paragraph::new(Line::from(right)).alignment(Alignment::Right),
        area,
    );
}

/// `▶ Title 1:05 / 42:10` for the header.
fn now_playing(np: &NowPlaying) -> String {
    let title = truncate(&Item::from(np.item.clone()).full_title(), 40);
    let icon = if np.paused { "‖" } else { "▶" };
    let mut s = format!("{icon} {title}");
    if np.live {
        s.push_str(" · live");
    } else if np.duration_ms > 0 {
        s.push_str(&format!(" {} / {}", format::clock_ms(np.position_ms), format::clock_ms(np.duration_ms)));
    }
    if np.queue_len > 1 {
        s.push_str(&format!(" ({}/{})", np.queue_pos + 1, np.queue_len));
    }
    s
}

/// What enter and the other keys do on a tool row.
pub(super) fn row_hints(item: &Item) -> Vec<(&'static str, &'static str)> {
    use patina_core::downloads::State;
    let Some(r) = &item.row else { return Vec::new() };
    match &r.act {
        Act::Download(d) => match d.state {
            State::Done => vec![("enter", "play"), ("a", "play group"), ("x", "delete")],
            State::Failed => vec![("enter", "retry"), ("x", "remove")],
            _ => vec![("x", "cancel")],
        },
        Act::Channel(_) => vec![("enter", "watch"), ("l", "later programs")],
        Act::Program { program } if program.guid.is_some() => vec![("enter", "record")],
        Act::Rule(_) => vec![("x", "delete rule")],
        Act::Activity { cancellable: true, .. } => vec![("x", "cancel")],
        Act::Transcode(_) => vec![("x", "stop")],
        Act::MoreHistory => vec![("enter", "load older")],
        Act::Butler(_) => vec![("enter", "start"), ("x", "stop")],
        Act::Server(_) | Act::Account(_) => vec![("enter", "run")],
        Act::OpenUrl(_) => vec![("enter", "open")],
        Act::Library(_) => vec![("enter", "actions"), ("l", "settings")],
        Act::Pref { .. } | Act::Setting(_) => vec![("enter", "change")],
        _ => Vec::new(),
    }
}

// ---- sign-in ---------------------------------------------------------------
fn key_hints(hints: &[(&str, &str)], theme: &Theme) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, (k, d)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(
            k.to_string(),
            Style::default().fg(theme.accent).bold(),
        ));
        spans.push(Span::styled(
            format!(" {d}"),
            Style::default().fg(theme.dim),
        ));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::inspector::progress_bar;
    use super::*;

    #[test]
    fn truncates_by_display_width() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 6), "hello…");
        assert_eq!(truncate("日本語テキスト", 5), "日本…");
        assert_eq!(truncate("abc", 0), "");
    }

    #[test]
    fn parses_accent() {
        assert_eq!(parse_hex("#ff0080"), Some(Color::Rgb(255, 0, 128)));
        assert_eq!(parse_hex("nope"), None);
        assert_eq!(Theme::new("amber").accent, Color::Rgb(0xd6, 0xa4, 0x4c));
        assert_eq!(Theme::new("").accent, Color::Rgb(0xe5, 0xa0, 0x0d));
    }

    #[test]
    fn now_playing_line() {
        let np = NowPlaying {
            item: patina_plex::Metadata { title: Some("Heat".into()), kind: Some("movie".into()), ..Default::default() },
            position_ms: 65_000,
            duration_ms: 600_000,
            paused: true,
            transcoding: false,
            queue_len: 1,
            queue_pos: 0,
            uid: 1,
            ending: false,
            stop_after: false,
            live: false,
            local: false,
        };
        assert_eq!(now_playing(&np), "‖ Heat 1:05 / 10:00");
    }

    #[test]
    fn progress_bar_width() {
        assert_eq!(progress_bar(0.5, 4), "━━──");
        assert_eq!(progress_bar(1.5, 2), "━━");
    }
}
