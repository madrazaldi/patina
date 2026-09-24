//! The now-playing bar with its Up next prompt, and the queue and lyrics panels above it.

use super::*;
use super::inspector::progress_bar;
use crate::tui::app::{LYRICS_HOLD, active_line, playing_lines, up_next};

/// Rows the now-playing bar needs: two, plus one for the Up next prompt.
pub(super) fn bar_height(app: &App) -> u16 {
    match &app.now_playing {
        None => 0,
        Some(np) if prompt(app, np).is_some() => 3,
        Some(_) => 2,
    }
}

/// What plays next, while the current item is ending (as the GUI prompts).
fn prompt(app: &App, np: &NowPlaying) -> Option<Metadata> {
    (np.ending && !np.stop_after).then(|| up_next(np, app.queue.as_ref())).flatten()
}

pub(super) fn draw_now_playing(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
    let Some(np) = &app.now_playing else { return };
    let dim = Style::default().fg(theme.dim);
    let accent = Style::default().fg(theme.accent);
    let width = area.width as usize;
    let mut lines = Vec::new();
    if let Some(next) = prompt(app, np) {
        let (t, s) = playing_lines(&next);
        let what = if s.is_empty() { t } else { format!("{t} · {s}") };
        let keys = "> play now  E stop after this ";
        let what = format!(" {}", truncate(&what, width.saturating_sub(keys.width() + 12)));
        let pad = width.saturating_sub(what.width() + keys.width() + 9);
        lines.push(Line::from(vec![
            Span::styled(" Up next ", Style::default().bg(theme.accent).fg(Color::Black).bold()),
            Span::raw(what),
            Span::raw(" ".repeat(pad)),
            Span::styled(keys, dim),
        ]));
    }
    let (title, sub) = playing_lines(&np.item);
    let pos = app.position_ms();
    let status = if np.live {
        "Live"
    } else if np.local {
        "Offline"
    } else if np.transcoding {
        "Transcode"
    } else {
        "Direct play"
    };
    let mut right = Vec::new();
    if np.stop_after {
        right.push(Span::styled("Stops after this · E undo  ", Style::default().fg(theme.star)));
    }
    right.push(Span::styled(format!("{status}  "), dim));
    if np.queue_len > 1 {
        right.push(Span::styled(format!("{} of {}  ", np.queue_pos + 1, np.queue_len), dim));
    }
    let time = if np.live { format::clock_ms(pos) } else { format!("{} / {}", format::clock_ms(pos), format::clock_ms(np.duration_ms)) };
    right.push(Span::raw(format!("{time} ")));
    let right_w: usize = right.iter().map(|s| s.content.width()).sum();
    let icon = if np.paused { "‖" } else { "▶" };
    let what = if sub.is_empty() { title } else { format!("{title} · {sub}") };
    let left = format!(" {icon} {}", truncate(&what, width.saturating_sub(right_w + 4)));
    let pad = width.saturating_sub(left.width() + right_w);
    let mut info = vec![Span::styled(left, accent.bold()), Span::raw(" ".repeat(pad))];
    info.extend(right);
    lines.push(Line::from(info));
    let mut keys = String::from(" P pause  < > prev/next  - = seek  X stop  Q queue");
    if np.item.kind() == "track" {
        keys.push_str("  Y lyrics");
    }
    let bar_w = width.saturating_sub(keys.width() + 2);
    let frac = if np.duration_ms > 0 { pos as f64 / np.duration_ms as f64 } else { 0.0 };
    lines.push(Line::from(vec![Span::styled(format!(" {}", if np.live { "─".repeat(bar_w) } else { progress_bar(frac, bar_w) }), accent), Span::styled(keys, dim)]));
    f.render_widget(Paragraph::new(lines), area);
}

/// A panel anchored above the now-playing bar, at the right like the GUI's.
fn panel_area(screen: Rect, bar: u16, height: u16) -> Rect {
    let width = 72.min(screen.width.saturating_sub(2));
    let bottom = screen.height.saturating_sub(1 + bar);
    let height = height.min(bottom.saturating_sub(2)).max(3);
    Rect { x: screen.width.saturating_sub(width + 1), y: bottom.saturating_sub(height), width, height }
}

pub(super) fn draw_queue(f: &mut Frame, app: &App, sel: Option<u64>, theme: &Theme) {
    let Some(q) = &app.queue else { return };
    let current = q.items.iter().position(|e| Some(e.uid) == q.current);
    let upcoming = current.map_or(q.items.len(), |c| q.items.len() - c - 1);
    let area = panel_area(f.area(), bar_height(app), q.items.len() as u16 + 3);
    f.render_widget(Clear, area);
    let dim = Style::default().fg(theme.dim);
    let flags = [(q.shuffled, "shuffled"), (q.stop_after, "stops after this")].iter().filter(|f| f.0).map(|f| f.1).collect::<Vec<_>>().join(" · ");
    let head = format!(" Up next · {} {}", if upcoming > 0 { format!("{upcoming} item{}", if upcoming == 1 { "" } else { "s" }) } else { "Nothing".into() }, if flags.is_empty() { String::new() } else { format!("· {flags} ") });
    let block = modal_block(Line::from(Span::styled(head, Style::default().fg(theme.accent).bold())), theme)
        .title_bottom(Line::from(Span::styled(" enter play · x remove · J/K move · z shuffle · E stop after · esc ", dim)));
    let width = area.width as usize - 2;
    let at = sel.and_then(|u| q.items.iter().position(|e| e.uid == u)).or(current).unwrap_or(0);
    let visible = area.height.saturating_sub(2) as usize;
    // Keep the selection in view, with one earlier entry above it when possible.
    let skip = at.saturating_sub(1).min(q.items.len().saturating_sub(visible)).max(at.saturating_sub(visible.saturating_sub(1)));
    let lines: Vec<Line> = q
        .items
        .iter()
        .enumerate()
        .skip(skip)
        .take(visible)
        .map(|(i, e)| {
            let (t, s) = playing_lines(&e.item);
            let playing = Some(i) == current;
            let mark = if playing { "♪" } else { " " };
            let dur = e.item.duration.map(format::clock_ms).unwrap_or_default();
            let text = if s.is_empty() { t } else { format!("{t} · {s}") };
            let body = truncate(&format!("{mark} {text}"), width.saturating_sub(dur.width() + 3));
            let pad = width.saturating_sub(body.width() + dur.width() + 2);
            let line = Line::from(format!(" {body}{}{dur} ", " ".repeat(pad)));
            if i == at {
                highlight(line, width, selected_style(theme))
            } else if playing {
                line.style(Style::default().fg(theme.accent).bold())
            } else if current.is_some_and(|c| i < c) {
                line.style(dim)
            } else {
                line
            }
        })
        .collect();
    let lines = if lines.is_empty() { vec![Line::from(Span::styled(" The queue is empty.", dim))] } else { lines };
    f.render_widget(Paragraph::new(lines).block(block), area);
}

pub(super) fn draw_lyrics(f: &mut Frame, app: &App, manual: Option<(usize, std::time::Instant)>, theme: &Theme) {
    let Some(np) = &app.now_playing else { return };
    let area = panel_area(f.area(), bar_height(app), 24);
    f.render_widget(Clear, area);
    let dim = Style::default().fg(theme.dim);
    let block = modal_block(Line::from(vec![Span::styled(" Lyrics ", Style::default().fg(theme.accent).bold()), Span::styled(format!("· {} ", truncate(np.item.title(), 40)), dim)]), theme)
        .title_bottom(Line::from(Span::styled(" j/k pick a line · enter seek to it · esc ", dim)));
    let width = area.width as usize - 2;
    let lines: Vec<Line> = match app.playing_lyrics() {
        None => vec![Line::from(Span::styled(format!(" {} Loading…", spinner(app)), dim))],
        Some(Err(e)) => vec![Line::from(Span::styled(format!(" {e}"), Style::default().fg(theme.error)))],
        Some(Ok(None)) => vec![Line::from(Span::styled(" No lyrics for this track", dim))],
        Some(Ok(Some(l))) => {
            let active = active_line(l, app.position_ms());
            let picked = manual.filter(|(_, t)| t.elapsed() < LYRICS_HOLD).map(|(i, _)| i);
            let focus = picked.or(active).unwrap_or(0);
            let visible = area.height.saturating_sub(2) as usize;
            // The focused line sits in the middle, as the GUI scrolls it.
            let skip = focus.saturating_sub(visible / 2).min(l.lines.len().saturating_sub(visible));
            l.lines
                .iter()
                .enumerate()
                .skip(skip)
                .take(visible)
                .map(|(i, line)| {
                    let text = format!(" {}", truncate(&line.text, width.saturating_sub(2)));
                    if Some(i) == picked {
                        highlight(Line::from(text), width, selected_style(theme))
                    } else if Some(i) == active {
                        Line::from(Span::styled(text, Style::default().fg(theme.accent).bold()))
                    } else if l.synced {
                        Line::from(Span::styled(text, dim))
                    } else {
                        Line::from(text)
                    }
                })
                .collect()
        }
    };
    f.render_widget(Paragraph::new(lines).block(block), area);
}
