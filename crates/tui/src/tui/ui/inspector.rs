use super::*;

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

pub(super) fn progress_bar(frac: f64, width: usize) -> String {
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
        let text = match item.link_source() {
            Some(Source::Continue) => "Movies and episodes in progress, plus the next episode of shows you're watching.",
            Some(Source::Recent) => "The newest additions across every library.",
            Some(Source::Watchlist) => {
                "Your plex.tv watchlist. Titles on this server are playable; ☆ marks ones that aren't. + adds or removes the selection."
            }
            Some(Source::Path(p)) if p == PLAYLISTS => "Your playlists. Space adds the selected item to one; x deletes; e renames.",
            Some(Source::Sessions) => "Who is streaming from this server right now. Refreshes every few seconds; x stops a stream.",
            Some(Source::History) => "Everything played on this server, newest first.",
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

pub(super) fn draw_inspector(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
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
