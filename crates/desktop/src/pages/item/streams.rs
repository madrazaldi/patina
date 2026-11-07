//! Version / audio / subtitle choosers. Changes are saved to Plex at once, and
//! playback follows them.

use crate::prelude::*;
use patina_core::servers;
use patina_plex::Metadata;

use crate::core::core;
use crate::dialogs::{self, form};
use crate::store::{self, store};
use crate::widgets;

/// A stream's label in a picker.
fn stream_label(s: &patina_plex::Stream) -> String {
    s.label()
}

/// Save the chosen audio and/or subtitle stream (0 turns subtitles off).
fn select(item: &Metadata, part: i64, audio: Option<i64>, subtitle: Option<i64>, cx: &mut App) {
    let app = core(cx);
    let foreign = item.server_id().map(str::to_string);
    store::act(
        cx,
        async move { Ok::<_, anyhow::Error>(servers::server_for(&app, foreign.as_deref()).await?.select_streams(part, audio, subtitle, true).await?) },
        None,
        true,
    );
}

fn remove_subtitle(id: i64, label: String, window: &mut Window, cx: &mut App) {
    widgets::confirm("Delete this subtitle?", Some(format!("{label} is removed from the server.").into()), "Delete", true, window, cx, move |_, cx| {
        let app = core(cx);
        store::act(cx, async move { Ok::<_, anyhow::Error>(app.server()?.delete_stream(id).await?) }, Some("Subtitle deleted".into()), true);
    });
}

/// The pickers for `item`; `on_version` changes the version the page plays.
pub fn pickers(item: &Metadata, media_index: usize, on_version: impl Fn(usize, &mut App) + 'static, cx: &App) -> Option<AnyElement> {
    let media = item.media.get(media_index).or(item.media.first())?;
    let part = media.parts.first()?;
    let part_id = part.id;
    // An item found on another server: its tracks are chosen there, and owner rights don't apply.
    let owner = store(cx).read(cx).owned() && item.server_id().is_none();
    let kind = item.kind();
    let audio: Vec<_> = part.audio_streams().collect();
    let subs: Vec<_> = part.subtitle_streams().collect();
    let selected_audio = audio.iter().find(|s| s.selected).or(audio.first()).and_then(|s| s.id).unwrap_or_default();
    let selected_sub = subs.iter().find(|s| s.selected).and_then(|s| s.id).unwrap_or(0);
    let external = subs.iter().find(|s| s.selected && s.is_external()).map(|s| (s.id, stream_label(s)));
    let picker = |label: AnyElement, control: AnyElement| v_flex().min_w(px(200.)).max_w(px(340.)).flex_1().gap_1p5().child(label).child(control);
    let label = |text: &str| form::label(text, cx).into_any_element();

    let mut row = h_flex().mt_6().flex_wrap().items_end().gap_4();
    if item.media.len() > 1 {
        let options: Vec<(String, String)> = item.media.iter().enumerate().map(|(i, m)| {
            let l = m.label();
            (i.to_string(), if l.is_empty() { format!("Version {}", i + 1) } else { l })
        }).collect();
        row = row.child(picker(
            label("Version"),
            form::select("pick-version", options, &media_index.to_string(), false, move |v, _, cx| {
                if let Ok(i) = v.parse() {
                    on_version(i, cx);
                }
            })
            .into_any_element(),
        ));
    }
    if !audio.is_empty() {
        let options = audio.iter().map(|s| (s.id.unwrap_or_default().to_string(), stream_label(s))).collect();
        let it = item.clone();
        row = row.child(picker(
            label("Audio"),
            form::select("pick-audio", options, &selected_audio.to_string(), false, move |v, _, cx| {
                if let (Some(part), Ok(id)) = (part_id, v.parse()) {
                    select(&it, part, Some(id), None, cx);
                }
            })
            .into_any_element(),
        ));
    }
    if kind != "track" {
        let mut options = vec![("0".to_string(), "None".to_string())];
        options.extend(subs.iter().map(|s| (s.id.unwrap_or_default().to_string(), format!("{}{}", stream_label(s), if s.is_external() { " · external" } else { "" }))));
        let it = item.clone();
        let search = item.clone();
        let fg = cx.theme().foreground;
        let head = h_flex()
            .justify_between()
            .gap_3()
            .child(form::label("Subtitles", cx))
            .when(owner && matches!(kind, "movie" | "episode"), |d| {
                d.child(
                    div()
                        .id("search-subtitles")
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .cursor_pointer()
                        .hover(move |s| s.text_color(fg))
                        .child("Search…")
                        .on_click(move |_, window, cx| dialogs::subtitles::open(search.clone(), window, cx)),
                )
            });
        let control = h_flex()
            .gap_1()
            .items_center()
            .child(div().flex_1().min_w_0().child(form::select("pick-subtitles", options, &selected_sub.to_string(), false, move |v, _, cx| {
                if let (Some(part), Ok(id)) = (part_id, v.parse()) {
                    select(&it, part, None, Some(id), cx);
                }
            })))
            .when_some(external.filter(|_| owner).and_then(|(id, l)| Some((id?, l))), |d, (id, l)| {
                d.child(widgets::icon_button("delete-subtitle", IconName::Trash, "Delete this subtitle file").on_click(move |_, window, cx| remove_subtitle(id, l.clone(), window, cx)))
            });
        row = row.child(picker(head.into_any_element(), control.into_any_element()));
    }
    Some(row.into_any_element())
}
