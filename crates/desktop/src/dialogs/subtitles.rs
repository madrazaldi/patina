//! Online subtitle search for a movie or episode.

use crate::prelude::*;
use gpui_kit::component::spinner::Spinner;
use patina_plex::{Metadata, Stream};

use super::form::{self, Guard, Modal};
use crate::core::{Load, bg, core};
use crate::store::store;

/// [ISO 639-1, ISO 639-2 (as Plex reports stream languages), name]
const LANGUAGES: &[(&str, &str, &str)] = &[
    ("ar", "ara", "Arabic"),
    ("bg", "bul", "Bulgarian"),
    ("zh", "chi", "Chinese"),
    ("hr", "hrv", "Croatian"),
    ("cs", "cze", "Czech"),
    ("da", "dan", "Danish"),
    ("nl", "dut", "Dutch"),
    ("en", "eng", "English"),
    ("et", "est", "Estonian"),
    ("fi", "fin", "Finnish"),
    ("fr", "fre", "French"),
    ("de", "ger", "German"),
    ("el", "gre", "Greek"),
    ("he", "heb", "Hebrew"),
    ("hi", "hin", "Hindi"),
    ("hu", "hun", "Hungarian"),
    ("is", "ice", "Icelandic"),
    ("id", "ind", "Indonesian"),
    ("it", "ita", "Italian"),
    ("ja", "jpn", "Japanese"),
    ("ko", "kor", "Korean"),
    ("lv", "lav", "Latvian"),
    ("lt", "lit", "Lithuanian"),
    ("ms", "may", "Malay"),
    ("no", "nor", "Norwegian"),
    ("fa", "per", "Persian"),
    ("pl", "pol", "Polish"),
    ("pt", "por", "Portuguese"),
    ("ro", "rum", "Romanian"),
    ("ru", "rus", "Russian"),
    ("sr", "srp", "Serbian"),
    ("sk", "slo", "Slovak"),
    ("sl", "slv", "Slovenian"),
    ("es", "spa", "Spanish"),
    ("sv", "swe", "Swedish"),
    ("th", "tha", "Thai"),
    ("tr", "tur", "Turkish"),
    ("uk", "ukr", "Ukrainian"),
    ("vi", "vie", "Vietnamese"),
];

/// ISO 639-2/T variants Plex sometimes uses.
const ALIASES: &[(&str, &str)] = &[
    ("zho", "zh"),
    ("ces", "cs"),
    ("nld", "nl"),
    ("fra", "fr"),
    ("deu", "de"),
    ("ell", "el"),
    ("isl", "is"),
    ("msa", "ms"),
    ("fas", "fa"),
    ("ron", "ro"),
    ("slk", "sk"),
];

fn to_iso1(code: Option<&str>) -> Option<&'static str> {
    let c = code?.to_lowercase();
    LANGUAGES.iter().find(|(a, b, _)| *a == c || *b == c).map(|(a, _, _)| *a).or_else(|| ALIASES.iter().find(|(k, _)| *k == c).map(|(_, v)| *v))
}

/// The language to search first: the chosen subtitle's, else the audio's, else English.
fn default_language(m: &Metadata) -> &'static str {
    let Some(part) = m.media.first().and_then(|m| m.parts.first()) else { return "en" };
    to_iso1(part.selected_subtitle().and_then(|s| s.language_code.as_deref()))
        .or_else(|| to_iso1(part.selected_audio().and_then(|s| s.language_code.as_deref())))
        .unwrap_or("en")
}

pub struct SubtitleDialog {
    item: Metadata,
    guard: Guard,
    language: String,
    hearing_impaired: bool,
    forced: bool,
    results: Load<Vec<Stream>>,
    generation: u64,
    /// Key of the subtitle being downloaded.
    busy: Option<String>,
}

pub fn open(item: Metadata, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| SubtitleDialog::new(item, cx));
    form::show(view, "Search subtitles", form::WIDE, window, cx);
}

impl Modal for SubtitleDialog {
    fn busy(&self) -> bool {
        self.busy.is_some()
    }
}

impl SubtitleDialog {
    fn new(item: Metadata, cx: &mut Context<Self>) -> Self {
        let language = default_language(&item).to_string();
        let mut this = Self { item, guard: Guard::new(cx), language, hearing_impaired: false, forced: false, results: Load::Loading, generation: 0, busy: None };
        this.search(cx);
        this
    }

    fn search(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let run = self.generation;
        self.results = Load::Loading;
        cx.notify();
        let (app, rk, asked) = (core(cx), self.item.rating_key().to_string(), self.guard.clone());
        let (language, hi, forced) = (self.language.clone(), self.hearing_impaired, self.forced);
        cx.spawn(async move |this, cx| {
            let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.search_subtitles(&rk, &language, hi, forced).await?) }).await;
            this.update(cx, |d, cx| {
                if d.generation == run && asked.valid(cx) {
                    d.results = Load::from_result(result);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn fetch(&mut self, key: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy.is_some() {
            return;
        }
        if !self.guard.valid(cx) {
            window.close_dialog(cx);
            return;
        }
        let (app, rk) = (core(cx), self.item.rating_key().to_string());
        self.busy = Some(key.clone());
        cx.notify();
        let fut = async move { Ok::<_, anyhow::Error>(app.server()?.download_subtitle(&rk, &key).await?) };
        form::run(window, cx, fut, Some("Subtitle downloaded".into()), true, |d, ok, window, cx| {
            d.busy = None;
            if ok {
                window.close_dialog(cx);
            }
            cx.notify();
        });
    }
}

impl Render for SubtitleDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let tint = store(cx).read(cx).tint();
        let busy = self.busy.clone();
        let languages: Vec<(String, String)> = LANGUAGES.iter().map(|(c, _, n)| (c.to_string(), n.to_string())).collect();
        let me = cx.entity().downgrade();
        let body = match &self.results {
            Load::Failed(e) => form::error(e.clone(), cx).into_any_element(),
            Load::Loading => form::waiting("Searching subtitle providers…", cx).into_any_element(),
            Load::Ready(l) if l.is_empty() => form::empty("No subtitles", "Nothing found for this language.", cx).into_any_element(),
            Load::Ready(l) => crate::widgets::rows(cx)
                .id("subtitles")
                .max_h(px(420.))
                .overflow_y_scroll()
                .children(l.iter().enumerate().map(|(i, s)| {
                    let name = s.display_title.clone().or_else(|| s.title.clone()).unwrap_or_else(|| "Subtitle".into());
                    let detail = [s.provider_title.clone(), s.title.clone().filter(|t| Some(t) != s.display_title.as_ref())].into_iter().flatten().collect::<Vec<_>>().join(" · ");
                    let key = s.key.clone();
                    let waiting = busy.is_some() && busy == key;
                    h_flex()
                        .gap_3()
                        .px_3()
                        .py_2()
                        .items_center()
                        .when(i > 0, |d| d.border_t_1().border_color(theme.border))
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().text_sm().font_medium().truncate().child(name))
                                .child(div().text_xs().truncate().text_color(theme.muted_foreground).child(detail)),
                        )
                        .child(
                            h_flex()
                                .flex_none()
                                .gap_2()
                                .items_center()
                                .when(s.perfect_match, |d| d.child(form::badge("Perfect match", Some(tint), cx)))
                                .when(s.hearing_impaired, |d| d.child(form::badge("SDH", None, cx)))
                                .when(s.forced, |d| d.child(form::badge("Forced", None, cx)))
                                .when_some(s.score, |d, n| d.child(div().min_w(px(26.)).text_right().text_xs().text_color(theme.muted_foreground).child(n.to_string())))
                                .map(|d| {
                                    if s.downloaded {
                                        d.child(div().text_xs().text_color(theme.muted_foreground).child("Downloaded"))
                                    } else {
                                        let k = key.clone();
                                        d.child(
                                            Button::new(("download", i))
                                                .outline()
                                                .small()
                                                .icon(if waiting { Icon::new(IconName::Loader) } else { Icon::new(IconName::Download) })
                                                .label("Download")
                                                .disabled(key.is_none() || busy.is_some())
                                                .on_click(cx.listener(move |this, _, window, cx| {
                                                    if let Some(k) = k.clone() {
                                                        this.fetch(k, window, cx);
                                                    }
                                                })),
                                        )
                                    }
                                })
                                .when(waiting, |d| d.child(Spinner::new().small())),
                        )
                }))
                .into_any_element(),
        };
        v_flex()
            .child(form::desc(self.item.full_title(), cx))
            .child(
                h_flex()
                    .mb_4()
                    .gap_2()
                    .items_center()
                    .child(div().mr_1().min_w(px(160.)).child({
                        let me = me.clone();
                        form::select("language", languages, &self.language, false, move |v, _, cx| {
                            me.update(cx, |d, cx| {
                                d.language = v;
                                d.search(cx);
                            })
                            .ok();
                        })
                    }))
                    .child(form::chip("hi", "Hearing impaired", self.hearing_impaired).on_click(cx.listener(|this, _, _, cx| {
                        this.hearing_impaired = !this.hearing_impaired;
                        this.search(cx);
                    })))
                    .child(form::chip("forced", "Forced only", self.forced).on_click(cx.listener(|this, _, _, cx| {
                        this.forced = !this.forced;
                        this.search(cx);
                    }))),
            )
            .child(body)
            .child(form::foot(cx).child(Button::new("close").outline().label("Close").disabled(self.busy.is_some()).on_click(|_, window, cx| window.close_dialog(cx))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn languages_map_to_iso1() {
        assert_eq!(to_iso1(Some("eng")), Some("en"));
        assert_eq!(to_iso1(Some("JA")), Some("ja"));
        assert_eq!(to_iso1(Some("deu")), Some("de"));
        assert_eq!(to_iso1(Some("xx")), None);
        assert_eq!(to_iso1(None), None);
    }

    #[test]
    fn default_language_prefers_subtitles_then_audio() {
        let m: Metadata = serde_json::from_str(
            r#"{"Media":[{"Part":[{"Stream":[
                {"id":1,"streamType":2,"languageCode":"jpn","selected":true},
                {"id":2,"streamType":3,"languageCode":"fre","selected":true}]}]}]}"#,
        )
        .unwrap();
        assert_eq!(default_language(&m), "fr");
        let m: Metadata = serde_json::from_str(r#"{"Media":[{"Part":[{"Stream":[{"id":1,"streamType":2,"languageCode":"jpn"}]}]}]}"#).unwrap();
        assert_eq!(default_language(&m), "ja");
        assert_eq!(default_language(&Metadata::default()), "en");
    }
}
