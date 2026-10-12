use super::*;
use crossterm::event::{KeyCode, KeyEvent};
use patina_core::{prefs, tracks};

impl App {
    /// `T`: pick audio and subtitles in one episode, applied across the show or a season.
    pub(super) fn open_tracks(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        let Some(m) = TracksModal::new(&item) else {
            return self.info("Audio & subtitles for a whole show: select a show, season or episode");
        };
        let (core, tx, show) = (self.core.clone(), self.tx.clone(), m.show.clone());
        self.modal = Some(Modal::Tracks(Box::new(m)));
        tokio::spawn(async move {
            let result = tracks::load(&core, &show).await.map_err(err);
            let _ = tx.send(Msg::TracksLoaded(show, result));
        });
    }

    pub(super) fn on_tracks_loaded(&mut self, show: String, result: Result<(), String>) {
        let Some(Modal::Tracks(m)) = &mut self.modal else { return };
        if m.show != show {
            return;
        }
        match result {
            Ok(()) => {
                m.loaded = true;
                self.replan();
            }
            Err(e) => m.error = Some(e),
        }
    }

    /// Ask the core for a fresh plan after a choice changed.
    fn replan(&mut self) {
        let Some(Modal::Tracks(m)) = &mut self.modal else { return };
        match tracks::view(&self.core, &m.query()) {
            Ok(v) => m.view = Some(v),
            Err(e) => m.error = Some(err(e)),
        }
    }

    fn apply_tracks(&mut self) {
        let Some(Modal::Tracks(m)) = &self.modal else { return };
        let Some(changes) = m.view.as_ref().map(|v| v.plan.changes.clone()).filter(|_| m.can_apply()) else { return };
        let remember = m.remember.then(|| m.query());
        // Before applying: applying drops the loaded tracks the rules are read from.
        // The core announces success with a toast.
        if let Some(q) = remember
            && let Err(e) = prefs::remember_tracks(&self.core, &q)
        {
            self.modal = None;
            return self.error(err(e));
        }
        if changes.is_empty() {
            self.modal = None;
            return;
        }
        if let Some(Modal::Tracks(m)) = &mut self.modal {
            m.progress = Some((0, changes.len()));
        }
        let (core, tx) = (self.core.clone(), self.tx.clone());
        tokio::spawn(async move {
            let ptx = tx.clone();
            let result = tracks::apply(&core, changes, move |done, total| {
                let _ = ptx.send(Msg::TracksProgress(done, total));
            })
            .await
            .map_err(err);
            let _ = tx.send(Msg::TracksApplied(result));
        });
    }

    pub(super) fn on_tracks_progress(&mut self, done: usize, total: usize) {
        if let Some(Modal::Tracks(m)) = &mut self.modal
            && m.progress.is_some()
        {
            m.progress = Some((done, total));
        }
    }

    pub(super) fn on_tracks_applied(&mut self, result: Result<(), String>) {
        if matches!(&self.modal, Some(Modal::Tracks(m)) if m.progress.is_some()) {
            self.modal = None;
        }
        // Success is announced by the core's toast.
        if let Err(e) = result {
            self.error(e);
        }
    }

    pub(super) fn tracks_key(&mut self, mut m: Box<TracksModal>, key: KeyEvent) -> Option<Modal> {
        let fields = TRACK_FIELDS.len();
        if m.progress.is_some() {
            // Busy: the dialog stays until the updates land, as on the GUI.
            return Some(Modal::Tracks(m));
        }
        let code = key.code;
        let down = matches!(code, KeyCode::Char('j') | KeyCode::Down);
        let up = matches!(code, KeyCode::Char('k') | KeyCode::Up);
        if let Some((field, mut at)) = m.choosing {
            let n = m.choices(field).0.len();
            match code {
                _ if down => at = (at + 1).min(n.saturating_sub(1)),
                _ if up => at = at.saturating_sub(1),
                KeyCode::Char('g') | KeyCode::Home => at = 0,
                KeyCode::Char('G') | KeyCode::End => at = n.saturating_sub(1),
                KeyCode::Enter | KeyCode::Char(' ') => {
                    m.choose(field, at);
                    m.choosing = None;
                    self.modal = Some(Modal::Tracks(m));
                    self.replan();
                    return self.modal.take();
                }
                KeyCode::Esc | KeyCode::Char('q' | 'h') | KeyCode::Left => {
                    m.choosing = None;
                    return Some(Modal::Tracks(m));
                }
                _ => {}
            }
            m.choosing = Some((field, at));
            return Some(Modal::Tracks(m));
        }
        match code {
            _ if down => m.cursor = (m.cursor + 1).min(fields),
            _ if up => m.cursor = m.cursor.saturating_sub(1),
            KeyCode::Esc | KeyCode::Char('q') => return None,
            KeyCode::Char('a') => {
                self.modal = Some(Modal::Tracks(m));
                self.apply_tracks();
                return self.modal.take();
            }
            // Also keep these choices as the show's automatic playback rules.
            KeyCode::Char('r') if m.view.is_some() => m.remember = !m.remember,
            // Left/right step through a field's choices in place.
            KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l') if m.cursor < fields && m.view.is_some() => {
                let (labels, at) = m.choices(m.cursor);
                let next = if matches!(code, KeyCode::Left | KeyCode::Char('h')) { at.checked_sub(1) } else { Some(at + 1).filter(|i| *i < labels.len()) };
                if let Some(i) = next {
                    m.choose(m.cursor, i);
                    self.modal = Some(Modal::Tracks(m));
                    self.replan();
                    return self.modal.take();
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') if m.cursor == fields => {
                self.modal = Some(Modal::Tracks(m));
                self.apply_tracks();
                return self.modal.take();
            }
            KeyCode::Enter | KeyCode::Char(' ') if m.view.is_some() => {
                let (labels, at) = m.choices(m.cursor);
                if !labels.is_empty() {
                    m.choosing = Some((m.cursor, at));
                }
            }
            _ => {}
        }
        Some(Modal::Tracks(m))
    }
}
