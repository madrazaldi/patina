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
        Kind::Person => "☺",
        Kind::Tag => "#",
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
        " ↑↓ move · tab next type · enter go · esc close ",
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
    let s = app.search.as_ref().expect("search open");
    if s.results.is_empty() {
        let msg = if let Some(e) = &s.error {
            Span::styled(format!(" {e}"), Style::default().fg(theme.error))
        } else if s.input.trim().chars().count() < 2 {
            Span::styled(" Type to search the server (movies, shows, episodes, music, people, genres…)", Style::default().fg(theme.dim))
        } else if s.pending {
            Span::styled(" Searching…", Style::default().fg(theme.dim))
        } else {
            Span::styled(" No results", Style::default().fg(theme.dim))
        };
        f.render_widget(Paragraph::new(msg), list);
        return;
    }

    // Results under a heading per type; the cursor only lands on results.
    let width = list.width as usize;
    let mut rows: Vec<Line> = Vec::new();
    let mut cursor_row = 0;
    for (i, item) in s.results.iter().enumerate() {
        let g = s.group_of[i];
        if i == 0 || s.group_of[i - 1] != g {
            let group = &s.groups[g];
            let count = group.child_count.map(|n| format!(" {n}")).unwrap_or_default();
            let title = group.title().to_string();
            let rule = width.saturating_sub(title.width() + count.width() + 3);
            rows.push(Line::from(vec![
                Span::styled(format!(" {title}"), Style::default().fg(theme.accent).bold()),
                Span::styled(count, Style::default().fg(theme.dim)),
                Span::raw(" "),
                Span::styled("─".repeat(rule), Style::default().fg(theme.surface)),
            ]));
        }
        if i == s.cursor {
            cursor_row = rows.len();
        }
        rows.push(result_line(item, i == s.cursor, width, theme));
    }
    let s = app.search.as_mut().expect("search open");
    // Keep the cursor's heading in view when moving up to a group's first result.
    let top = if s.cursor == 0 || s.group_of.get(s.cursor - 1) != s.group_of.get(s.cursor) { cursor_row.saturating_sub(1) } else { cursor_row };
    if top < s.scroll {
        s.scroll = top;
    } else if cursor_row >= s.scroll + height.max(1) {
        s.scroll = cursor_row + 1 - height.max(1);
    }
    let lines: Vec<Line> = rows.into_iter().skip(s.scroll).take(height).collect();
    f.render_widget(Paragraph::new(lines), list);
}

fn result_line(item: &Item, selected: bool, width: usize, theme: &Theme) -> Line<'static> {
    let icon = kind_icon(item.kind());
    let right = match item.kind() {
        Kind::Person | Kind::Tag => item.summary.clone().unwrap_or_default(),
        _ => item.library_section_title.clone().unwrap_or_default(),
    };
    let mut title = item.full_title();
    if let Some(y) = item.year.filter(|_| matches!(item.kind(), Kind::Movie | Kind::Show)) {
        title.push_str(&format!(" ({y})"));
    }
    let title = truncate(&title, width.saturating_sub(right.width() + 8));
    let pad = width.saturating_sub(title.width() + right.width() + 7);
    let line = Line::from(vec![
        Span::raw("   "),
        Span::styled(icon, Style::default().fg(theme.accent)),
        Span::raw(" "),
        Span::raw(title),
        Span::raw(" ".repeat(pad)),
        Span::styled(right, Style::default().fg(theme.dim)),
        Span::raw(" "),
    ]);
    if selected { highlight(line, width, selected_style(theme)) } else { line }
}
