use super::*;

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

pub(super) fn draw_search(f: &mut Frame, app: &mut App, theme: &Theme) {
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
