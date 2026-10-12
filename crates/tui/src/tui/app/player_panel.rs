//! Player controls beyond transport (volume, speed, repeat, sleep timer, chapters), the
//! player panel with its diagnostics, playback failures and the Resume offer at start-up.

use super::*;
use crossterm::event::{KeyCode, KeyEvent};
use patina_core::controls::Tune;
use patina_core::failure::Recover;
use patina_core::ops::playback as ops;

/// One press of `9`/`0` and `{`/`}`.
const VOLUME_STEP: f64 = 5.0;
const SPEED_STEP: f64 = 0.25;

impl App {
    pub(super) fn tune(&mut self, tune: Tune) {
        if self.now_playing.is_none() {
            return self.info("Nothing is playing");
        }
        player::tune(&self.core, tune);
    }

    /// Volume, mute, speed and the sleep timer, from the browser or any player panel.
    pub(super) fn tune_key(&mut self, key: KeyEvent) -> bool {
        let tune = match key.code {
            KeyCode::Char('9') => Tune::VolumeBy { delta: -VOLUME_STEP },
            KeyCode::Char('0') => Tune::VolumeBy { delta: VOLUME_STEP },
            KeyCode::Char('M') => Tune::Mute { on: None },
            KeyCode::Char('{') => Tune::SpeedBy { delta: -SPEED_STEP },
            KeyCode::Char('}') => Tune::SpeedBy { delta: SPEED_STEP },
            KeyCode::Char('Z') => Tune::CycleSleep,
            _ => return false,
        };
        self.tune(tune);
        true
    }

    // ---- player panel -----------------------------------------------------

    /// `V`: the player panel, or the options after a failure when nothing is playing.
    pub(super) fn open_player(&mut self) {
        if self.now_playing.is_none() {
            if self.failure.is_some() {
                self.modal = Some(Modal::Failure { cursor: 0 });
            } else {
                self.info("Nothing is playing");
            }
            return;
        }
        let cursor = self.playback.as_ref().and_then(|p| p.chapter).unwrap_or(0);
        self.modal = Some(Modal::Player { cursor });
    }

    pub(super) fn player_key(&mut self, cursor: usize, key: KeyEvent) -> Option<Modal> {
        if self.transport_key(key) {
            return Some(Modal::Player { cursor });
        }
        let chapters = self.playback.as_ref().map_or(0, |p| p.chapters.len());
        let last = chapters.saturating_sub(1);
        let mut cursor = cursor.min(last);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => cursor = (cursor + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => cursor = cursor.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => cursor = 0,
            KeyCode::Char('G') | KeyCode::End => cursor = last,
            KeyCode::Enter if chapters > 0 => self.tune(Tune::Chapter { index: cursor }),
            KeyCode::Char('r') => self.tune(Tune::CycleRepeat),
            KeyCode::Char('Q') => {
                self.open_queue();
                return self.modal.take();
            }
            KeyCode::Esc | KeyCode::Char('q' | 'V') => return None,
            _ => {}
        }
        Some(Modal::Player { cursor })
    }

    // ---- failures ---------------------------------------------------------

    pub(super) fn on_failure(&mut self, failure: Option<PlaybackFailure>) {
        self.failure = failure;
        let Some(f) = &self.failure else {
            if matches!(self.modal, Some(Modal::Failure { .. })) {
                self.modal = None;
            }
            return;
        };
        // Don't take over a dialog the user is typing into.
        let free = matches!(self.modal, None | Some(Modal::Queue { .. } | Modal::Lyrics { .. } | Modal::Player { .. } | Modal::Failure { .. } | Modal::Help { .. }));
        if free {
            self.modal = Some(Modal::Failure { cursor: 0 });
        } else {
            let text = format!("{} Press V for options.", f.reason);
            self.error(text);
        }
    }

    pub(super) fn failure_key(&mut self, cursor: usize, key: KeyEvent) -> Option<Modal> {
        let f = self.failure.clone()?;
        let rows = failure_actions(&f);
        let last = rows.len() - 1;
        let cursor = cursor.min(last);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Modal::Failure { cursor: (cursor + 1).min(last) }),
            KeyCode::Char('k') | KeyCode::Up => Some(Modal::Failure { cursor: cursor.saturating_sub(1) }),
            KeyCode::Char('g') | KeyCode::Home => Some(Modal::Failure { cursor: 0 }),
            KeyCode::Char('G') | KeyCode::End => Some(Modal::Failure { cursor: last }),
            KeyCode::Enter => {
                self.run_failure_action(rows[cursor].0);
                None
            }
            // Closing the dialog is dismissing the failure; V reopens it while it is kept.
            KeyCode::Esc | KeyCode::Char('q') => {
                failure::clear(&self.core);
                None
            }
            _ => Some(Modal::Failure { cursor }),
        }
    }

    fn run_failure_action(&mut self, action: FailureAction) {
        let recover = match action {
            FailureAction::Retry => Recover::Retry,
            FailureAction::Version(index) => Recover::Version { index },
            FailureAction::LowerQuality => Recover::LowerQuality,
            FailureAction::Dismiss => return failure::clear(&self.core),
        };
        match failure::recover(&self.core, recover) {
            Ok(()) => self.info("Trying again…"),
            Err(e) => self.error(err(e)),
        }
    }

    // ---- resume -----------------------------------------------------------

    /// Offer the queue that was playing when Patina last stopped. Nothing plays until it is accepted.
    pub(super) fn offer_resume(&mut self) {
        if self.now_playing.is_some() || self.modal.is_some() {
            return;
        }
        let Some(saved) = ops::saved_queue(&self.core) else { return };
        let title = Item::from(saved.current.clone()).full_title();
        let at = if saved.position_ms > 0 { format!(" at {}", crate::format::clock_ms(saved.position_ms)) } else { String::new() };
        let queue = if saved.count > 1 { format!(" (item {} of {})", saved.index + 1, saved.count) } else { String::new() };
        self.modal = Some(Modal::Confirm(Confirm { message: format!("Resume {title}{at}{queue}?"), action: ConfirmAction::ResumeQueue }));
    }

    pub(super) fn resume_queue(&mut self) {
        match ops::resume_saved(&self.core) {
            Ok(()) => self.info("Resuming…"),
            Err(e) => self.error(err(e)),
        }
    }

    pub(super) fn forget_queue(&mut self) {
        ops::forget_saved(&self.core);
        self.info("Forgot the saved queue");
    }
}
