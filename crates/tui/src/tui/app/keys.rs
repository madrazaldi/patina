use super::*;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use patina_core::session;

use crate::tui::column::SortKey;

impl App {
    // ---- input -----------------------------------------------------------

    pub fn on_event(&mut self, ev: Event) {
        match ev {
            Event::Key(k) if k.kind != KeyEventKind::Release => self.on_key(k),
            Event::Paste(text) => {
                if let Some(s) = &mut self.search {
                    s.input.push_str(text.trim());
                    self.search_changed();
                } else if let Some(Modal::Input(i)) = &mut self.modal {
                    i.value.push_str(text.trim());
                } else if let Some(col) = self.columns.last_mut().filter(|c| c.filtering) {
                    col.filter.push_str(text.trim());
                    col.refilter(&mut self.fuzzy);
                }
            }
            _ => {}
        }
    }

    pub(super) fn on_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        if self.modal.is_some() {
            self.modal_key(key);
        } else if !self.ready() {
            self.stage_key(key);
            return;
        } else if self.search.is_some() {
            self.search_key(key);
        } else if self.columns.last().is_some_and(|c| c.filtering) {
            self.filter_key(key);
        } else {
            self.normal_key(key);
        }
        self.after_selection_change();
    }

    pub(super) fn filter_key(&mut self, key: KeyEvent) {
        let Some(col) = self.columns.last_mut() else { return };
        match key.code {
            KeyCode::Esc => {
                col.filtering = false;
                col.filter.clear();
                col.refilter(&mut self.fuzzy);
            }
            KeyCode::Enter => col.filtering = false,
            KeyCode::Backspace => {
                if col.filter.pop().is_none() {
                    col.filtering = false;
                }
                col.refilter(&mut self.fuzzy);
            }
            KeyCode::Down => col.move_by(1),
            KeyCode::Up => col.move_by(-1),
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                col.filter.push(c);
                col.refilter(&mut self.fuzzy);
            }
            _ => {}
        }
    }

    pub(super) fn normal_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let page = self.list_height.max(2) as isize;
        let Some(col) = self.columns.last_mut() else { return };
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.modal = Some(Modal::Help { scroll: 0 }),
            KeyCode::Char('j') | KeyCode::Down => col.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => col.move_by(-1),
            KeyCode::Char('d') if ctrl => col.move_by(page / 2),
            KeyCode::Char('u') if ctrl => col.move_by(-page / 2),
            KeyCode::Char('f') if ctrl => col.move_by(page),
            KeyCode::Char('b') if ctrl => col.move_by(-page),
            KeyCode::PageDown => col.move_by(page),
            KeyCode::PageUp => col.move_by(-page),
            KeyCode::Char('g') | KeyCode::Home => col.move_to(0),
            KeyCode::Char('G') | KeyCode::End => col.move_to(usize::MAX),
            KeyCode::Char('l') | KeyCode::Right => self.drill(),
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Backspace => self.back(),
            KeyCode::Esc => {
                if !col.filter.is_empty() {
                    col.filter.clear();
                    col.refilter(&mut self.fuzzy);
                } else if self.notice.is_some() {
                    self.notice = None;
                }
            }
            KeyCode::Enter => {
                if col.selected().is_some_and(Item::is_playable) {
                    self.play_selected(false);
                } else {
                    self.drill();
                }
            }
            KeyCode::Char('p') => {
                if col.selected().is_some_and(Item::is_playable) {
                    self.play_selected(true);
                }
            }
            KeyCode::Char('a') => {
                if let Some(i) = col.selected().cloned() {
                    self.play_container(i, false);
                }
            }
            KeyCode::Char('z') => {
                if let Some(i) = col.selected().cloned() {
                    self.play_container(i, true);
                }
            }
            KeyCode::Char('o') => self.open_play_options(),
            KeyCode::Char('P' | '>' | '<' | '=' | '-' | 'X') => {
                self.transport_key(key);
            }
            KeyCode::Char('n') => self.enqueue(true),
            KeyCode::Char('N') => self.enqueue(false),
            KeyCode::Char('Q') => self.open_queue(),
            KeyCode::Char('Y') => self.open_lyrics(),
            KeyCode::Char('E') => self.toggle_stop_after(),
            KeyCode::Char('w') => self.set_watched(true),
            KeyCode::Char('u') => self.set_watched(false),
            KeyCode::Char('*') => self.open_rate(),
            KeyCode::Char(' ') => self.open_playlist_picker(),
            KeyCode::Char('x') | KeyCode::Delete => self.delete_action(),
            KeyCode::Char('e') => {
                if col.selected().is_some_and(|i| i.kind() == Kind::Playlist) {
                    self.open_rename();
                } else {
                    self.open_edit();
                }
            }
            KeyCode::Char('+') => self.toggle_watchlist(),
            KeyCode::Char('C') => self.open_collections(),
            KeyCode::Char('T') => self.open_tracks(),
            KeyCode::Char('c') => self.open_related(),
            KeyCode::Char('U') => self.open_users(),
            KeyCode::Char('J') => self.move_entry(true),
            KeyCode::Char('K') => self.move_entry(false),
            KeyCode::Char('f') => self.open_search(),
            KeyCode::Char('/') => col.filtering = true,
            KeyCode::Char('s') if col.library().is_some() => self.open_sort(),
            KeyCode::Char('s') => {
                let cursor = SortKey::ALL.iter().position(|k| *k == col.sort).unwrap_or(0);
                self.modal = Some(Modal::Sort { cursor });
            }
            KeyCode::Char('F') => self.open_filters(),
            KeyCode::Char('A') => self.open_letters(),
            KeyCode::Char('t') => self.cycle_type(),
            KeyCode::Char('v') => self.open_browse(),
            KeyCode::Char('H') => {
                col.hide_watched = !col.hide_watched;
                let on = col.hide_watched;
                col.rebuild(&mut self.fuzzy);
                self.info(if on { "Hiding watched items" } else { "Showing watched items" });
            }
            KeyCode::Char('i') => self.show_inspector = !self.show_inspector,
            KeyCode::Char(']') => self.inspector_scroll = self.inspector_scroll.saturating_add(3),
            KeyCode::Char('[') => self.inspector_scroll = self.inspector_scroll.saturating_sub(3),
            KeyCode::Char('r') => {
                let idx = self.columns.len() - 1;
                if idx == 0 {
                    let core = self.core.clone();
                    tokio::spawn(async move {
                        let _ = session::reload_sections(&core).await;
                    });
                } else {
                    if let Some(k) = self.selected().and_then(|i| i.rating_key.clone()) {
                        self.details.remove(&k);
                    }
                    self.load(idx);
                }
            }
            KeyCode::Char('R') => self.refresh_all(),
            KeyCode::Char('S') => self.scan_library(false),
            KeyCode::Char('m' | '.') => self.open_menu(),
            KeyCode::Char('O') => self.open_web(),
            KeyCode::Char('y') => self.copy_url(),
            KeyCode::Char('D') => self.download_selected(),
            KeyCode::Char('L') => {
                self.modal = Some(Modal::Confirm(Confirm { message: "Sign out of Patina on this device?".into(), action: ConfirmAction::Logout }))
            }
            _ => {}
        }
    }

    pub(super) fn modal_key(&mut self, key: KeyEvent) {
        let Some(modal) = self.modal.take() else { return };
        let down = matches!(key.code, KeyCode::Char('j') | KeyCode::Down);
        let up = matches!(key.code, KeyCode::Char('k') | KeyCode::Up);
        let esc = matches!(key.code, KeyCode::Esc | KeyCode::Char('q'));
        let enter = key.code == KeyCode::Enter;

        let next: Option<Modal> = match modal {
            Modal::Help { scroll } => {
                if down {
                    Some(Modal::Help { scroll: scroll.saturating_add(1) })
                } else if up {
                    Some(Modal::Help { scroll: scroll.saturating_sub(1) })
                } else {
                    None
                }
            }
            Modal::Sort { mut cursor } => {
                let n = SortKey::ALL.len();
                if down {
                    cursor = (cursor + 1).min(n - 1);
                    Some(Modal::Sort { cursor })
                } else if up {
                    cursor = cursor.saturating_sub(1);
                    Some(Modal::Sort { cursor })
                } else if enter || key.code == KeyCode::Char(' ') {
                    let k = SortKey::ALL[cursor];
                    if let Some(col) = self.columns.last_mut() {
                        if col.sort == k {
                            col.sort_desc = !col.sort_desc;
                        } else {
                            col.sort = k;
                            col.sort_desc = k.natural_desc();
                        }
                        col.rebuild(&mut self.fuzzy);
                        col.move_to(0);
                    }
                    None
                } else if esc {
                    None
                } else {
                    Some(Modal::Sort { cursor })
                }
            }
            Modal::Browse { section_id, section_title, rows, mut cursor } => {
                if down {
                    cursor = (cursor + 1).min(rows.len().saturating_sub(1));
                    Some(Modal::Browse { section_id, section_title, rows, cursor })
                } else if up {
                    cursor = cursor.saturating_sub(1);
                    Some(Modal::Browse { section_id, section_title, rows, cursor })
                } else if enter {
                    if let Some((label, source)) = rows.into_iter().nth(cursor) {
                        self.browse_to(&section_id, &section_title, &label, source);
                    }
                    None
                } else if esc {
                    None
                } else {
                    Some(Modal::Browse { section_id, section_title, rows, cursor })
                }
            }
            Modal::Filter(p) => self.filter_modal_key(p, key),
            Modal::Tracks(m) => self.tracks_key(m, key),
            Modal::Queue { sel } => self.queue_key(sel, key),
            Modal::Lyrics { manual } => self.lyrics_key(manual, key),
            Modal::Menu(mut m) => {
                if down {
                    m.cursor = (m.cursor + 1).min(m.rows.len().saturating_sub(1));
                    Some(Modal::Menu(m))
                } else if matches!(key.code, KeyCode::Char('g') | KeyCode::Home) {
                    m.cursor = 0;
                    Some(Modal::Menu(m))
                } else if matches!(key.code, KeyCode::Char('G') | KeyCode::End) {
                    m.cursor = m.rows.len().saturating_sub(1);
                    Some(Modal::Menu(m))
                } else if up {
                    m.cursor = m.cursor.saturating_sub(1);
                    Some(Modal::Menu(m))
                } else if enter {
                    if let Some(row) = m.rows.get(m.cursor) {
                        self.run_menu(row.0);
                    }
                    return;
                } else if esc {
                    None
                } else {
                    Some(Modal::Menu(m))
                }
            }
            Modal::Input(mut i) => match key.code {
                KeyCode::Esc => match i.purpose {
                    InputPurpose::EditField { edit, .. } => Some(Modal::Edit(*edit)),
                    _ => None,
                },
                KeyCode::Enter => {
                    self.submit_input(i);
                    return;
                }
                KeyCode::Backspace => {
                    i.value.pop();
                    Some(Modal::Input(i))
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    i.value.clear();
                    Some(Modal::Input(i))
                }
                KeyCode::Char(c) => {
                    i.value.push(c);
                    Some(Modal::Input(i))
                }
                _ => Some(Modal::Input(i)),
            },
            Modal::Confirm(c) => match key.code {
                KeyCode::Char('y' | 'Y') | KeyCode::Enter => {
                    self.run_confirmed(c.action);
                    None
                }
                KeyCode::Char('n' | 'N') | KeyCode::Esc | KeyCode::Char('q') => None,
                _ => Some(Modal::Confirm(c)),
            },
            Modal::Playlists(mut p) => {
                let n = p.rows.len() + 1;
                if down {
                    p.cursor = (p.cursor + 1).min(n - 1);
                    Some(Modal::Playlists(p))
                } else if up {
                    p.cursor = p.cursor.saturating_sub(1);
                    Some(Modal::Playlists(p))
                } else if key.code == KeyCode::Char('n') {
                    p.cursor = p.rows.len();
                    self.modal = Some(Modal::Playlists(p));
                    self.toggle_playlist_row();
                    return;
                } else if (enter || key.code == KeyCode::Char(' ')) && !p.loading && p.error.is_none() {
                    self.modal = Some(Modal::Playlists(p));
                    self.toggle_playlist_row();
                    return;
                } else if esc {
                    None
                } else {
                    Some(Modal::Playlists(p))
                }
            }
            Modal::PlayOptions(mut m) => {
                let n = m.rows.len();
                if down {
                    let mut c = m.cursor + 1;
                    while c < n && !m.selectable(c) {
                        c += 1;
                    }
                    if c < n {
                        m.cursor = c;
                    }
                    Some(Modal::PlayOptions(m))
                } else if up {
                    let mut c = m.cursor;
                    while c > 0 {
                        c -= 1;
                        if m.selectable(c) {
                            m.cursor = c;
                            break;
                        }
                    }
                    Some(Modal::PlayOptions(m))
                } else if enter || key.code == KeyCode::Char(' ') {
                    let row = m.rows.get(m.cursor).map(|r| r.0.clone());
                    match row {
                        Some(OptRow::Play) => {
                            self.play_with_options(m);
                            None
                        }
                        Some(OptRow::Version(i)) => {
                            m.opts.media_index = i;
                            m.opts.audio_stream_id = None;
                            m.opts.subtitle_stream_id = None;
                            build_option_rows(&mut m);
                            Some(Modal::PlayOptions(m))
                        }
                        Some(OptRow::Audio(id)) => {
                            m.opts.audio_stream_id = Some(id);
                            Some(Modal::PlayOptions(m))
                        }
                        Some(OptRow::Subtitle(id)) => {
                            m.opts.subtitle_stream_id = Some(id.unwrap_or(0));
                            Some(Modal::PlayOptions(m))
                        }
                        Some(OptRow::Resume(from_start)) => {
                            m.opts.from_start = from_start;
                            Some(Modal::PlayOptions(m))
                        }
                        Some(OptRow::Quality(q)) => {
                            m.opts.quality = q;
                            Some(Modal::PlayOptions(m))
                        }
                        _ => Some(Modal::PlayOptions(m)),
                    }
                } else if esc {
                    None
                } else {
                    Some(Modal::PlayOptions(m))
                }
            }
            Modal::Collections(mut c) => {
                let n = c.rows.len() + 1;
                if down {
                    c.cursor = (c.cursor + 1).min(n - 1);
                    Some(Modal::Collections(c))
                } else if up {
                    c.cursor = c.cursor.saturating_sub(1);
                    Some(Modal::Collections(c))
                } else if key.code == KeyCode::Char('n') && !c.loading {
                    c.cursor = c.rows.len();
                    self.modal = Some(Modal::Collections(c));
                    self.toggle_collection_row();
                    return;
                } else if (enter || key.code == KeyCode::Char(' ')) && !c.loading && c.error.is_none() {
                    self.modal = Some(Modal::Collections(c));
                    self.toggle_collection_row();
                    return;
                } else if esc {
                    None
                } else {
                    Some(Modal::Collections(c))
                }
            }
            Modal::Edit(mut m) => {
                let n = m.fields.len();
                if down {
                    m.cursor = (m.cursor + 1).min(n.saturating_sub(1));
                    Some(Modal::Edit(m))
                } else if up {
                    m.cursor = m.cursor.saturating_sub(1);
                    Some(Modal::Edit(m))
                } else if enter && n > 0 {
                    let (field, _) = m.fields[m.cursor];
                    self.edit_field(m, field, None);
                    return;
                } else if key.code == KeyCode::Char('l') && n > 0 {
                    self.toggle_field_lock(&m.item, m.fields[m.cursor].0);
                    None
                } else if esc {
                    None
                } else {
                    Some(Modal::Edit(m))
                }
            }
            Modal::Pick(mut p) => {
                // Letters jump directly; arrows move.
                if let (PickKind::Letters, KeyCode::Char(c)) = (&p.kind, key.code) {
                    let letter = if c.is_alphabetic() { c.to_uppercase().collect() } else { "#".to_string() };
                    if let Some(i) = p.rows.iter().position(|(e, _)| matches!(e, PickEntry::Letter(l, _) if *l == letter)) {
                        p.cursor = i;
                        self.pick_selected(p);
                        return;
                    }
                    return self.modal = Some(Modal::Pick(p));
                }
                if down {
                    p.cursor = (p.cursor + 1).min(p.rows.len().saturating_sub(1));
                    Some(Modal::Pick(p))
                } else if matches!(key.code, KeyCode::Char('g') | KeyCode::Home) {
                    p.cursor = 0;
                    Some(Modal::Pick(p))
                } else if matches!(key.code, KeyCode::Char('G') | KeyCode::End) {
                    p.cursor = p.rows.len().saturating_sub(1);
                    Some(Modal::Pick(p))
                } else if up {
                    p.cursor = p.cursor.saturating_sub(1);
                    Some(Modal::Pick(p))
                } else if key.code == KeyCode::Char('v') {
                    // Preview a poster candidate; the picker stays open.
                    if let Some((PickEntry::Artwork(poster), _)) = p.rows.get(p.cursor)
                        && let Some(k) = &poster.key
                        && let Ok(server) = self.core.server()
                        && let Ok(url) = server.authed_url(k, &Params::new())
                        && let Err(e) = open::that_detached(url.as_str())
                    {
                        self.error(format!("Couldn't open the image: {e}"));
                    }
                    Some(Modal::Pick(p))
                } else if enter && !p.rows.is_empty() {
                    self.pick_selected(p);
                    return;
                } else if esc {
                    None
                } else {
                    Some(Modal::Pick(p))
                }
            }
        };
        if self.modal.is_none() {
            self.modal = next;
        }
    }
}
