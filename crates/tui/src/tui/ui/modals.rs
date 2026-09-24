use super::*;

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

pub(super) fn draw_modal(f: &mut Frame, app: &App, theme: &Theme) {
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
