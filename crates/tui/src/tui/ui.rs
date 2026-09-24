//! Rendering.

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
use patina_core::state::Stage;
use patina_plex::stream_type::{AUDIO as STREAM_AUDIO, SUBTITLE as STREAM_SUBTITLE};

use super::app::{App, CONTINUE, HISTORY, Level, PLAYLISTS, RECENT, SESSIONS, WATCHLIST, active_download};
use super::column::{Column, SortKey};
use super::modal::*;
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
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
    ])
    .areas(f.area());

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
    if let Some(np) = &app.now_playing {
        right.push(Span::styled(format!("{} ", now_playing(np)), Style::default().fg(theme.accent)));
        if np.ending && np.queue_pos + 1 < np.queue_len {
            right.push(Span::styled("Up next ", Style::default().fg(theme.dim)));
        }
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
    } else {
        let playable = app.selected().is_some_and(Item::is_playable);
        let mut hints = vec![("enter", if playable { "play" } else { "open" })];
        if playable {
            hints.push(("o", "options"));
        }
        if app.columns.len() > 1 {
            hints.push(("h", "back"));
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

// ---- sign-in ---------------------------------------------------------------

fn draw_stage(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let dim = Style::default().fg(theme.dim);
    let accent = Style::default().fg(theme.accent);
    let hints = |h: &[(&str, &str)]| key_hints(h, theme);
    let mut lines: Vec<Line> = Vec::new();
    let title;
    match &app.stage {
        Stage::Booting | Stage::Ready => {
            title = "Patina";
            lines.push(Line::from(Span::styled(format!("{} Starting…", spinner(app)), dim)));
        }
        Stage::Connecting { name } => {
            title = "Connecting";
            lines.push(Line::from(Span::styled(format!("{} Connecting to {name}…", spinner(app)), dim)));
        }
        Stage::Login => {
            title = "Sign in";
            match &app.login.code {
                Some((code, _)) => {
                    lines.push(Line::from(vec![Span::raw("Go to "), Span::styled("plex.tv/link", accent.bold()), Span::raw(" and enter")]));
                    lines.push(Line::raw(""));
                    lines.push(Line::from(Span::styled(format!("   {code}   "), selected_style(theme).bold())));
                    lines.push(Line::raw(""));
                    lines.push(Line::from(Span::styled("or press o to sign in with a browser", dim)));
                    lines.push(Line::from(Span::styled(format!("{} Waiting for plex.tv…", spinner(app)), dim)));
                }
                None if app.login.requesting => {
                    lines.push(Line::from(Span::styled(format!("{} Asking plex.tv for a code…", spinner(app)), dim)));
                }
                None => {}
            }
            if let Some(e) = &app.login.error {
                lines.push(Line::raw(""));
                lines.push(Line::from(Span::styled(e.clone(), Style::default().fg(theme.error))));
            }
            lines.push(Line::raw(""));
            lines.push(hints(&[("o", "open browser"), ("r", "new code"), ("m", "server URL + token"), ("q", "quit")]));
        }
        Stage::PickServer { servers, user } => {
            title = "Choose a server";
            lines.push(Line::from(Span::styled(format!("Signed in as {}", user.name), dim)));
            lines.push(Line::raw(""));
            if servers.is_empty() {
                lines.push(Line::from(Span::styled("No servers on this account.", Style::default().fg(theme.error))));
            }
            let width = 52;
            for (i, s) in servers.iter().enumerate() {
                let note = match (s.owned, s.presence) {
                    (_, false) => "offline".to_string(),
                    (true, _) => "owned".to_string(),
                    (false, _) => format!("shared by {}", s.source_title.as_deref().unwrap_or("someone")),
                };
                let name = truncate(&s.name, width - note.width() - 4);
                let pad = width.saturating_sub(name.width() + note.width() + 3);
                let line = Line::from(vec![Span::raw(format!(" {name}")), Span::raw(" ".repeat(pad)), Span::styled(format!("{note} "), dim)]);
                lines.push(if i == app.login.cursor { highlight(line, width, selected_style(theme)) } else { line });
            }
            lines.push(Line::raw(""));
            lines.push(hints(&[("enter", "connect"), ("L", "sign out"), ("q", "quit")]));
        }
        Stage::Failed { message } => {
            title = "Couldn't connect";
            lines.push(Line::from(Span::styled(message.clone(), Style::default().fg(theme.error))));
            lines.push(Line::raw(""));
            lines.push(hints(&[("r", "retry"), ("L", "sign out"), ("q", "quit")]));
        }
    }
    let block = modal_block(Span::styled(format!(" {title} "), Style::default().fg(theme.accent).bold()), theme)
        .padding(ratatui::widgets::Padding::uniform(1));
    let para = Paragraph::new(lines).alignment(Alignment::Center).wrap(Wrap { trim: false });
    let width = 60.min(area.width.saturating_sub(4));
    let height = para.line_count(width.saturating_sub(4)) as u16 + 4;
    f.render_widget(para.block(block), centered(area, width, height));
}

// ---- columns ---------------------------------------------------------------

fn nav_icon(key: Option<&str>) -> &'static str {
    match key {
        Some(CONTINUE) => "▶",
        Some(RECENT) => "✦",
        Some(WATCHLIST) => "☆",
        Some(PLAYLISTS) => "≡",
        Some(SESSIONS) => "◉",
        Some(HISTORY) => "↺",
        _ => "•",
    }
}

fn section_icon(kind: Option<&str>) -> &'static str {
    match kind {
        Some("movie") => "▣",
        Some("show") => "▤",
        Some("artist") => "♫",
        Some("photo") => "◩",
        _ => "▢",
    }
}

/// Columns whose items appear outside their natural parent need full titles.
fn context_free(col: &Column) -> bool {
    match col.path.as_deref() {
        None => col.id != 1,
        Some(p) => {
            [CONTINUE, RECENT, HISTORY, SESSIONS, WATCHLIST].contains(&p)
                || p.starts_with("/playlists/")
                || p.ends_with("/related")
                || p.contains("/onDeck")
                || p.contains("/recentlyAdded")
                || p.contains("/recentlyViewed")
                || p.starts_with("/library/collections/")
        }
    }
}

struct RowCtx<'a> {
    app: &'a App,
    theme: &'a Theme,
    full_titles: bool,
    sessions: bool,
    history: bool,
}

fn status_glyph(item: &Item, ctx: &RowCtx) -> Span<'static> {
    let accent = Style::default().fg(ctx.theme.accent);
    let dim = Style::default().fg(ctx.theme.dim);
    match item.kind() {
        Kind::Nav => Span::styled(nav_icon(item.key.as_deref()), accent),
        Kind::Section => Span::styled(section_icon(item.kind.as_deref()), accent),
        Kind::Playlist => Span::styled("≡", dim),
        Kind::Collection => Span::styled("▦", dim),
        Kind::Folder => Span::styled("▸", dim),
        Kind::Artist => Span::styled("♪", dim),
        Kind::Album => Span::styled("◎", dim),
        Kind::PhotoAlbum => Span::styled("▢", dim),
        Kind::Remote => Span::styled("☆", dim),
        _ if ctx.sessions => match item.player.as_ref().and_then(|p| p.state.as_deref()) {
            Some("paused") => Span::styled("‖", accent),
            Some("buffering") => Span::styled("…", accent),
            _ => Span::styled("▶", accent),
        },
        _ if ctx.history => Span::styled("✓", dim),
        _ if !ctx.app.tui.show_watch_status || !item.is_watchable() => Span::raw(" "),
        Kind::Show | Kind::Season => {
            if item.is_watched() {
                Span::raw(" ")
            } else if item.viewed_leaf_count.unwrap_or(0) > 0 {
                Span::styled("◐", accent)
            } else {
                Span::styled("●", accent)
            }
        }
        _ if item.in_progress() && !item.is_watched() => Span::styled("◐", accent),
        _ if !item.is_watched() => Span::styled("●", accent),
        _ => Span::raw(" "),
    }
}

fn meta(item: &Item, ctx: &RowCtx) -> (String, Style) {
    let dim = Style::default().fg(ctx.theme.dim);
    let accent = Style::default().fg(ctx.theme.accent);
    if ctx.sessions {
        let user = item
            .user
            .as_ref()
            .and_then(|u| u.title.clone())
            .unwrap_or_default();
        let pct = item
            .progress()
            .map(|p| format!(" {:.0}%", p * 100.0))
            .unwrap_or_default();
        return (format!("{user}{pct}"), dim);
    }
    if ctx.history {
        return (
            item.viewed_at.map(format::relative).unwrap_or_default(),
            dim,
        );
    }
    match item.kind() {
        Kind::Section => {
            let id = item.key.as_deref().unwrap_or("");
            match ctx.app.counts.get(id).filter(|_| ctx.app.tui.show_library_counts) {
                Some(n) => (n.to_string(), dim),
                None => (String::new(), dim),
            }
        }
        Kind::Show | Kind::Season if ctx.app.tui.show_watch_status => {
            match item.unwatched_leaves() {
                Some(n) if n > 0 => (n.to_string(), accent),
                _ => (
                    item.year
                        .filter(|_| item.kind() == Kind::Show)
                        .map(|y| y.to_string())
                        .unwrap_or_default(),
                    dim,
                ),
            }
        }
        Kind::Movie | Kind::Show | Kind::Remote => {
            (item.year.map(|y| y.to_string()).unwrap_or_default(), dim)
        }
        Kind::Episode | Kind::Clip if item.in_progress() && !item.is_watched() => (
            item.progress()
                .map(|p| format!("{:.0}%", p * 100.0))
                .unwrap_or_default(),
            accent,
        ),
        Kind::Episode | Kind::Clip => (
            item.duration.map(format::duration_ms).unwrap_or_default(),
            dim,
        ),
        Kind::Track => (item.duration.map(format::clock_ms).unwrap_or_default(), dim),
        Kind::Album => (item.year.map(|y| y.to_string()).unwrap_or_default(), dim),
        Kind::Playlist => (
            item.leaf_count.map(|n| n.to_string()).unwrap_or_default(),
            dim,
        ),
        Kind::Collection => (
            item.child_count.map(|n| n.to_string()).unwrap_or_default(),
            dim,
        ),
        _ => (String::new(), dim),
    }
}

fn row_line(item: &Item, width: usize, ctx: &RowCtx) -> Line<'static> {
    let glyph = status_glyph(item, ctx);
    let title = if ctx.full_titles || ctx.sessions || ctx.history {
        item.full_title()
    } else {
        item.display_title()
    };
    let (meta, meta_style) = meta(item, ctx);
    let meta_w = meta.width();
    let lead = 2 + glyph.content.width();
    let avail = width.saturating_sub(lead + if meta_w > 0 { meta_w + 1 } else { 0 });
    let title = truncate(&title, avail);
    let pad = width.saturating_sub(lead + title.width() + meta_w);
    let watched_dim = ctx.app.tui.show_watch_status
        && item.is_watchable()
        && item.is_watched()
        && !ctx.sessions
        && !ctx.history;
    let title_style = if watched_dim || item.kind() == Kind::Remote {
        Style::default().fg(ctx.theme.dim)
    } else {
        Style::default()
    };
    Line::from(vec![
        Span::raw(" "),
        glyph,
        Span::raw(" "),
        Span::styled(title, title_style),
        Span::raw(" ".repeat(pad)),
        Span::styled(meta, meta_style),
    ])
}

fn draw_columns(f: &mut Frame, app: &mut App, theme: &Theme, area: Rect) {
    let n_visible = match area.width {
        w if w >= 140 => 3,
        w if w >= 70 => 2,
        _ => 1,
    }
    .min(app.columns.len());
    let constraints: Vec<Constraint> = match n_visible {
        1 => vec![Constraint::Percentage(100)],
        2 => vec![Constraint::Percentage(38), Constraint::Percentage(62)],
        _ => vec![
            Constraint::Percentage(24),
            Constraint::Percentage(30),
            Constraint::Percentage(46),
        ],
    };
    let areas = Layout::horizontal(constraints).split(area);
    let first = app.columns.len() - n_visible;

    for (slot, idx) in (first..app.columns.len()).enumerate() {
        let rect = areas[slot];
        let focused = idx + 1 == app.columns.len();
        let inner_h = rect.height.saturating_sub(2) as usize;
        if focused {
            app.list_height = inner_h;
        }
        app.columns[idx].clamp_scroll(inner_h);

        let app_ref: &App = app;
        let col = &app_ref.columns[idx];
        let border = if focused { theme.accent } else { theme.dim };
        let mut title_spans = vec![Span::styled(
            format!(
                " {} ",
                truncate(&col.title, rect.width.saturating_sub(14) as usize)
            ),
            if focused {
                Style::default().fg(theme.accent).bold()
            } else {
                Style::default().fg(theme.dim)
            },
        )];
        if col.loaded || !col.items.is_empty() {
            title_spans.push(Span::styled(
                format!("{} ", col.count_label()),
                Style::default().fg(theme.dim),
            ));
        }
        if col.loading {
            title_spans.push(Span::styled(
                format!("{} ", spinner(app_ref)),
                Style::default().fg(theme.accent),
            ));
        }
        let mut block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border))
            .title(Line::from(title_spans));
        let mut bottom = Vec::new();
        if !col.filter.is_empty() {
            bottom.push(Span::styled(
                format!(" /{} ", col.filter),
                Style::default().fg(theme.accent),
            ));
        }
        if col.sort != SortKey::Default {
            bottom.push(Span::styled(
                format!(
                    " {} {} ",
                    col.sort.label(),
                    if col.sort_desc { "↓" } else { "↑" }
                ),
                Style::default().fg(theme.dim),
            ));
        }
        if col.hide_watched {
            bottom.push(Span::styled(
                " unwatched only ",
                Style::default().fg(theme.dim),
            ));
        }
        if !bottom.is_empty() {
            block = block.title_bottom(Line::from(bottom));
        }
        if focused && col.view.len() > inner_h {
            block = block.title_bottom(
                Line::from(Span::styled(
                    format!(" {}/{} ", col.cursor + 1, col.view.len()),
                    Style::default().fg(theme.dim),
                ))
                .right_aligned(),
            );
        }
        let inner = block.inner(rect);
        f.render_widget(block, rect);

        let ctx = RowCtx {
            app: app_ref,
            theme,
            full_titles: context_free(col),
            sessions: col.is_sessions(),
            history: col.path.as_deref() == Some(HISTORY),
        };

        if col.view.is_empty() {
            let msg = if let Some(e) = &col.error {
                Line::from(vec![Span::styled(
                    format!(" {} ", truncate(e, inner.width as usize)),
                    Style::default().fg(theme.error),
                )])
            } else if col.loading || !col.loaded {
                Line::from(Span::styled(
                    format!(" {} Loading…", spinner(app_ref)),
                    Style::default().fg(theme.dim),
                ))
            } else if !col.filter.is_empty() || col.hide_watched {
                Line::from(Span::styled(" No matches", Style::default().fg(theme.dim)))
            } else if col.is_sessions() {
                Line::from(Span::styled(
                    " Nobody is streaming right now",
                    Style::default().fg(theme.dim),
                ))
            } else {
                Line::from(Span::styled(" Empty", Style::default().fg(theme.dim)))
            };
            let mut lines = vec![msg];
            if col.error.is_some() {
                lines.push(Line::from(Span::styled(
                    " press r to retry",
                    Style::default().fg(theme.dim),
                )));
            }
            f.render_widget(Paragraph::new(lines), inner);
            continue;
        }

        let width = inner.width as usize;
        let lines: Vec<Line> = col
            .view
            .iter()
            .enumerate()
            .skip(col.scroll)
            .take(inner.height as usize)
            .map(|(pos, &i)| {
                let line = row_line(&col.items[i], width, &ctx);
                match pos == col.cursor {
                    true if focused => highlight(line, width, selected_style(theme)),
                    true => highlight(line, width, Style::default().bg(theme.surface)),
                    false => line,
                }
            })
            .collect();
        f.render_widget(Paragraph::new(lines), inner);
        scrollbar(
            f,
            rect,
            col.view.len(),
            inner.height as usize,
            col.scroll,
            border,
        );

        if col.error.is_some() && inner.height > 0 {
            let r = Rect {
                y: inner.y + inner.height - 1,
                height: 1,
                ..inner
            };
            f.render_widget(Clear, r);
            f.render_widget(
                Paragraph::new(Span::styled(
                    " refresh failed (r to retry)",
                    Style::default().fg(theme.error),
                )),
                r,
            );
        }
    }
}

// ---- inspector -------------------------------------------------------------

const LABEL_W: usize = 10;

fn kv<'a>(k: &str, v: impl Into<String>, theme: &Theme) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("{k:<LABEL_W$}"), Style::default().fg(theme.dim)),
        Span::raw(v.into()),
    ])
}

/// A section heading followed by a rule to the panel edge, preceded by a gap.
fn heading(lines: &mut Vec<Line<'static>>, title: &str, width: usize, theme: &Theme) {
    lines.push(Line::raw(""));
    let rule = width.saturating_sub(title.width() + 1);
    lines.push(Line::from(vec![
        Span::styled(title.to_string(), Style::default().fg(theme.accent).bold()),
        Span::raw(" "),
        Span::styled("─".repeat(rule), Style::default().fg(theme.surface)),
    ]));
}

fn tags(list: &[patina_plex::Tag], n: usize) -> String {
    let names: Vec<&str> = list
        .iter()
        .map(|t| t.name())
        .filter(|s| !s.is_empty())
        .take(n)
        .collect();
    let mut s = names.join(", ");
    if list.len() > n {
        s.push_str(&format!(" +{}", list.len() - n));
    }
    s
}

fn progress_bar(frac: f64, width: usize) -> String {
    let filled = ((frac * width as f64).round() as usize).min(width);
    format!("{}{}", "━".repeat(filled), "─".repeat(width - filled))
}

/// Pushes a labelled track list, selected tracks first, capped at a few rows.
fn track_list(
    lines: &mut Vec<Line<'static>>,
    label: &str,
    streams: &[&patina_plex::Stream],
    theme: &Theme,
) {
    const MAX: usize = 4;
    let mut sorted: Vec<_> = streams.to_vec();
    sorted.sort_by_key(|s| !s.selected);
    for (i, s) in sorted.iter().take(MAX).enumerate() {
        let lead = if i == 0 { label } else { "" };
        let (mark, style) = if s.selected {
            ("● ", Style::default().fg(theme.accent))
        } else {
            ("  ", Style::default())
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{lead:<LABEL_W$}"), Style::default().fg(theme.dim)),
            Span::styled(mark, style),
            Span::styled(s.label(), style),
        ]));
    }
    if sorted.len() > MAX {
        lines.push(Line::from(Span::styled(
            format!(
                "{:LABEL_W$}  +{} more · o to choose",
                "",
                sorted.len() - MAX
            ),
            Style::default().fg(theme.dim),
        )));
    }
}

fn inspector_lines(app: &App, item: &Item, theme: &Theme, width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    let accent = Style::default().fg(theme.accent);
    let dim = Style::default().fg(theme.dim);

    lines.push(Line::from(Span::styled(item.title().to_string(), accent.bold())));
    let context = match item.kind() {
        Kind::Episode => Some(format!(
            "{} · Season {} · Episode {}",
            item.grandparent_title.as_deref().unwrap_or(""),
            item.parent_index.unwrap_or(0),
            item.index.unwrap_or(0)
        )),
        Kind::Season => item.parent_title.clone(),
        Kind::Track => Some(format!(
            "{} · {}",
            item.grandparent_title.as_deref().unwrap_or(""),
            item.parent_title.as_deref().unwrap_or("")
        )),
        Kind::Album => item.parent_title.clone(),
        _ => item.original_title.clone().filter(|o| o != item.title()),
    };
    if let Some(c) = context {
        lines.push(Line::from(Span::styled(c, dim)));
    }

    // Facts line.
    let mut facts: Vec<Span> = Vec::new();
    if let Some(y) = item.year {
        facts.push(Span::raw(y.to_string()));
    }
    if let Some(c) = &item.content_rating {
        facts.push(Span::raw(c.clone()));
    }
    if let Some(d) = item.duration {
        facts.push(Span::raw(format::duration_ms(d)));
    }
    if let Some(r) = item.audience_rating.or(item.rating).filter(|r| *r > 0.0) {
        facts.push(Span::styled(
            format!("★ {r:.1}"),
            Style::default().fg(theme.star),
        ));
    }
    if let Some(r) = item.user_rating {
        facts.push(Span::styled(format!("you {r:.0}/10"), accent));
    }
    if !facts.is_empty() {
        let mut spans = Vec::new();
        for (i, f) in facts.into_iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" · ", dim));
            }
            spans.push(f);
        }
        lines.push(Line::from(spans));
    }

    // Genre chips.
    if !item.genres.is_empty() {
        let chip = Style::default().bg(theme.surface);
        let mut spans = Vec::new();
        for g in item
            .genres
            .iter()
            .map(|t| t.name())
            .filter(|g| !g.is_empty())
            .take(5)
        {
            spans.push(Span::styled(format!(" {g} "), chip));
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
    }

    // Watch status.
    match item.kind() {
        Kind::Show | Kind::Season | Kind::Artist | Kind::Album | Kind::Collection => {
            if let (Some(total), Some(unwatched)) = (item.leaf_count, item.unwatched_leaves()) {
                let label = match item.kind() {
                    Kind::Artist | Kind::Album => "tracks",
                    _ => "episodes",
                };
                let mut s = format!("{total} {label}");
                if let Some(c) = item.child_count.filter(|_| item.kind() == Kind::Show) {
                    s = format!("{c} season{} · {s}", if c == 1 { "" } else { "s" });
                }
                lines.push(Line::from(Span::styled(s, dim)));
                if item.is_watchable() && total > 0 {
                    let watched = total.saturating_sub(unwatched);
                    let bar_w = width.saturating_sub(18).clamp(8, 30);
                    let note = if unwatched == 0 {
                        " all watched".to_string()
                    } else {
                        format!(" {unwatched} unwatched")
                    };
                    lines.push(Line::from(vec![
                        Span::styled(progress_bar(watched as f64 / total as f64, bar_w), accent),
                        Span::styled(note, dim),
                    ]));
                }
            } else if let Some(c) = item.child_count {
                lines.push(Line::from(Span::styled(format!("{c} items"), dim)));
            }
        }
        _ if item.is_watchable() => {
            if let (true, Some(p)) = (item.in_progress(), item.progress()) {
                let left = item.duration.unwrap_or(0) - item.view_offset.unwrap_or(0);
                let bar_w = width.saturating_sub(22).clamp(8, 30);
                lines.push(Line::from(vec![
                    Span::styled(progress_bar(p, bar_w), accent),
                    Span::styled(format!(" {:.0}%", p * 100.0), accent),
                    Span::styled(format!(" · {} left", format::duration_ms(left)), dim),
                ]));
            } else if item.is_watched() {
                let when = item
                    .last_viewed_at
                    .map(|t| format!(" · {}", format::relative(t)))
                    .unwrap_or_default();
                lines.push(Line::from(vec![
                    Span::styled("✓ Watched", Style::default().fg(theme.success)),
                    Span::styled(when, dim),
                ]));
            } else {
                lines.push(Line::from(Span::styled("● Unwatched", accent)));
            }
        }
        _ => {}
    }

    if item.kind() == Kind::Remote {
        lines.push(Line::from(Span::styled(
            "Not on this server",
            Style::default().fg(theme.dim),
        )));
    }
    if item
        .guid
        .as_ref()
        .is_some_and(|g| app.watchlist.contains(g))
    {
        lines.push(Line::from(Span::styled("☆ On your watchlist", accent)));
    }

    // Session details.
    if let Some(user) = &item.user {
        heading(&mut lines, "Stream", width, theme);
        lines.push(kv("User", user.title.clone().unwrap_or_default(), theme));
        if let Some(p) = &item.player {
            lines.push(kv(
                "Player",
                format!(
                    "{} ({} {})",
                    p.title.as_deref().unwrap_or(""),
                    p.product.as_deref().unwrap_or(""),
                    p.platform.as_deref().unwrap_or("")
                ),
                theme,
            ));
            lines.push(kv(
                "State",
                format!(
                    "{}{}",
                    p.state.as_deref().unwrap_or("?"),
                    if p.local {
                        " · local"
                    } else {
                        " · remote"
                    }
                ),
                theme,
            ));
            if let Some(a) = &p.address {
                lines.push(kv("Address", a.clone(), theme));
            }
        }
        if let (Some(o), Some(d)) = (item.view_offset, item.duration) {
            lines.push(kv(
                "Position",
                format!("{} / {}", format::clock_ms(o), format::clock_ms(d)),
                theme,
            ));
        }
        match &item.transcode_session {
            Some(t) => lines.push(kv(
                "Stream",
                format!(
                    "video {} · audio {}{}",
                    t.video_decision.as_deref().unwrap_or("copy"),
                    t.audio_decision.as_deref().unwrap_or("copy"),
                    t.speed.map(|s| format!(" · {s:.1}x")).unwrap_or_default()
                ),
                theme,
            )),
            None => lines.push(kv("Stream", "direct play", theme)),
        }
        if let Some(b) = item.session.as_ref().and_then(|s| s.bandwidth) {
            lines.push(kv("Bandwidth", format::bitrate_kbps(b), theme));
        }
    }

    if let Some(t) = &item.tagline {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            t.clone(),
            Style::default().add_modifier(Modifier::ITALIC),
        )));
    }
    if let Some(s) = item.summary.as_deref().filter(|s| !s.is_empty()) {
        lines.push(Line::raw(""));
        for para in s.split('\n').filter(|p| !p.trim().is_empty()) {
            lines.push(Line::raw(para.trim().to_string()));
        }
    }

    // Credits.
    let mut credits = Vec::new();
    if !item.directors.is_empty() {
        credits.push(kv("Director", tags(&item.directors, 3), theme));
    }
    if !item.writers.is_empty() {
        credits.push(kv("Writer", tags(&item.writers, 3), theme));
    }
    if !item.roles.is_empty() {
        let cast: Vec<String> = item
            .roles
            .iter()
            .take(6)
            .map(|r| match &r.role {
                Some(role) if !role.is_empty() => format!("{} ({role})", r.name()),
                _ => r.name().to_string(),
            })
            .collect();
        credits.push(kv("Cast", cast.join(", "), theme));
    }
    if let Some(s) = &item.studio {
        credits.push(kv("Studio", s.clone(), theme));
    }
    if !item.countries.is_empty() {
        credits.push(kv("Country", tags(&item.countries, 3), theme));
    }
    if !item.collections.is_empty() {
        credits.push(kv("In", tags(&item.collections, 3), theme));
    }
    if !item.labels.is_empty() {
        credits.push(kv("Labels", tags(&item.labels, 5), theme));
    }
    if let Some(d) = &item.originally_available_at {
        credits.push(kv("Released", d.clone(), theme));
    }
    if !credits.is_empty() {
        heading(&mut lines, "Credits", width, theme);
        lines.extend(credits);
    }

    // Media.
    if let Some(media) = item.media.first() {
        heading(&mut lines, "Media", width, theme);
        let v = [
            format::resolution(media.height, media.video_resolution.as_deref()),
            media.video_codec.as_deref().map(format::codec),
            media
                .video_profile
                .clone()
                .filter(|p| p.contains("10") || p.contains("dv") || p.contains("hdr")),
            media.video_frame_rate.clone(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        if !v.is_empty() {
            lines.push(kv("Video", v, theme));
        }
        let a = [
            media.audio_codec.as_deref().map(format::codec),
            media.audio_channels.map(format::channels),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ");
        if !a.is_empty() {
            lines.push(kv("Audio", a, theme));
        }
        let f = [
            media.container.as_deref().map(str::to_uppercase),
            media.bitrate.map(format::bitrate_kbps),
            media.parts.first().and_then(|p| p.size).map(format::bytes),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        if !f.is_empty() {
            lines.push(kv("File", f, theme));
        }
        if item.media.len() > 1 {
            lines.push(kv("Versions", item.media.len().to_string(), theme));
        }
        if let Some(part) = media.parts.first() {
            let of = |t| -> Vec<_> {
                part.streams
                    .iter()
                    .filter(|s| s.stream_type == Some(t))
                    .collect()
            };
            let audio = of(STREAM_AUDIO);
            if audio.len() > 1 {
                track_list(&mut lines, "Tracks", &audio, theme);
            }
            let subs = of(STREAM_SUBTITLE);
            if !subs.is_empty() {
                track_list(&mut lines, "Subtitles", &subs, theme);
            }
            if let Some(file) = &part.file {
                lines.push(Line::raw(""));
                lines.push(Line::from(Span::styled(file.clone(), dim)));
            }
        }
    }

    // Markers and chapters.
    let markers: Vec<String> = item
        .markers
        .iter()
        .filter_map(|m| {
            let (s, e) = (m.start_time_offset?, m.end_time_offset?);
            let label = match m.kind.as_deref() {
                Some("intro") => "Intro",
                Some("credits") => "Credits",
                Some("commercial") => "Ad",
                _ => return None,
            };
            Some(format!(
                "{label} {}–{}",
                format::clock_ms(s),
                format::clock_ms(e)
            ))
        })
        .collect();
    if !markers.is_empty() || !item.chapters.is_empty() {
        heading(&mut lines, "Timeline", width, theme);
        if !markers.is_empty() {
            lines.push(kv("Markers", markers.join(" · "), theme));
        }
        const MAX_CHAPTERS: usize = 8;
        for (i, c) in item.chapters.iter().take(MAX_CHAPTERS).enumerate() {
            let name = c
                .tag
                .clone()
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| format!("Chapter {}", c.index.unwrap_or(i as i64 + 1)));
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:<LABEL_W$}", if i == 0 { "Chapters" } else { "" }),
                    dim,
                ),
                Span::styled(
                    format!(
                        "{:>8}  ",
                        format::clock_ms(c.start_time_offset.unwrap_or(0))
                    ),
                    dim,
                ),
                Span::raw(name),
            ]));
        }
        if item.chapters.len() > MAX_CHAPTERS {
            lines.push(Line::from(Span::styled(
                format!(
                    "{:LABEL_W$}+{} more",
                    "",
                    item.chapters.len() - MAX_CHAPTERS
                ),
                dim,
            )));
        }
    }

    // Extras.
    if let Some(extras) = item.extras.as_ref().filter(|e| !e.metadata.is_empty()) {
        heading(&mut lines, "Extras", width, theme);
        const MAX_EXTRAS: usize = 5;
        for x in extras.metadata.iter().take(MAX_EXTRAS) {
            let kind = extra_label(x.extra_type, x.subtype.as_deref());
            let dur = x
                .duration
                .map(|d| format!(" · {}", format::clock_ms(d)))
                .unwrap_or_default();
            lines.push(Line::from(vec![
                Span::styled(format!("{kind:<18}"), dim),
                Span::raw(x.title().to_string()),
                Span::styled(dur, dim),
            ]));
        }
        let total = extras.size.unwrap_or(extras.metadata.len() as i64) as usize;
        lines.push(Line::from(Span::styled(
            if total > MAX_EXTRAS {
                format!("+{} more · m → Extras", total - MAX_EXTRAS)
            } else {
                "m → Extras to play".to_string()
            },
            dim,
        )));
    }

    // Library sections.
    if item.kind() == Kind::Section
        && let Some(dir) = &item.dir
    {
        heading(&mut lines, "Library", width, theme);
        lines.push(kv("Type", dir.kind.clone().unwrap_or_default(), theme));
        if let Some(n) = dir.key.as_deref().and_then(|k| app.counts.get(k)) {
            lines.push(kv("Items", n.to_string(), theme));
        }
        if let Some(a) = &dir.agent {
            lines.push(kv("Agent", a.clone(), theme));
        }
        if let Some(s) = &dir.scanner {
            lines.push(kv("Scanner", s.clone(), theme));
        }
        if let Some(t) = dir.scanned_at {
            lines.push(kv("Scanned", format::relative(t), theme));
        }
        let scanning = dir.refreshing || app.activities.iter().any(|a| a.section_id.as_deref() == dir.key.as_deref());
        if scanning {
            lines.push(Line::from(Span::styled("Scanning now…", accent)));
        }
        for l in dir.location.iter().filter_map(|l| l.path.clone()) {
            lines.push(kv("Folder", l, theme));
        }
        lines.push(Line::raw(""));
        lines.push(key_hints(
            &[("v", "browse by"), ("S", "scan"), ("m", "more")],
            theme,
        ));
    }

    if item.kind() == Kind::Nav {
        lines.push(Line::raw(""));
        let text = match item.key.as_deref() {
            Some(CONTINUE) => {
                "Movies and episodes in progress, plus the next episode of shows you're watching."
            }
            Some(RECENT) => "The newest additions across every library.",
            Some(WATCHLIST) => {
                "Your plex.tv watchlist. Titles on this server are playable; ☆ marks ones that aren't. + adds or removes the selection."
            }
            Some(PLAYLISTS) => {
                "Your playlists. Space adds the selected item to one; x deletes; e renames."
            }
            Some(SESSIONS) => {
                "Who is streaming from this server right now. Refreshes every few seconds; x stops a stream."
            }
            Some(HISTORY) => "Everything played on this server, newest first.",
            _ => "",
        };
        lines.push(Line::from(Span::styled(text, dim)));
    }

    if item.kind() == Kind::Playlist {
        heading(&mut lines, "Playlist", width, theme);
        if let Some(n) = item.leaf_count {
            lines.push(kv("Items", n.to_string(), theme));
        }
        if let Some(d) = item.duration {
            lines.push(kv("Length", format::duration_ms(d), theme));
        }
        lines.push(kv(
            "Type",
            format!(
                "{}{}",
                item.playlist_type.as_deref().unwrap_or(""),
                if item.smart {
                    " · smart"
                } else {
                    ""
                }
            ),
            theme,
        ));
    }

    // Timestamps.
    let mut times = Vec::new();
    if let Some(t) = item.added_at {
        times.push(format!("added {}", format::date(t)));
    }
    if let Some(t) = item.last_viewed_at.filter(|_| !item.is_watched()) {
        times.push(format!("viewed {}", format::relative(t)));
    }
    if let Some(t) = item.viewed_at {
        times.push(format!("played {}", format::relative(t)));
    }
    if !times.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(times.join(" · "), dim)));
    }
    lines
}

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

fn draw_inspector(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.dim))
        .title(Span::styled(" Details ", Style::default().fg(theme.dim)));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let Some(item) = app.selected_detail() else {
        f.render_widget(
            Paragraph::new(Span::styled(
                " Nothing selected",
                Style::default().fg(theme.dim),
            )),
            inner,
        );
        return;
    };
    let text_area = Rect {
        x: inner.x + 1,
        width: inner.width.saturating_sub(2),
        ..inner
    };
    let lines = inspector_lines(app, item, theme, text_area.width as usize);
    let para = Paragraph::new(lines).wrap(Wrap { trim: false });
    let total = para.line_count(text_area.width);
    f.render_widget(para.scroll((app.inspector_scroll, 0)), text_area);
    scrollbar(
        f,
        area,
        total,
        inner.height as usize,
        app.inspector_scroll as usize,
        theme.dim,
    );
}

// ---- search ----------------------------------------------------------------

fn kind_icon(kind: Kind) -> &'static str {
    match kind {
        Kind::Movie => "▣",
        Kind::Show | Kind::Season => "▤",
        Kind::Episode | Kind::Clip => "▸",
        Kind::Artist => "♪",
        Kind::Album => "◎",
        Kind::Track => "♫",
        Kind::Playlist => "≡",
        Kind::Collection => "▦",
        Kind::Remote => "☆",
        _ => "•",
    }
}

fn draw_search(f: &mut Frame, app: &mut App, theme: &Theme) {
    let area = centered(
        f.area(),
        (f.area().width * 7 / 10).max(60),
        (f.area().height * 7 / 10).max(12),
    );
    f.render_widget(Clear, area);
    let Some(s) = &app.search else { return };
    let mut block = modal_block(Line::from(Span::styled(" Search ", Style::default().fg(theme.accent).bold())), theme).title_bottom(Line::from(Span::styled(
        " ↑↓ move · enter go · esc close ",
        Style::default().fg(theme.dim),
    )));
    if !s.results.is_empty() {
        block = block.title_bottom(
            Line::from(Span::styled(
                format!(" {}/{} ", s.cursor + 1, s.results.len()),
                Style::default().fg(theme.dim),
            ))
            .right_aligned(),
        );
    }
    let inner = block.inner(area);
    f.render_widget(block, area);
    let [input, rule, list] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .areas(inner);
    f.render_widget(
        Paragraph::new(Span::styled(
            "─".repeat(rule.width as usize),
            Style::default().fg(theme.surface),
        )),
        rule,
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" › ", Style::default().fg(theme.accent).bold()),
            Span::raw(s.input.clone()),
            Span::styled("█", Style::default().fg(theme.accent)),
            Span::styled(
                if s.pending {
                    format!("  {}", spinner(app))
                } else {
                    String::new()
                },
                Style::default().fg(theme.dim),
            ),
        ])),
        input,
    );

    let height = list.height as usize;
    let s = app.search.as_mut().expect("search open");
    if s.cursor < s.scroll {
        s.scroll = s.cursor;
    } else if s.cursor >= s.scroll + height.max(1) {
        s.scroll = s.cursor + 1 - height.max(1);
    }
    let s = app.search.as_ref().expect("search open");

    if s.results.is_empty() {
        let msg = if let Some(e) = &s.error {
            Span::styled(format!(" {e}"), Style::default().fg(theme.error))
        } else if s.input.trim().is_empty() {
            Span::styled(" Type to search the server (movies, shows, episodes, music, people…)", Style::default().fg(theme.dim))
        } else if s.pending {
            Span::styled(" Searching…", Style::default().fg(theme.dim))
        } else {
            Span::styled(" No results", Style::default().fg(theme.dim))
        };
        f.render_widget(Paragraph::new(msg), list);
        return;
    }

    let width = list.width as usize;
    let lines: Vec<Line> = s
        .results
        .iter()
        .enumerate()
        .skip(s.scroll)
        .take(height)
        .map(|(i, item)| {
            let kind = item.kind.clone().unwrap_or_default();
            let icon = kind_icon(item.kind());
            let lib = item.library_section_title.clone().unwrap_or_default();
            let right = if lib.is_empty() {
                kind
            } else {
                format!("{kind} · {lib}")
            };
            let mut title = item.full_title();
            if let Some(y) = item
                .year
                .filter(|_| matches!(item.kind(), Kind::Movie | Kind::Show))
            {
                title.push_str(&format!(" ({y})"));
            }
            let title = truncate(&title, width.saturating_sub(right.width() + 6));
            let pad = width.saturating_sub(title.width() + right.width() + 5);
            let line = Line::from(vec![
                Span::raw(" "),
                Span::styled(icon, Style::default().fg(theme.accent)),
                Span::raw(" "),
                Span::raw(title),
                Span::raw(" ".repeat(pad)),
                Span::styled(right, Style::default().fg(theme.dim)),
                Span::raw(" "),
            ]);
            if i == s.cursor {
                highlight(line, width, selected_style(theme))
            } else {
                line
            }
        })
        .collect();
    f.render_widget(Paragraph::new(lines), list);
}

// ---- modals ----------------------------------------------------------------

fn list_lines(rows: &[String], cursor: usize, theme: &Theme, width: usize) -> Vec<Line<'static>> {
    rows.iter()
        .enumerate()
        .map(|(i, r)| {
            let text = truncate(r, width.saturating_sub(2));
            let pad = width.saturating_sub(text.width() + 1);
            if i == cursor {
                Line::from(Span::styled(
                    format!(" {text}{}", " ".repeat(pad)),
                    Style::default().bg(theme.accent).fg(Color::Black),
                ))
            } else {
                Line::from(format!(" {text}"))
            }
        })
        .collect()
}

pub const HELP: &[(&str, &str)] = &[
    ("", "Navigation"),
    ("j/k ↑/↓", "move"),
    ("l → / h ← ⌫", "open / back"),
    ("g/G", "top / bottom"),
    ("^d/^u PgDn/PgUp", "half page / page"),
    ("enter", "play (resume) or open"),
    ("", "Playback"),
    ("p", "play from start"),
    ("a", "play all / from here (next-up for shows)"),
    ("z", "shuffle"),
    ("o", "play with options (version, tracks, quality)"),
    ("P", "pause / resume the player"),
    ("< / >", "previous / next in the queue"),
    ("- / =", "seek back / forward 10 s"),
    ("X", "stop the player"),
    ("", "Library"),
    ("w / u", "mark watched / unwatched"),
    ("*", "rate 0–10"),
    ("space", "add to / remove from playlists"),
    ("x", "delete playlist · remove entry · stop stream"),
    ("J / K", "move playlist entry down / up"),
    ("+", "add to / remove from watchlist"),
    ("C", "add to / remove from collections"),
    ("e", "edit details (renames a playlist)"),
    ("v", "browse library by genre, year, collection…"),
    ("S", "scan library for new files"),
    ("m  .", "actions menu (more options)"),
    ("", "Finding things"),
    ("f", "search the server"),
    ("/", "filter this column"),
    ("s", "sort"),
    ("H", "hide / show watched"),
    ("", "Other"),
    ("i", "toggle details panel"),
    ("[ / ]", "scroll details"),
    ("O", "open in Plex Web"),
    ("y", "copy stream URL"),
    ("D", "download for offline"),
    ("r / R", "refresh view / everything"),
    ("U", "switch Plex Home user"),
    ("L", "sign out"),
    ("q  ^c", "quit"),
];

fn draw_modal(f: &mut Frame, app: &App, theme: &Theme) {
    let screen = f.area();
    let Some(modal) = &app.modal else { return };
    match modal {
        Modal::Help { scroll } => {
            let area = centered(screen, 72, HELP.len() as u16 + 2);
            f.render_widget(Clear, area);
            let lines: Vec<Line> = HELP
                .iter()
                .map(|(k, d)| {
                    if k.is_empty() {
                        Line::from(Span::styled(
                            format!(" {d}"),
                            Style::default().fg(theme.accent).bold(),
                        ))
                    } else {
                        Line::from(vec![
                            Span::styled(format!("   {k:<18}"), Style::default().fg(theme.accent)),
                            Span::raw(d.to_string()),
                        ])
                    }
                })
                .collect();
            f.render_widget(
                Paragraph::new(lines)
                    .block(modal_block(" Keys ", theme))
                    .scroll((*scroll, 0)),
                area,
            );
        }
        Modal::Sort { cursor } => {
            let col = app.columns.last();
            let rows: Vec<String> = sort_rows()
                .iter()
                .map(|k| {
                    let current = col.is_some_and(|c| c.sort == *k);
                    let dir = col
                        .map(|c| if c.sort_desc { " ↓" } else { " ↑" })
                        .unwrap_or("");
                    format!(
                        "{} {}{}",
                        if current { "●" } else { " " },
                        k.label(),
                        if current { dir } else { "" }
                    )
                })
                .collect();
            let area = centered(screen, 34, rows.len() as u16 + 2);
            f.render_widget(Clear, area);
            let lines = list_lines(&rows, *cursor, theme, area.width as usize - 2);
            f.render_widget(
                Paragraph::new(lines).block(modal_block(" Sort ", theme)),
                area,
            );
        }
        Modal::Browse {
            section_title,
            cursor,
            ..
        } => {
            let rows: Vec<String> = BROWSE_VIEWS.iter().map(|(l, _)| l.to_string()).collect();
            let area = centered(screen, 36, rows.len() as u16 + 2);
            f.render_widget(Clear, area);
            let lines = list_lines(&rows, *cursor, theme, area.width as usize - 2);
            f.render_widget(
                Paragraph::new(lines)
                    .block(modal_block(format!(" Browse {section_title} "), theme)),
                area,
            );
        }
        Modal::Menu(m) => {
            let width = 46u16;
            let area = centered(screen, width, m.rows.len() as u16 + 2);
            f.render_widget(Clear, area);
            let inner_w = area.width as usize - 2;
            let lines: Vec<Line> = m
                .rows
                .iter()
                .enumerate()
                .map(|(i, (_, label, key))| {
                    let pad = inner_w.saturating_sub(label.width() + key.width() + 2);
                    let mut line = Line::from(vec![
                        Span::raw(format!(" {label}")),
                        Span::raw(" ".repeat(pad)),
                        Span::styled(format!("{key} "), Style::default().fg(theme.dim)),
                    ]);
                    if i == m.cursor {
                        let st = Style::default().bg(theme.accent).fg(Color::Black);
                        for s in line.spans.iter_mut() {
                            s.style = s.style.patch(st);
                        }
                    }
                    line
                })
                .collect();
            let visible = area.height.saturating_sub(2) as usize;
            let skip = m.cursor.saturating_sub(visible.saturating_sub(1));
            let lines: Vec<Line> = lines.into_iter().skip(skip).collect();
            f.render_widget(
                Paragraph::new(lines)
                    .block(modal_block(format!(" {} ", truncate(&m.title, 36)), theme)),
                area,
            );
        }
        Modal::Input(i) => {
            let area = centered(screen, 52, 5);
            f.render_widget(Clear, area);
            let lines = vec![
                Line::from(Span::styled(
                    format!(" {}", i.hint),
                    Style::default().fg(theme.dim),
                )),
                Line::from(vec![
                    Span::raw(if matches!(i.purpose, InputPurpose::UserPin { .. }) {
                        format!(" {}", "•".repeat(i.value.chars().count()))
                    } else {
                        // Keep the end (and the cursor) visible for long values.
                        let w = area.width.saturating_sub(5) as usize;
                        let v: Vec<char> = i.value.chars().collect();
                        let skip = v.len().saturating_sub(w);
                        format!(" {}", v[skip..].iter().collect::<String>())
                    }),
                    Span::styled("█", Style::default().fg(theme.accent)),
                ]),
                Line::from(Span::styled(
                    " enter save · esc cancel",
                    Style::default().fg(theme.dim),
                )),
            ];
            f.render_widget(
                Paragraph::new(lines)
                    .block(modal_block(format!(" {} ", truncate(&i.title, 40)), theme)),
                area,
            );
        }
        Modal::Confirm(c) => {
            let area = centered(screen, (c.message.width() as u16 + 6).clamp(30, 70), 5);
            f.render_widget(Clear, area);
            let lines = vec![
                Line::raw(format!(" {}", c.message)),
                Line::raw(""),
                Line::from(vec![
                    Span::styled(" y", Style::default().fg(theme.accent).bold()),
                    Span::raw(" yes   "),
                    Span::styled("n", Style::default().fg(theme.accent).bold()),
                    Span::raw(" no"),
                ]),
            ];
            f.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .block(modal_block(" Confirm ", theme)),
                area,
            );
        }
        Modal::Playlists(p) => {
            let height = (p.rows.len() as u16 + 4).clamp(6, 22);
            let area = centered(screen, 50, height);
            f.render_widget(Clear, area);
            let block = modal_block(
                format!(" Playlists · {} ", truncate(p.item.title(), 28)),
                theme,
            )
            .title_bottom(Line::from(Span::styled(
                " space toggle · n new · esc close ",
                Style::default().fg(theme.dim),
            )));
            let inner_w = area.width as usize - 2;
            let lines: Vec<Line> = if p.loading {
                vec![Line::from(Span::styled(
                    format!(" {} Checking playlists…", spinner(app)),
                    Style::default().fg(theme.dim),
                ))]
            } else if let Some(e) = &p.error {
                vec![
                    Line::from(Span::styled(
                        format!(" {e}"),
                        Style::default().fg(theme.error),
                    )),
                    Line::from(Span::styled(
                        " Membership unknown; close and try again.",
                        Style::default().fg(theme.dim),
                    )),
                ]
            } else {
                let mut rows: Vec<String> = p
                    .rows
                    .iter()
                    .map(|r| {
                        let mark = if r.busy {
                            spinner(app).to_string()
                        } else if r.entry_id.is_some() {
                            "✓".into()
                        } else {
                            " ".into()
                        };
                        format!("[{mark}] {}", r.playlist.title())
                    })
                    .collect();
                rows.push("  + New playlist…".into());
                let visible = area.height.saturating_sub(2) as usize;
                let skip = p.cursor.saturating_sub(visible.saturating_sub(1));
                list_lines(&rows, p.cursor, theme, inner_w)
                    .into_iter()
                    .skip(skip)
                    .collect()
            };
            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::Collections(c) => {
            let height = (c.rows.len() as u16 + 4).clamp(6, 22);
            let area = centered(screen, 50, height);
            f.render_widget(Clear, area);
            let block = modal_block(
                format!(" Collections · {} ", truncate(c.item.title(), 26)),
                theme,
            )
            .title_bottom(Line::from(Span::styled(
                " space toggle · n new · esc close ",
                Style::default().fg(theme.dim),
            )));
            let inner_w = area.width as usize - 2;
            let lines: Vec<Line> = if c.loading {
                vec![Line::from(Span::styled(
                    format!(" {} Loading collections…", spinner(app)),
                    Style::default().fg(theme.dim),
                ))]
            } else if let Some(e) = &c.error {
                vec![Line::from(Span::styled(
                    format!(" {e}"),
                    Style::default().fg(theme.error),
                ))]
            } else {
                let mut rows: Vec<String> = c
                    .rows
                    .iter()
                    .map(|r| {
                        let mark = if r.busy {
                            spinner(app).to_string()
                        } else if r.member {
                            "✓".into()
                        } else {
                            " ".into()
                        };
                        format!("[{mark}] {}", r.collection.title())
                    })
                    .collect();
                rows.push("  + New collection…".into());
                let visible = area.height.saturating_sub(2) as usize;
                let skip = c.cursor.saturating_sub(visible.saturating_sub(1));
                list_lines(&rows, c.cursor, theme, inner_w)
                    .into_iter()
                    .skip(skip)
                    .collect()
            };
            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::Edit(m) => {
            let area = centered(screen, 70, EDIT_FIELDS.len() as u16 + 2);
            f.render_widget(Clear, area);
            let inner_w = area.width as usize - 2;
            let rows: Vec<String> = EDIT_FIELDS
                .iter()
                .map(|(field, label)| {
                    let lock_name = if *field == "labels" { "label" } else { field };
                    let lock = if m.item.is_field_locked(lock_name) {
                        "🔒"
                    } else {
                        "  "
                    };
                    let value = field_value(&m.item, field).replace('\n', " ");
                    format!("{lock} {label:<15} {value}")
                })
                .collect();
            let block = modal_block(format!(" Edit · {} ", truncate(m.item.title(), 40)), theme)
                .title_bottom(Line::from(Span::styled(
                    " enter edit · l lock/unlock · esc close ",
                    Style::default().fg(theme.dim),
                )));
            f.render_widget(
                Paragraph::new(list_lines(&rows, m.cursor, theme, inner_w)).block(block),
                area,
            );
        }
        Modal::Pick(p) => {
            let height = (p.rows.len() as u16 + 2).clamp(5, 22);
            let area = centered(screen, 64, height);
            f.render_widget(Clear, area);
            let block = modal_block(format!(" {} ", truncate(&p.title, 50)), theme).title_bottom(
                Line::from(Span::styled(p.hint, Style::default().fg(theme.dim))),
            );
            let inner_w = area.width as usize - 2;
            let lines: Vec<Line> = if p.loading {
                vec![Line::from(Span::styled(
                    format!(" {} Loading…", spinner(app)),
                    Style::default().fg(theme.dim),
                ))]
            } else if let Some(e) = &p.error {
                vec![Line::from(Span::styled(
                    format!(" {e}"),
                    Style::default().fg(theme.error),
                ))]
            } else {
                let rows: Vec<String> = p.rows.iter().map(|(_, l)| l.clone()).collect();
                let visible = area.height.saturating_sub(2) as usize;
                let skip = p.cursor.saturating_sub(visible.saturating_sub(1));
                list_lines(&rows, p.cursor, theme, inner_w)
                    .into_iter()
                    .skip(skip)
                    .collect()
            };
            f.render_widget(Paragraph::new(lines).block(block), area);
        }
        Modal::PlayOptions(m) => {
            let height = (m.rows.len() as u16 + 2).clamp(5, screen.height.saturating_sub(2));
            let area = centered(screen, 60, height);
            f.render_widget(Clear, area);
            let title = m.item.as_ref().map(|i| i.full_title()).unwrap_or_default();
            let block = modal_block(format!(" Play · {} ", truncate(&title, 44)), theme);
            let inner_w = area.width as usize - 2;
            let mut lines: Vec<Line> = Vec::new();
            if m.item.is_none() {
                lines.push(Line::from(Span::styled(
                    format!(" {} Loading streams…", spinner(app)),
                    Style::default().fg(theme.dim),
                )));
            }
            for (i, (row, label)) in m.rows.iter().enumerate() {
                let line = match row {
                    OptRow::Header(h) => Line::from(Span::styled(
                        format!(" {h}"),
                        Style::default().fg(theme.dim).bold(),
                    )),
                    OptRow::Play => Line::from(Span::styled(
                        format!("   {label}"),
                        Style::default().fg(theme.accent).bold(),
                    )),
                    _ => Line::raw(format!(
                        "   ({}) {}",
                        if m.is_checked(row) { "•" } else { " " },
                        truncate(label, inner_w.saturating_sub(8))
                    )),
                };
                let line = if i == m.cursor {
                    let st = Style::default().bg(theme.accent).fg(Color::Black);
                    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                    let pad = inner_w.saturating_sub(text.width());
                    Line::from(Span::styled(format!("{text}{}", " ".repeat(pad)), st))
                } else {
                    line
                };
                lines.push(line);
            }
            let visible = area.height.saturating_sub(2) as usize;
            let skip = m.cursor.saturating_sub(visible.saturating_sub(1));
            let lines: Vec<Line> = lines.into_iter().skip(skip).collect();
            f.render_widget(Paragraph::new(lines).block(block), area);
        }
    }
}

#[cfg(test)]
mod tests {
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
