use super::*;

/// The glyph of a root entry, by what it opens.
fn nav_icon(source: Option<&Source>) -> &'static str {
    match source {
        Some(Source::Home) => "⌂",
        Some(Source::Search) => "⌕",
        Some(Source::Watchlist) => "☆",
        Some(Source::Path(p)) if p == PLAYLISTS => "≡",
        Some(Source::Sessions) => "◉",
        Some(Source::History(_)) => "↺",
        Some(Source::Downloads) => "↓",
        Some(Source::Group(Group::LiveTv)) => "▭",
        Some(Source::Group(Group::Activity)) => "◉",
        Some(Source::Group(Group::Server)) => "▣",
        Some(Source::Settings) => "⚙",
        Some(Source::Tasks) => "⟳",
        _ => "•",
    }
}

pub(super) fn tone(t: Tone, theme: &Theme) -> Style {
    match t {
        Tone::Normal => Style::default(),
        Tone::Accent => Style::default().fg(theme.accent),
        Tone::Dim => Style::default().fg(theme.dim),
        Tone::Error => Style::default().fg(theme.error),
        Tone::Good => Style::default().fg(theme.success),
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

/// Continue Watching and On Deck style hubs.
pub(super) fn hub_continues(item: &Item) -> bool {
    let id = item.key.as_deref().unwrap_or("").to_ascii_lowercase();
    ["continue", "ondeck", "inprogress"].iter().any(|s| id.contains(s))
}

/// Columns whose items appear outside their natural parent need full titles.
fn context_free(col: &Column) -> bool {
    match &col.source {
        Source::Root | Source::Folder { .. } => false,
        Source::Library(v) => v.leaves(),
        Source::Path(p) => {
            p.starts_with("/playlists/")
                || p.ends_with("/related")
                || p.contains("/onDeck")
                || p.contains("/recentlyAdded")
                || p.contains("/recentlyViewed")
                || p.starts_with("/library/collections/")
        }
        _ => true,
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
        Kind::Nav => Span::styled(nav_icon(item.link_source()), accent),
        Kind::Section => Span::styled(section_icon(item.kind.as_deref()), accent),
        Kind::Playlist => Span::styled("≡", dim),
        Kind::Collection => Span::styled("▦", dim),
        Kind::Folder => Span::styled("▸", dim),
        Kind::Artist => Span::styled("♪", dim),
        Kind::Album => Span::styled("◎", dim),
        Kind::PhotoAlbum => Span::styled("▢", dim),
        Kind::Remote => Span::styled("☆", dim),
        Kind::Hub => Span::styled(if hub_continues(item) { "▶" } else { "▦" }, accent),
        Kind::Person => Span::styled("☺", dim),
        Kind::Tag => Span::styled("#", dim),
        Kind::Row => {
            let r = item.row.as_deref();
            Span::styled(r.map_or(" ", |r| r.glyph), tone(r.map_or(Tone::Dim, |r| r.tone), ctx.theme))
        }
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
    if let Some(r) = &item.row {
        return (truncate(&r.note, 32), dim);
    }
    if ctx.history {
        let when = item.viewed_at.map(format::relative).unwrap_or_default();
        return match item.extra.get("accountName").and_then(|v| v.as_str()) {
            Some(who) => (format!("{who} · {when}"), dim),
            None => (when, dim),
        };
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
        Kind::Hub => (item.child_count.map(|n| n.to_string()).unwrap_or_default(), dim),
        Kind::Person | Kind::Tag => (item.summary.clone().unwrap_or_default(), dim),
        Kind::Movie | Kind::Episode | Kind::Clip if item.in_progress() && !item.is_watched() => (
            item.progress()
                .map(|p| format!("{:.0}%", p * 100.0))
                .unwrap_or_default(),
            accent,
        ),
        Kind::Movie | Kind::Show | Kind::Remote => {
            (item.year.map(|y| y.to_string()).unwrap_or_default(), dim)
        }
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
    let heading = item.row.as_ref().is_some_and(|r| matches!(r.act, Act::Heading));
    let title_style = if heading {
        Style::default().fg(ctx.theme.accent).bold()
    } else if watched_dim || item.kind() == Kind::Remote {
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

pub(super) fn draw_columns(f: &mut Frame, app: &mut App, theme: &Theme, area: Rect) {
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
        if let Some(v) = col.library() {
            bottom.push(Span::styled(format!(" {} ", v.describe()), Style::default().fg(if v.filters.is_empty() { theme.dim } else { theme.accent })));
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
            history: matches!(col.source, Source::History(_)),
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
            } else if col.source == Source::Downloads {
                Line::from(Span::styled(" Nothing downloaded yet · D on a movie, episode, season, show, album or artist", Style::default().fg(theme.dim)))
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
