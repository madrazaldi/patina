//! The metadata editor. Only changed fields are sent, and they are locked so a
//! metadata refresh keeps them.

use crate::prelude::*;
use std::collections::HashMap;

use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use patina_core::ops::edit::{self, TagEdit};
use patina_plex::{Metadata, MetadataOptions, Tag};

use super::form::{self, Guard, Modal};
use crate::core::{Load, bg, core};

/// Every editable field: the Plex field name and its label.
const LABELS: &[(&str, &str)] = &[
    ("title", "Title"),
    ("titleSort", "Sort title"),
    ("originalTitle", "Original title"),
    ("editionTitle", "Edition"),
    ("year", "Year"),
    ("originallyAvailableAt", "Release date"),
    ("contentRating", "Content rating"),
    ("studio", "Studio"),
    ("tagline", "Tagline"),
    ("summary", "Summary"),
];

/// The fields of each kind, in form order.
fn fields_of(kind: &str) -> &'static [&'static str] {
    match kind {
        "movie" => &["title", "titleSort", "originalTitle", "editionTitle", "year", "originallyAvailableAt", "contentRating", "studio", "tagline", "summary"],
        "show" => &["title", "titleSort", "originalTitle", "year", "originallyAvailableAt", "contentRating", "studio", "tagline", "summary"],
        "season" => &["title", "summary"],
        "episode" => &["title", "titleSort", "originallyAvailableAt", "summary"],
        "artist" => &["title", "titleSort", "summary"],
        "album" => &["title", "titleSort", "year", "originallyAvailableAt", "studio", "summary"],
        "track" => &["title", "titleSort", "originalTitle"],
        "collection" => &["title", "titleSort", "contentRating", "summary"],
        _ => &["title", "titleSort", "summary"],
    }
}

/// The tag lists of each kind.
fn tags_of(kind: &str) -> &'static [&'static str] {
    match kind {
        "movie" | "show" | "artist" | "album" => &["genre", "label"],
        "collection" => &["label"],
        _ => &[],
    }
}

fn label(kind: &str, field: &str) -> &'static str {
    match (kind, field) {
        ("album", "studio") => "Record label",
        ("track", "originalTitle") => "Track artist",
        (_, "genre") => "Genres",
        (_, "label") => "Labels",
        _ => LABELS.iter().find(|(f, _)| *f == field).map_or("", |(_, l)| l),
    }
}

fn tag_list(list: &[Tag]) -> String {
    list.iter().filter_map(|t| t.tag.clone()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join(", ")
}

/// The current value of every field and tag list.
fn values_of(m: &Metadata) -> HashMap<String, String> {
    let s = |v: &Option<String>| v.clone().unwrap_or_default();
    let mut v = HashMap::new();
    v.insert("title".into(), s(&m.title));
    v.insert("titleSort".into(), s(&m.title_sort));
    v.insert("originalTitle".into(), s(&m.original_title));
    v.insert("editionTitle".into(), s(&m.edition_title));
    v.insert("year".into(), m.year.map(|y| y.to_string()).unwrap_or_default());
    v.insert("originallyAvailableAt".into(), s(&m.originally_available_at));
    v.insert("contentRating".into(), s(&m.content_rating));
    v.insert("studio".into(), s(&m.studio));
    v.insert("tagline".into(), s(&m.tagline));
    v.insert("summary".into(), s(&m.summary));
    v.insert("genre".into(), tag_list(&m.genres));
    v.insert("label".into(), tag_list(&m.labels));
    v
}

/// "a, b, a" → ["a", "b"].
fn split(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in s.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        if !out.iter().any(|x| x == t) {
            out.push(t.to_string());
        }
    }
    out
}

/// The fields that differ from the original, and the tags added and removed.
fn changes(fields: &[&str], tags: &[&str], original: &HashMap<String, String>, form: &HashMap<String, String>) -> (Vec<(String, String)>, Vec<TagEdit>) {
    let get = |m: &HashMap<String, String>, k: &str| m.get(k).cloned().unwrap_or_default();
    let out = fields
        .iter()
        .filter(|f| get(form, f).trim() != get(original, f))
        .map(|f| (f.to_string(), get(form, f).trim().to_string()))
        .collect();
    let has = |list: &[String], t: &str| list.iter().any(|x| x.eq_ignore_ascii_case(t));
    let mut edits = Vec::new();
    for tag in tags {
        let before = split(&get(original, tag));
        let after = split(&get(form, tag));
        let add: Vec<String> = after.iter().filter(|t| !has(&before, t)).cloned().collect();
        let remove: Vec<String> = before.iter().filter(|t| !has(&after, t)).cloned().collect();
        if !add.is_empty() || !remove.is_empty() {
            edits.push(TagEdit { tag: tag.to_string(), add, remove });
        }
    }
    (out, edits)
}

/// Why the form can't be saved, if it can't.
fn invalid(fields: &[&str], form: &HashMap<String, String>) -> Option<&'static str> {
    let get = |k: &str| form.get(k).map(|s| s.trim().to_string()).unwrap_or_default();
    let year = get("year");
    if fields.contains(&"year") && !year.is_empty() && !(year.len() == 4 && year.chars().all(|c| c.is_ascii_digit())) {
        return Some("Year should be four digits.");
    }
    let date = get("originallyAvailableAt");
    let is_date = |s: &str| {
        let b = s.as_bytes();
        b.len() == 10 && b[4] == b'-' && b[7] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    };
    if fields.contains(&"originallyAvailableAt") && !date.is_empty() && !is_date(&date) {
        return Some("Release date should be YYYY-MM-DD.");
    }
    None
}

/// The loaded item and an input per field.
struct Form {
    item: Metadata,
    original: HashMap<String, String>,
    inputs: Vec<(&'static str, Entity<InputState>)>,
    summary: Option<Entity<TextareaState>>,
}

pub struct EditDialog {
    item: Metadata,
    guard: Guard,
    form: Load<Form>,
    busy: bool,
    _subs: Vec<Subscription>,
}

pub fn open(item: Metadata, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| EditDialog::new(item, window, cx));
    form::show(view, "Edit", form::WIDE, window, cx);
}

impl Modal for EditDialog {
    fn busy(&self) -> bool {
        self.busy
    }
}

impl EditDialog {
    fn new(item: Metadata, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let app = core(cx);
        let rk = item.rating_key().to_string();
        // Menu items may be partial, so edit the full item.
        cx.spawn_in(window, async move |this, cx| {
            let result = bg(async move { Ok::<_, anyhow::Error>(app.server()?.metadata(&rk, MetadataOptions::full()).await?) }).await;
            this.update_in(cx, |d, window, cx| {
                if !d.guard.valid(cx) {
                    return;
                }
                d.form = match result {
                    Ok(Some(m)) => Load::Ready(d.build(m, window, cx)),
                    Ok(None) => Load::Failed("This item no longer exists on the server.".into()),
                    Err(e) => Load::Failed(crate::core::message(&e)),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
        Self { item, guard: Guard::new(cx), form: Load::Loading, busy: false, _subs: Vec::new() }
    }

    fn build(&mut self, m: Metadata, window: &mut Window, cx: &mut Context<Self>) -> Form {
        let kind = m.kind().to_string();
        let original = values_of(&m);
        let mut inputs = Vec::new();
        for f in fields_of(&kind).iter().chain(tags_of(&kind)).filter(|f| **f != "summary") {
            let value = original.get(*f).cloned().unwrap_or_default();
            let placeholder = match *f {
                "titleSort" => original.get("title").cloned().unwrap_or_default(),
                "genre" | "label" => "Comma-separated".into(),
                "originallyAvailableAt" => "YYYY-MM-DD".into(),
                _ => String::new(),
            };
            let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value));
            self._subs.push(cx.subscribe_in(&state, window, |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::PressEnter { .. } => this.save(window, cx),
                InputEvent::Change => cx.notify(),
                _ => {}
            }));
            inputs.push((*f, state));
        }
        let summary = fields_of(&kind).contains(&"summary").then(|| {
            let value = original.get("summary").cloned().unwrap_or_default();
            let state = cx.new(|cx| TextareaState::new(window, cx).auto_grow(6, 14).default_value(value));
            self._subs.push(cx.observe(&state, |_, _, cx| cx.notify()));
            state
        });
        if let Some((_, first)) = inputs.first() {
            first.update(cx, |s, cx| s.focus(window, cx));
        }
        Form { item: m, original, inputs, summary }
    }

    fn values(form: &Form, cx: &App) -> HashMap<String, String> {
        let mut v: HashMap<String, String> = form.inputs.iter().map(|(f, s)| (f.to_string(), s.read(cx).value().to_string())).collect();
        if let Some(s) = &form.summary {
            v.insert("summary".into(), s.read(cx).value().to_string());
        }
        v
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = self.form.ready() else { return };
        if self.busy {
            return;
        }
        if !self.guard.valid(cx) {
            window.close_dialog(cx);
            return;
        }
        let kind = form.item.kind().to_string();
        let values = Self::values(form, cx);
        if invalid(fields_of(&kind), &values).is_some() {
            return;
        }
        let (fields, tags) = changes(fields_of(&kind), tags_of(&kind), &form.original, &values);
        if fields.is_empty() && tags.is_empty() {
            return;
        }
        let rk = form.item.rating_key().to_string();
        let app = core(cx);
        self.busy = true;
        cx.notify();
        form::run(window, cx, async move { edit::edit_metadata(&app, &rk, &fields, &tags).await }, Some("Saved".into()), true, |d, ok, window, cx| {
            d.busy = false;
            if ok && d.guard.valid(cx) {
                window.close_dialog(cx);
            }
            cx.notify();
        });
    }
}

impl Render for EditDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let busy = self.busy;
        let title = self.form.ready().map_or_else(|| self.item.full_title(), |f| f.item.full_title());
        let mut problem = None;
        let mut dirty = false;
        let body = match &self.form {
            Load::Failed(e) => form::error(e.clone(), cx).into_any_element(),
            Load::Loading => form::spinner().into_any_element(),
            Load::Ready(f) => {
                let kind = f.item.kind().to_string();
                let values = Self::values(f, cx);
                problem = invalid(fields_of(&kind), &values);
                let (a, b) = changes(fields_of(&kind), tags_of(&kind), &f.original, &values);
                dirty = !a.is_empty() || !b.is_empty();
                // Title and tagline take a whole row; the others go two to a row.
                let mut rows: Vec<Vec<(&'static str, Entity<InputState>)>> = Vec::new();
                for (name, state) in &f.inputs {
                    let wide = matches!(*name, "title" | "tagline");
                    match rows.last_mut() {
                        Some(last) if !wide && last.len() == 1 && !matches!(last[0].0, "title" | "tagline") => last.push((name, state.clone())),
                        _ => rows.push(vec![(name, state.clone())]),
                    }
                }
                v_flex()
                    .gap_4()
                    .children(rows.into_iter().map(|row| {
                        h_flex().gap_3p5().items_start().children(row.into_iter().map(|(name, state)| {
                            div().flex_1().min_w_0().child(form::field(label(&kind, name), Input::new(&state).disabled(busy), cx))
                        }))
                    }))
                    .when_some(f.summary.clone(), |d, s| d.child(form::field("Summary", Textarea::new(&s).disabled(busy), cx)))
                    .into_any_element()
            }
        };
        v_flex()
            .child(form::desc(title, cx))
            .child(body)
            .child(
                form::foot(cx)
                    .child(
                        div()
                            .mr_auto()
                            .text_xs()
                            .text_color(if problem.is_some() { theme.danger } else { theme.muted_foreground })
                            .child(problem.unwrap_or("Edited fields are locked, so refreshing metadata won't overwrite them.")),
                    )
                    .child(Button::new("cancel").outline().label("Cancel").disabled(busy).on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(
                        Button::new("save")
                            .primary()
                            .label(if busy { "Saving…" } else { "Save" })
                            .loading(busy)
                            .disabled(!dirty || problem.is_some() || busy)
                            .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn only_changed_fields_and_tags_are_sent() {
        let original = map(&[("title", "Copper Sky"), ("year", "2015"), ("genre", "Drama, Mystery")]);
        let form = map(&[("title", " Copper Sky "), ("year", "2016"), ("genre", "drama, Thriller, Thriller")]);
        let (fields, tags) = changes(&["title", "year"], &["genre"], &original, &form);
        assert_eq!(fields, vec![("year".to_string(), "2016".to_string())]);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].add, vec!["Thriller".to_string()]);
        assert_eq!(tags[0].remove, vec!["Mystery".to_string()]);
    }

    #[test]
    fn validates_year_and_date() {
        let fields = ["year", "originallyAvailableAt"];
        assert_eq!(invalid(&fields, &map(&[("year", "15")])), Some("Year should be four digits."));
        assert_eq!(invalid(&fields, &map(&[("originallyAvailableAt", "2015-5-1")])), Some("Release date should be YYYY-MM-DD."));
        assert_eq!(invalid(&fields, &map(&[("year", "2015"), ("originallyAvailableAt", "2015-05-01")])), None);
        assert_eq!(invalid(&["title"], &map(&[("year", "x")])), None);
    }

    #[test]
    fn labels_follow_the_kind() {
        assert_eq!(label("album", "studio"), "Record label");
        assert_eq!(label("track", "originalTitle"), "Track artist");
        assert_eq!(label("movie", "studio"), "Studio");
        assert_eq!(split("a, b,, a "), vec!["a", "b"]);
    }
}
