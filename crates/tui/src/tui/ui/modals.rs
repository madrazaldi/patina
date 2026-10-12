use super::*;
use super::inspector::progress_bar;

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
    ("n / N", "play next / add to the end of the queue"),
    ("Q", "play queue: jump, remove, J/K reorder, z shuffle"),
    ("E", "stop after this (when Up next shows: > plays it now)"),
    ("Y", "lyrics of the playing track (enter seeks to a line)"),
    ("V", "player panel: chapters, diagnostics; after a failure: retry"),
    ("9 / 0", "volume down / up 5%"),
    ("M", "mute / unmute"),
    ("{ / }", "playback speed down / up 0.25x"),
    ("Z", "sleep timer: off, 15-90 min, end of this item"),
    ("r", "repeat off / all / one (in the queue or player panel)"),
    ("m → radio", "start radio from an artist, album or track"),
    ("", "Library"),
    ("w / u", "mark watched / unwatched"),
    ("I  ^z", "undo the last watch-state change, playlist removal or Continue Watching removal"),
    ("*", "rate 0–10"),
    ("space", "add to / remove from playlists"),
    ("x", "delete playlist or collection · remove entry · stop stream · delete history entry"),
    ("J / K", "move a playlist (or custom-sorted collection) entry down / up"),
    ("+", "add to / remove from watchlist"),
    ("C", "add to / remove from collections"),
    ("T", "audio & subtitles for a whole show or season (r there: remember)"),
    ("c", "related & similar, cast & crew, genres and tags"),
    ("e", "edit details, genres, labels (renames a playlist); menu: poster, background, match"),
    ("S", "scan library for new files"),
    ("b / B", "go to the season or album / the show or artist"),
    ("m  .", "actions menu (more options)"),
    ("", "Selecting several items"),
    ("tab", "mark / unmark the item and move on"),
    ("^a", "mark everything shown (loads a whole library first)"),
    ("M", "with items marked: actions for them (otherwise mute); esc clears the marks"),
    ("n N w u D", "with items marked: queue next / at the end · mark watched / unwatched · download"),
    ("space  C", "with items marked: add them to a playlist / collection (or a new one)"),
    ("x", "with items marked in a playlist or collection: remove them"),
    ("", "Downloads, Live TV, Activity, Server, Settings"),
    ("enter", "a row's main action: play, pause/resume download, retry, watch, record, run, change"),
    ("x", "remove a download · delete a rule · cancel a task · stop a transcode"),
    ("a", "play a downloaded show, album or all movies"),
    ("l", "a channel's later programs · a library's settings"),
    ("/", "search server settings (or any list)"),
    ("", "Finding things"),
    ("f", "search the server, grouped by type (tab: next type; ^a: all servers, enter: choose a copy)"),
    ("r", "on Pick something to watch: reroll the candidates"),
    ("/", "filter this column"),
    ("s", "sort (the server's sorts in a library)"),
    ("F", "library filters (unwatched, genre, HDR…)"),
    ("A", "jump to a letter (library sorted by title)"),
    ("t", "switch type: shows / seasons / episodes…"),
    ("v", "library views: recommended, collections, folders…"),
    ("W", "saved views of this library: apply, save the current one, x deletes (shared with the desktop app)"),
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
            let area = centered(screen, 80, HELP.len() as u16 + 2);
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
        Modal::Browse { section_title, rows, cursor, .. } => {
            let rows: Vec<String> = rows.iter().map(|(l, _)| l.clone()).collect();
            let area = centered(screen, 36, rows.len() as u16 + 2);
            f.render_widget(Clear, area);
            let visible = area.height.saturating_sub(2) as usize;
            let skip = cursor.saturating_sub(visible.saturating_sub(1));
            let lines: Vec<Line> = list_lines(&rows, *cursor, theme, area.width as usize - 2).into_iter().skip(skip).collect();
            f.render_widget(Paragraph::new(lines).block(modal_block(format!(" {section_title} "), theme)), area);
        }
        Modal::Copies(p) => {
            let rows: Vec<String> = p
                .group
                .copies
                .iter()
                .map(|c| {
                    let quality: Vec<String> = c.versions.iter().map(|v| v.label.clone()).collect();
                    let q = if quality.is_empty() { String::new() } else { format!("  {}", quality.join(", ")) };
                    format!("{}{}{q}", c.server_name, if c.current { " (this server)" } else { "" })
                })
                .collect();
            let area = centered(screen, 64, rows.len() as u16 + 4);
            f.render_widget(Clear, area);
            let mut lines = list_lines(&rows, p.cursor, theme, area.width as usize - 2);
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(" enter play · p from start · l browse · esc cancel", Style::default().fg(theme.dim))));
            f.render_widget(Paragraph::new(lines).block(modal_block(format!(" {} ", truncate(&p.group.title, 50)), theme)), area);
        }
        Modal::Filter(p) => draw_filters(f, app, p, theme),
        Modal::Tracks(m) => draw_tracks(f, app, m, theme),
        Modal::Queue { sel } => super::player::draw_queue(f, app, *sel, theme),
        Modal::Lyrics { manual } => super::player::draw_lyrics(f, app, *manual, theme),
        Modal::Player { cursor } => super::player::draw_player(f, app, *cursor, theme),
        Modal::Failure { cursor } => super::player::draw_failure(f, app, *cursor, theme),
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
                    Span::raw(if matches!(i.purpose, InputPurpose::UserPin { .. }) || i.purpose.secret() {
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
                    match i.purpose {
                        InputPurpose::Password { .. } | InputPurpose::TwoFactor { .. } => " enter sign in · esc cancel",
                        InputPurpose::Login
                        | InputPurpose::LibraryName(_)
                        | InputPurpose::LibraryFolders(_)
                        | InputPurpose::EditLibraryName { .. } => " enter next · esc cancel",
                        InputPurpose::LibraryLanguage(_) => " enter add library · esc cancel",
                        _ => " enter save · esc cancel",
                    },
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
            let resume = matches!(c.action, ConfirmAction::ResumeQueue);
            let width = (c.message.width() as u16 + 6).clamp(30, 70);
            let wrapped = (c.message.width() as u16).div_ceil(width - 4).max(1);
            let area = centered(screen, width, wrapped + 4 + u16::from(wrapped > 1));
            f.render_widget(Clear, area);
            let lines = vec![
                Line::raw(c.message.as_str()),
                Line::raw(""),
                Line::from(vec![
                    Span::styled("y", Style::default().fg(theme.accent).bold()),
                    Span::raw(if resume { " resume   " } else { " yes   " }),
                    Span::styled("n", Style::default().fg(theme.accent).bold()),
                    Span::raw(if resume { " not now   " } else { " no" }),
                    Span::styled(if resume { "x" } else { "" }, Style::default().fg(theme.accent).bold()),
                    Span::raw(if resume { " forget" } else { "" }),
                ]),
            ];
            f.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: true })
                    .block(modal_block(" Confirm ", theme).padding(ratatui::widgets::Padding::horizontal(1))),
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
            let area = centered(screen, 70, m.fields.len() as u16 + 2);
            f.render_widget(Clear, area);
            let inner_w = area.width as usize - 2;
            let rows: Vec<String> = m
                .fields
                .iter()
                .map(|(field, label)| {
                    let lock = if m.item.is_field_locked(field) { "🔒" } else { "  " };
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

fn draw_filters(f: &mut Frame, app: &App, p: &FilterPicker, theme: &Theme) {
    let view = app.columns.last().and_then(|c| c.library());
    let dim = Style::default().fg(theme.dim);
    let (title, hint, rows, cursor, status): (String, &str, Vec<String>, usize, Option<Span>) = match &p.values {
        Some(v) => {
            let rows = v
                .shown()
                .iter()
                .map(|(value, label)| format!("[{}] {label}", if view.is_some_and(|w| w.is_on(&v.def.filter, value)) { "✓" } else { " " }))
                .collect();
            let status = if v.loading {
                Some(Span::styled(format!(" {} Loading…", spinner(app)), dim))
            } else if let Some(e) = &v.error {
                Some(Span::styled(format!(" {e}"), Style::default().fg(theme.error)))
            } else if v.shown().is_empty() {
                Some(Span::styled(" Nothing to choose", dim))
            } else {
                None
            };
            (format!(" {} ", v.def.title), " type to narrow · enter toggle · esc back ", rows, v.cursor, status)
        }
        None => {
            let mut rows: Vec<String> = p
                .defs
                .iter()
                .map(|d| {
                    let on: Vec<&str> = view.map(|w| w.filters.iter().filter(|x| x.filter == d.filter).map(|x| x.label.as_str()).collect()).unwrap_or_default();
                    if d.kind == "boolean" {
                        format!("[{}] {}", if on.is_empty() { " " } else { "✓" }, d.title)
                    } else {
                        format!("    {:<18} {} ›", d.title, on.join(", "))
                    }
                })
                .collect();
            if view.is_some_and(|w| !w.filters.is_empty()) {
                rows.push("    Clear all filters".into());
            }
            let status = if p.loading {
                Some(Span::styled(format!(" {} Loading filters…", spinner(app)), dim))
            } else {
                p.error.as_ref().map(|e| Span::styled(format!(" {e}"), Style::default().fg(theme.error)))
            };
            (format!(" Filter {} ", p.section_title), " enter toggle / choose · esc close ", rows, p.cursor, status)
        }
    };
    let query = p.values.as_ref().map(|v| v.query.clone());
    let extra = u16::from(query.is_some()) + u16::from(status.is_some());
    let area = centered(f.area(), 52, (rows.len() as u16 + 2 + extra).clamp(5, 24));
    f.render_widget(Clear, area);
    let block = modal_block(title, theme).title_bottom(Line::from(Span::styled(hint, dim)));
    let inner_w = area.width as usize - 2;
    let mut lines: Vec<Line> = Vec::new();
    if let Some(q) = query {
        lines.push(Line::from(vec![Span::styled(" › ", Style::default().fg(theme.accent).bold()), Span::raw(q), Span::styled("█", Style::default().fg(theme.accent))]));
    }
    if let Some(s) = status {
        lines.push(Line::from(s));
    }
    let visible = (area.height.saturating_sub(2) as usize).saturating_sub(lines.len());
    let skip = cursor.saturating_sub(visible.saturating_sub(1));
    lines.extend(list_lines(&rows, cursor, theme, inner_w).into_iter().skip(skip));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_tracks(f: &mut Frame, app: &App, m: &TracksModal, theme: &Theme) {
    let screen = f.area();
    let max_h = screen.height.saturating_sub(4).min(34) as usize;
    let width = centered(screen, 76, 3).width as usize - 2;
    let draw = |f: &mut Frame, lines: Vec<Line<'static>>, block: Block<'static>| {
        let area = centered(screen, 76, lines.len() as u16 + 2);
        f.render_widget(Clear, area);
        f.render_widget(Paragraph::new(lines).block(block), area);
    };
    let dim = Style::default().fg(theme.dim);
    let hint = match (m.progress, m.choosing) {
        (Some(_), _) => " updating… ",
        (_, Some(_)) => " enter choose · esc back ",
        _ => " enter choose · ←/→ change · r remember · a apply · esc close ",
    };
    let block = modal_block(format!(" Audio & subtitles · {} ", truncate(&m.title, 48)), theme).title_bottom(Line::from(Span::styled(hint, dim)));
    let mut lines = vec![Line::from(Span::styled(" Pick tracks in one episode; the closest match is selected in every other.", dim)), Line::raw("")];
    let status = |text: String, style: Style| Line::from(Span::styled(format!(" {text}"), style));
    let Some(v) = &m.view else {
        lines.push(match &m.error {
            Some(e) => status(e.clone(), Style::default().fg(theme.error)),
            None => status(format!("{} Reading tracks from every episode…", spinner(app)), dim),
        });
        return draw(f, lines, block);
    };
    if v.total == 0 {
        lines.push(status("This show has no episodes.".into(), dim));
        return draw(f, lines, block);
    }
    let eps = |n: usize| format!("{n} episode{}", if n == 1 { "" } else { "s" });
    if let Some((field, at)) = m.choosing {
        lines.push(status(TRACK_FIELDS[field].to_string(), Style::default().fg(theme.accent).bold()));
        let (labels, current) = m.choices(field);
        let rows: Vec<String> = labels.iter().enumerate().map(|(i, l)| format!("{} {l}", if i == current { "●" } else { " " })).collect();
        let visible = max_h.saturating_sub(5);
        let skip = at.saturating_sub(visible.saturating_sub(1));
        lines.extend(list_lines(&rows, at, theme, width).into_iter().skip(skip).take(visible));
        return draw(f, lines, block);
    }
    for (i, name) in TRACK_FIELDS.iter().enumerate() {
        let (labels, at) = m.choices(i);
        let value = labels.get(at).cloned().unwrap_or_else(|| "—".into());
        let text = format!("{name:<18}{}", truncate(&value, width.saturating_sub(21)));
        let line = if i == m.cursor && m.progress.is_none() {
            highlight(Line::from(format!(" {text}")), width, selected_style(theme))
        } else {
            Line::from(vec![Span::styled(format!(" {name:<18}"), dim), Span::raw(truncate(&value, width.saturating_sub(21)))])
        };
        lines.push(line);
    }
    lines.push(Line::from(vec![
        Span::styled(format!(" [{}] ", if m.remember { "x" } else { " " }), if m.remember { Style::default().fg(theme.accent).bold() } else { dim }),
        Span::styled("Remember for this show (r): use these languages for new episodes too", dim),
    ]));
    let apply = if v.plan.episodes > 0 {
        format!("Apply to {}", eps(v.plan.episodes))
    } else if m.remember && m.can_apply() {
        "Remember".into()
    } else {
        "Apply".into()
    };
    let apply_style = if !m.can_apply() {
        dim
    } else if m.cursor == TRACK_FIELDS.len() {
        selected_style(theme)
    } else {
        Style::default().fg(theme.accent).bold()
    };
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(format!(" [ {apply} ] "), apply_style)));
    lines.push(Line::raw(""));
    if let Some((done, total)) = m.progress {
        let bar = progress_bar(done as f64 / total.max(1) as f64, width.saturating_sub(4).min(40));
        lines.push(Line::from(vec![Span::styled(format!(" {bar} "), Style::default().fg(theme.accent)), Span::raw(format!("Updating {done} of {total}…"))]));
    } else if v.nothing_chosen {
        lines.push(status("Choose an audio or subtitle track. Matches are found by language".into(), dim));
        lines.push(status("first, then title, codec and channels.".into(), dim));
    } else if v.plan.episodes == 0 && v.plan.no_audio.is_empty() && v.plan.no_subtitle.is_empty() {
        lines.push(status("Every episode already uses these tracks.".into(), Style::default()));
    } else {
        lines.push(status(format!("{} of {} will change.", v.plan.episodes, eps(v.scope.len())), Style::default()));
    }
    if m.progress.is_none() {
        let warn = Style::default().fg(theme.star);
        for (missing, what) in [(&v.plan.no_audio, format!("no audio matching “{}”", v.audio_name.as_deref().unwrap_or(""))), (&v.plan.no_subtitle, format!("no subtitles matching “{}”", v.subtitle_name.as_deref().unwrap_or("")))] {
            if missing.is_empty() {
                continue;
            }
            let n = missing.len();
            lines.push(status(format!("⚠ {} {} {what}, left unchanged", eps(n), if n == 1 { "has" } else { "have" }), warn));
            let room = max_h.saturating_sub(lines.len() + 5).clamp(1, 6);
            for e in missing.iter().take(room) {
                lines.push(Line::from(Span::styled(format!("   {}", truncate(e, width.saturating_sub(4))), dim)));
            }
            if n > room {
                lines.push(Line::from(Span::styled(format!("   … and {} more", n - room), dim)));
            }
        }
    }
    draw(f, lines, block);
}
