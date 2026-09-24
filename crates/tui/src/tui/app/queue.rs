//! The play queue panel, play next / add to queue, radio, lyrics and the Up next prompt.

use super::*;
use crossterm::event::{KeyCode, KeyEvent};
use patina_core::lyrics::{self, Lyrics};
use patina_core::player::{Control, PlayRequest};

/// Synced lyrics follow playback again this long after a line was picked by hand.
/// Lyrics of a track: its rating key, and the result once loaded.
pub type LyricsSlot = (String, Option<Result<Option<Lyrics>, String>>);

pub const LYRICS_HOLD: Duration = Duration::from_secs(4);

impl App {
    /// `n` / `N`: queue an item (or everything in a container) next or at the
    /// end. With nothing playing, the core starts playing it.
    pub(super) fn enqueue(&mut self, next: bool) {
        let Some(item) = self.selected().cloned() else { return };
        if !queueable(&item) {
            return self.info("Select something playable to queue");
        }
        let idle = self.now_playing.is_none();
        let title = item.full_title();
        player::queue(self.core.clone(), QueueCommand::Add { item: Box::new(item.meta), next });
        self.success(match (idle, next) {
            (true, _) => format!("Starting {title}…"),
            (false, true) => format!("{title} plays next"),
            (false, false) => format!("Added {title} to the queue"),
        });
    }

    /// Radio for an artist, album or track (a shuffled artist without sonic analysis).
    pub(super) fn start_radio(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        if !matches!(item.kind(), Kind::Artist | Kind::Album | Kind::Track) || item.rating_key.is_none() {
            return self.info("Radio is for artists, albums and tracks");
        }
        self.start_play(PlayRequest { item: item.meta, radio: true, ..Default::default() }, Vec::new());
    }

    pub(super) fn open_queue(&mut self) {
        match &self.queue {
            Some(q) if self.now_playing.is_some() => self.modal = Some(Modal::Queue { sel: q.current }),
            _ => self.info("Nothing is playing"),
        }
    }

    pub(super) fn toggle_stop_after(&mut self) {
        let Some(np) = &self.now_playing else { return self.info("Nothing is playing") };
        let on = !np.stop_after;
        player::queue(self.core.clone(), QueueCommand::StopAfter { on });
        self.info(if on { "Stopping after this" } else { "Playing on after this" });
    }

    pub(super) fn queue_key(&mut self, mut sel: Option<u64>, key: KeyEvent) -> Option<Modal> {
        if self.transport_key(key) {
            return Some(Modal::Queue { sel });
        }
        let q = self.queue.clone().filter(|_| self.now_playing.is_some())?;
        let uids: Vec<u64> = q.items.iter().map(|i| i.uid).collect();
        let at = sel.and_then(|u| uids.iter().position(|x| *x == u)).or_else(|| q.current.and_then(|c| uids.iter().position(|x| *x == c))).unwrap_or(0);
        let send = |cmd| player::queue(self.core.clone(), cmd);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => sel = uids.get((at + 1).min(uids.len().saturating_sub(1))).copied(),
            KeyCode::Char('k') | KeyCode::Up => sel = uids.get(at.saturating_sub(1)).copied(),
            KeyCode::Char('g') | KeyCode::Home => sel = uids.first().copied(),
            KeyCode::Char('G') | KeyCode::End => sel = uids.last().copied(),
            KeyCode::Enter => {
                if let Some(uid) = uids.get(at).copied().filter(|u| Some(*u) != q.current) {
                    send(QueueCommand::Jump { uid });
                }
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                if let Some(uid) = uids.get(at).copied().filter(|u| Some(*u) != q.current) {
                    send(QueueCommand::Remove { uid });
                    sel = uids.get(at + 1).or(at.checked_sub(1).and_then(|i| uids.get(i))).copied();
                } else {
                    self.info("That's playing now; use X to stop or > to skip");
                }
            }
            KeyCode::Char(c @ ('J' | 'K')) => {
                if let (Some(uid), Some(before)) = (uids.get(at).copied(), move_before(&uids, at, c == 'J')) {
                    send(QueueCommand::Move { uid, before });
                }
            }
            KeyCode::Char('z') => send(QueueCommand::Shuffle { on: !q.shuffled }),
            KeyCode::Char('E') => self.toggle_stop_after(),
            KeyCode::Esc | KeyCode::Char('q' | 'Q') => return None,
            _ => {}
        }
        Some(Modal::Queue { sel })
    }

    /// Player controls that work from the browser and the player panels.
    pub(super) fn transport_key(&mut self, key: KeyEvent) -> bool {
        let cmd = match key.code {
            KeyCode::Char('P') => Control::TogglePause,
            KeyCode::Char('>') => Control::Next,
            KeyCode::Char('<') => Control::Previous,
            KeyCode::Char('=') => Control::SeekRelative(SEEK_SECONDS),
            KeyCode::Char('-') => Control::SeekRelative(-SEEK_SECONDS),
            KeyCode::Char('X') => Control::Stop,
            _ => return false,
        };
        self.control(cmd);
        true
    }

    // ---- lyrics --------------------------------------------------------------

    pub(super) fn open_lyrics(&mut self) {
        let Some(np) = &self.now_playing else { return self.info("Nothing is playing") };
        if np.item.kind() != "track" {
            return self.info("Lyrics are for music tracks");
        }
        self.modal = Some(Modal::Lyrics { manual: None });
        self.load_lyrics();
    }

    /// Fetch the playing track's lyrics unless they are loaded (or loading).
    pub(super) fn load_lyrics(&mut self) {
        let Some(key) = self.now_playing.as_ref().filter(|np| np.item.kind() == "track").map(|np| np.item.rating_key().to_string()) else { return };
        if self.lyrics.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        self.lyrics = Some((key.clone(), None));
        let (server, tx) = (self.server(), self.tx.clone());
        tokio::spawn(async move {
            let result = async { lyrics::load(&server?, &key).await.map_err(err) }.await;
            let _ = tx.send(Msg::Lyrics(key, result));
        });
    }

    pub(super) fn on_lyrics(&mut self, key: String, result: Result<Option<Lyrics>, String>) {
        if let Some((k, slot)) = &mut self.lyrics
            && *k == key
        {
            *slot = Some(result);
        }
    }

    /// The loaded lyrics of the playing track, if any.
    pub fn playing_lyrics(&self) -> Option<&Result<Option<Lyrics>, String>> {
        let np = self.now_playing.as_ref()?;
        let (k, slot) = self.lyrics.as_ref()?;
        (k == np.item.rating_key()).then_some(slot.as_ref()).flatten()
    }

    /// Playback position now, interpolated between the player's ticks.
    pub fn position_ms(&self) -> i64 {
        let Some(np) = &self.now_playing else { return 0 };
        if np.paused {
            return np.position_ms;
        }
        np.position_ms + self.np_at.elapsed().as_millis().min(1500) as i64
    }

    pub(super) fn lyrics_key(&mut self, mut manual: Option<(usize, Instant)>, key: KeyEvent) -> Option<Modal> {
        if self.transport_key(key) {
            return Some(Modal::Lyrics { manual });
        }
        let lines = match self.playing_lyrics() {
            Some(Ok(Some(l))) => Some(l.clone()),
            _ => None,
        };
        let n = lines.as_ref().map_or(0, |l| l.lines.len());
        let at = manual.filter(|(_, t)| t.elapsed() < LYRICS_HOLD).map(|(i, _)| i).or_else(|| lines.as_ref().and_then(|l| active_line(l, self.position_ms()))).unwrap_or(0);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down if n > 0 => manual = Some(((at + 1).min(n - 1), Instant::now())),
            KeyCode::Char('k') | KeyCode::Up if n > 0 => manual = Some((at.saturating_sub(1), Instant::now())),
            KeyCode::Enter => {
                // Seek to a synced line, as clicking one does on the GUI.
                if let Some(ms) = lines.as_ref().filter(|l| l.synced).and_then(|l| l.lines.get(at)).and_then(|l| l.time_ms) {
                    self.control(Control::SeekTo(ms as f64 / 1000.0));
                    manual = None;
                }
            }
            KeyCode::Char('E') => self.toggle_stop_after(),
            KeyCode::Esc | KeyCode::Char('q' | 'Y') => return None,
            _ => {}
        }
        Some(Modal::Lyrics { manual })
    }
}

/// What can go in the play queue: anything playable, or a container of it (not photos).
pub fn queueable(item: &Item) -> bool {
    item.rating_key.is_some()
        && (item.is_playable() || matches!(item.kind(), Kind::Show | Kind::Season | Kind::Artist | Kind::Album | Kind::Playlist | Kind::Collection))
        && !matches!(item.kind(), Kind::Photo | Kind::PhotoAlbum)
}

/// The `before` of a move one step down (or up) of entry `at`: the uid it
/// lands before, `None` for the end; no move at the edges.
pub fn move_before(uids: &[u64], at: usize, down: bool) -> Option<Option<u64>> {
    if down {
        (at + 1 < uids.len()).then(|| uids.get(at + 2).copied())
    } else {
        at.checked_sub(1).map(|i| Some(uids[i]))
    }
}

/// The last synced line that has started at `pos_ms`.
pub fn active_line(l: &Lyrics, pos_ms: i64) -> Option<usize> {
    if !l.synced {
        return None;
    }
    l.lines.iter().rposition(|line| line.time_ms.is_some_and(|t| t <= pos_ms))
}

/// The entry after the playing one.
pub fn up_next(np: &NowPlaying, q: Option<&QueueView>) -> Option<Metadata> {
    let q = q?;
    let i = q.items.iter().position(|e| e.uid == np.uid)?;
    q.items.get(i + 1).map(|e| e.item.clone())
}

/// Title and subtitle of a playing item, as the GUI's player shows them.
pub fn playing_lines(m: &Metadata) -> (String, String) {
    match m.kind() {
        "episode" => (m.grandparent_title.clone().unwrap_or_default(), format!("{} · {}", m.episode_code(), m.title())),
        "track" => (m.title().to_string(), m.grandparent_title.clone().unwrap_or_default()),
        _ => (m.title().to_string(), m.year.map(|y| y.to_string()).unwrap_or_default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use patina_core::lyrics::Line;

    #[test]
    fn moves_one_step() {
        let uids = [1, 2, 3, 4];
        assert_eq!(move_before(&uids, 0, true), Some(Some(3)));
        assert_eq!(move_before(&uids, 2, true), Some(None));
        assert_eq!(move_before(&uids, 3, true), None);
        assert_eq!(move_before(&uids, 2, false), Some(Some(2)));
        assert_eq!(move_before(&uids, 0, false), None);
    }

    #[test]
    fn follows_synced_lines() {
        let line = |t: Option<i64>| Line { time_ms: t, text: "x".into() };
        let l = Lyrics { synced: true, lines: vec![line(Some(0)), line(Some(5000)), line(None), line(Some(9000))] };
        assert_eq!(active_line(&l, 4999), Some(0));
        assert_eq!(active_line(&l, 5000), Some(1));
        assert_eq!(active_line(&l, 8000), Some(1));
        assert_eq!(active_line(&l, 60_000), Some(3));
        assert_eq!(active_line(&Lyrics { synced: false, ..l }, 60_000), None);
    }

    #[test]
    fn lines_like_the_gui() {
        let ep = Metadata { kind: Some("episode".into()), title: Some("Pilot".into()), grandparent_title: Some("Show".into()), index: Some(1), parent_index: Some(1), ..Default::default() };
        assert_eq!(playing_lines(&ep), ("Show".into(), "S01E01 · Pilot".into()));
        let t = Metadata { kind: Some("track".into()), title: Some("Song".into()), grandparent_title: Some("Band".into()), ..Default::default() };
        assert_eq!(playing_lines(&t), ("Song".into(), "Band".into()));
    }
}
