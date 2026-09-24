use super::*;
use patina_core::config::QUALITIES;
use patina_core::downloads::State as DownloadState;
use patina_core::player::{Control, PlayRequest};
use patina_plex::stream_type;

impl App {
    // ---- playback ----------------------------------------------------------

    /// Hand a request to the core player. Shows and seasons are fetched with
    /// their on-deck episode first, so playback continues where you left off.
    pub(super) fn start_play(&mut self, req: PlayRequest, queue_after: Vec<Metadata>) {
        self.info(format!("Starting {}…", Item::from(req.item.clone()).full_title()));
        self.queue_after = (!queue_after.is_empty()).then(|| (req.item.rating_key().to_string(), queue_after));
        let core = self.core.clone();
        self.spawn_op(Refresh::Nothing, async move {
            let mut req = req;
            if matches!(req.item.kind(), "show" | "season") && !req.shuffle && req.item.on_deck.is_none() {
                let opts = MetadataOptions { on_deck: true, ..Default::default() };
                let server = core.server().map_err(err)?;
                if let Some(full) = server.metadata(req.item.rating_key(), opts).await.map_err(err)? {
                    req.item = full;
                }
            }
            player::play(core, req);
            Ok(None)
        });
    }

    /// The tracks after the cursor, to queue behind the selected one.
    pub(super) fn tracks_after_cursor(&self) -> Vec<Metadata> {
        match self.columns.last() {
            Some(col) if self.search.is_none() => {
                col.queue_from_cursor().into_iter().skip(1).filter(|i| i.kind() == Kind::Track).take(MAX_QUEUE).map(|i| i.meta).collect()
            }
            _ => Vec::new(),
        }
    }

    pub(super) fn play_selected(&mut self, from_start: bool) {
        let Some(item) = self.selected().cloned() else { return };
        if !item.is_playable() {
            return self.play_container(item, false);
        }
        let after = if item.kind() == Kind::Track { self.tracks_after_cursor() } else { Vec::new() };
        self.start_play(PlayRequest { item: item.meta, resume: !from_start, ..Default::default() }, after);
    }

    /// Play every leaf of a container (from next-up for shows), optionally
    /// shuffled; on a playable item, play from there (shuffling its parent).
    pub(super) fn play_container(&mut self, item: Item, shuffle: bool) {
        if item.is_playable() {
            if !shuffle {
                return self.play_selected(false);
            }
            let parent = match item.kind() {
                Kind::Episode => item.parent_rating_key.clone().map(|k| (k, "season")),
                Kind::Track => item.parent_rating_key.clone().map(|k| (k, "album")),
                _ => None,
            };
            let item = match parent {
                Some((rk, kind)) => Metadata { rating_key: Some(rk), kind: Some(kind.into()), ..Default::default() },
                None => item.meta,
            };
            return self.start_play(PlayRequest { item, shuffle: true, ..Default::default() }, Vec::new());
        }
        if item.rating_key.is_none() || !matches!(item.kind(), Kind::Show | Kind::Season | Kind::Artist | Kind::Album | Kind::Playlist | Kind::Collection | Kind::PhotoAlbum) {
            self.info("Nothing to play here");
            return;
        }
        self.start_play(PlayRequest { item: item.meta, shuffle, ..Default::default() }, Vec::new());
    }

    pub(super) fn open_play_options(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        if !item.is_playable() {
            self.info("Play options are for a single movie, episode or track");
            return;
        }
        let Some(key) = item.rating_key.clone() else { return };
        let mut m = PlayOptionsModal {
            item: None,
            opts: PlayOptions::with_quality(self.core.settings().player.quality().id),
            cursor: 0,
            rows: Vec::new(),
        };
        build_option_rows(&mut m);
        self.modal = Some(Modal::PlayOptions(m));
        let (core, tx) = (self.core.clone(), self.tx.clone());
        tokio::spawn(async move {
            let r = async {
                let server = core.server().map_err(err)?;
                let m = server.metadata(&key, MetadataOptions::default()).await.map_err(err)?;
                m.map(|m| Box::new(Item::from(m))).ok_or_else(|| "Item not found".to_string())
            }
            .await;
            let _ = tx.send(Msg::PlayOptionsItem(r));
        });
    }

    /// Play with the dialog's choices. Track choices are saved to the server
    /// first: the player (and the transcoder) apply the server's selection.
    pub(super) fn play_with_options(&mut self, m: PlayOptionsModal) {
        let Some(item) = m.item else { return };
        let opts = m.opts;
        let part = item.media.get(opts.media_index).and_then(|md| md.parts.first()).and_then(|p| p.id);
        let choose = opts.audio_stream_id.is_some() || opts.subtitle_stream_id.is_some();
        let req = PlayRequest {
            item: item.meta.clone(),
            resume: !opts.from_start,
            media_index: opts.media_index,
            quality: Some(QUALITIES[opts.quality.min(QUALITIES.len() - 1)].id.to_string()),
            ..Default::default()
        };
        let core = self.core.clone();
        self.info(format!("Starting {}…", item.full_title()));
        self.spawn_op(Refresh::Nothing, async move {
            if choose && let Some(part) = part {
                let server = core.server().map_err(err)?;
                if let Err(e) = server.select_streams(part, opts.audio_stream_id, opts.subtitle_stream_id, true).await {
                    tracing::warn!("failed to save stream selection: {e}");
                }
            }
            player::play(core, req);
            Ok(None)
        });
    }

    pub(super) fn control(&mut self, cmd: Control) {
        if self.now_playing.is_none() {
            self.info("Nothing is playing");
            return;
        }
        player::control(cmd);
    }

    pub(super) fn download_selected(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        if !(item.is_playable() || matches!(item.kind(), Kind::Show | Kind::Season | Kind::Album | Kind::Artist)) {
            self.info("Select something to download");
            return;
        }
        let Some(key) = item.rating_key.clone() else { return };
        let (core, title) = (self.core.clone(), item.full_title());
        self.spawn_op(Refresh::Nothing, async move {
            let n = downloads::enqueue(&core, &key).await.map_err(err)?;
            Ok(Some(match n {
                0 => format!("{title} is already downloaded"),
                1 => format!("Downloading {title} to {}", core.settings().player.downloads_dir().display()),
                n => format!("Downloading {n} items of {title}"),
            }))
        });
    }

    pub(super) fn copy_url(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        let Some(key) = item.rating_key.clone().filter(|_| item.is_playable()) else {
            self.info("Select a movie, episode or track");
            return;
        };
        let server = self.server();
        self.spawn_op(Refresh::Nothing, async move {
            let server = server?;
            let full = server.metadata(&key, MetadataOptions::default()).await.map_err(err)?.ok_or("Item not found")?;
            let part = full.media.first().and_then(|m| m.parts.first()).and_then(|p| p.key.clone()).ok_or("no media file")?;
            let url = server.part_url(&part, false).map_err(err)?;
            copy_to_clipboard(url.as_str()).map_err(err)?;
            Ok(Some("Stream URL copied (it contains your token)".into()))
        });
    }
}

/// Lay out the play-options rows for the loaded item.
pub fn build_option_rows(m: &mut PlayOptionsModal) {
    let mut rows: Vec<(OptRow, String)> = Vec::new();
    if let Some(item) = &m.item {
        if item.media.len() > 1 {
            rows.push((OptRow::Header("Version"), String::new()));
            for (i, media) in item.media.iter().enumerate() {
                let label = [
                    crate::format::resolution(media.height, media.video_resolution.as_deref()),
                    media.video_codec.as_deref().map(crate::format::codec),
                    media.bitrate.map(crate::format::bitrate_kbps),
                    media.title.clone(),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · ");
                rows.push((OptRow::Version(i), label));
            }
        }
        let part = item.media.get(m.opts.media_index).and_then(|md| md.parts.first());
        let streams = part.map(|p| p.streams.as_slice()).unwrap_or(&[]);
        let audio: Vec<_> = streams.iter().filter(|s| s.stream_type == Some(stream_type::AUDIO)).collect();
        if audio.len() > 1 {
            rows.push((OptRow::Header("Audio"), String::new()));
            for s in audio {
                rows.push((OptRow::Audio(s.id.unwrap_or_default()), s.label()));
            }
        }
        let subs: Vec<_> = streams.iter().filter(|s| s.stream_type == Some(stream_type::SUBTITLE)).collect();
        if !subs.is_empty() {
            rows.push((OptRow::Header("Subtitles"), String::new()));
            rows.push((OptRow::Subtitle(None), "None".into()));
            for s in subs {
                rows.push((OptRow::Subtitle(s.id), s.label()));
            }
        }
        if matches!(item.kind(), Kind::Movie | Kind::Episode | Kind::Clip) {
            rows.push((OptRow::Header("Quality"), String::new()));
            for (i, q) in QUALITIES.iter().enumerate() {
                let label = if q.kbps.is_some() { format!("Transcode · {}", q.label) } else { q.label.to_string() };
                rows.push((OptRow::Quality(i), label));
            }
        }
        if let Some(offset) = item.view_offset.filter(|o| *o > 0) {
            rows.push((OptRow::Header("Start"), String::new()));
            rows.push((OptRow::Resume(false), format!("Resume from {}", crate::format::clock_ms(offset))));
            rows.push((OptRow::Resume(true), "From the beginning".into()));
        }
        rows.push((OptRow::Header(""), String::new()));
    }
    rows.push((OptRow::Play, "▶ Play".into()));
    m.cursor = rows.iter().position(|(r, _)| !matches!(r, OptRow::Header(_))).unwrap_or(0);
    m.rows = rows;
}

/// The running download, as `(title, received, size)`.
pub fn active_download(list: &[Download]) -> Option<(String, i64, i64)> {
    let d = list.iter().find(|d| d.state == DownloadState::Downloading)?;
    Some((Item::from(d.item.clone()).full_title(), d.received, d.size))
}

fn which(binary: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else { return false };
    std::env::split_paths(&path).any(|dir| dir.join(binary).is_file())
}

/// Copy via the platform clipboard tool, falling back to OSC 52 (which
/// most modern terminals, including over SSH, understand).
fn copy_to_clipboard(text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let tools: &[(&str, &[&str])] = &[
        ("pbcopy", &[]),
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
        ("clip.exe", &[]),
    ];
    for (cmd, args) in tools {
        if !which(cmd) {
            continue;
        }
        if let Ok(mut child) = Command::new(cmd).args(*args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes())?;
            }
            if child.wait().map(|s| s.success()).unwrap_or(false) {
                return Ok(());
            }
        }
    }
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    let mut out = std::io::stdout();
    write!(out, "\x1b]52;c;{encoded}\x07")?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_rows_for_a_movie() {
        let mut item = crate::item::test_item("movie", "Heat");
        item.view_offset = Some(60_000);
        let mut m = PlayOptionsModal { item: Some(item), opts: PlayOptions::with_quality("original"), cursor: 0, rows: Vec::new() };
        build_option_rows(&mut m);
        assert!(m.rows.iter().any(|(r, _)| *r == OptRow::Quality(0)));
        assert!(m.is_checked(&OptRow::Quality(0)));
        assert!(m.is_checked(&OptRow::Resume(false)));
        assert_eq!(m.rows.last().map(|r| &r.0), Some(&OptRow::Play));
        assert!(m.selectable(m.cursor));
    }
}
