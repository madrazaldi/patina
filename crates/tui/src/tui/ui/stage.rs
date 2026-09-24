use super::*;

pub(super) fn draw_stage(f: &mut Frame, app: &App, theme: &Theme, area: Rect) {
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
            if !app.downloads.is_empty() {
                lines.push(Line::raw(""));
                lines.push(hints(&[("d", "downloads offline"), ("q", "quit")]));
            }
        }
        Stage::Login => {
            title = "Sign in";
            match &app.login.code {
                Some((_, url)) if app.login.browser => {
                    lines.push(Line::raw("Approve the sign-in in your browser."));
                    lines.push(Line::raw(""));
                    lines.push(Line::from(Span::styled(url.clone(), dim)));
                    lines.push(Line::raw(""));
                    lines.push(Line::from(Span::styled(format!("{} Waiting for plex.tv…", spinner(app)), dim)));
                }
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
            lines.push(hints(&[("b", "browser sign-in"), ("p", "email & password"), ("r", "link code")]));
            lines.push(hints(&[("o", "open plex.tv/link"), ("m", "server URL + token"), ("q", "quit")]));
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
            let mut h = vec![("r", "retry"), ("L", "sign out"), ("q", "quit")];
            if !app.downloads.is_empty() {
                h.insert(1, ("d", "downloads offline"));
            }
            lines.push(hints(&h));
        }
    }
    let block = modal_block(Span::styled(format!(" {title} "), Style::default().fg(theme.accent).bold()), theme)
        .padding(ratatui::widgets::Padding::uniform(1));
    let para = Paragraph::new(lines).alignment(Alignment::Center).wrap(Wrap { trim: false });
    let width = 60.min(area.width.saturating_sub(4));
    let height = para.line_count(width.saturating_sub(4)) as u16 + 4;
    f.render_widget(para.block(block), centered(area, width, height));
}
