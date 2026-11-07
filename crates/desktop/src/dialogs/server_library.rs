//! Add a library, or rename one and change its folders (`ServerLibraryDialog.svelte`),
//! and one library's advanced settings (`ServerLibraryPrefs.svelte`).

use crate::prelude::*;
use std::rc::Rc;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use patina_core::ops::server::{self as ops, Choices, LibraryForm, Pref};
use patina_plex::{Directory, Params};

use crate::core::{bg, core, message};
use crate::pages::server::{Res, Save, TextPrefs, pref_row};
use crate::store;
use crate::widgets;

const KINDS: [(&str, &str); 4] = [("movie", "Movies"), ("show", "TV Shows"), ("artist", "Music"), ("photo", "Photos")];

fn kind_label(kind: &str) -> &'static str {
    KINDS.iter().find(|(k, _)| *k == kind).map_or("Library", |(_, l)| l)
}

/// The non-empty, trimmed folder paths.
fn paths(locations: &[String]) -> Vec<String> {
    locations.iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()
}

fn label(text: &str) -> Div {
    div().text_sm().font_medium().child(text.to_string())
}

/// A select-like button with a menu of `(value, label)` choices.
fn select(id: &'static str, current: &str, options: Vec<(String, String)>, disabled: bool, pick: impl Fn(&mut Form, String, &mut Context<Form>) + 'static, cx: &mut Context<Form>) -> impl IntoElement {
    let shown = options.iter().find(|(v, _)| v == current).map_or(current.to_string(), |(_, l)| l.clone());
    let (weak, value, pick) = (cx.entity().downgrade(), current.to_string(), Rc::new(pick));
    Button::new(id)
        .outline()
        .w_full()
        .justify_between()
        .disabled(disabled)
        .child(div().flex_1().min_w_0().truncate().text_left().child(shown))
        .dropdown_caret(true)
        .dropdown_menu(move |mut menu, _, _| {
            for (v, l) in &options {
                let (v, weak, pick) = (v.clone(), weak.clone(), pick.clone());
                menu = menu.item(PopupMenuItem::new(l.clone()).checked(v == value).on_click(move |_, _, cx| {
                    weak.update(cx, |f, cx| pick(f, v.clone(), cx)).ok();
                }));
            }
            menu
        })
}

pub struct Form {
    section: Option<Directory>,
    /// The server this opened on; library ids and folders never go to another one.
    server: Option<String>,
    name: Entity<InputState>,
    language: Entity<InputState>,
    folders: Vec<Entity<InputState>>,
    kind: String,
    agent: String,
    scanner: String,
    choices: Option<Choices>,
    choices_error: Option<String>,
    busy: bool,
    subs: Vec<Subscription>,
}

impl Form {
    fn new(section: Option<Directory>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let server = store::store(cx).read(cx).session.as_ref().and_then(|s| s.machine_identifier.clone());
        let title = section.as_ref().and_then(|s| s.title.clone()).unwrap_or_default();
        let name = cx.new(|cx| InputState::new(window, cx).placeholder(if section.is_some() { "" } else { "Movies" }).default_value(title));
        let language = cx.new(|cx| InputState::new(window, cx).placeholder("en-US").default_value("en-US"));
        let mut this = Self {
            server,
            name: name.clone(),
            language,
            folders: Vec::new(),
            kind: "movie".into(),
            agent: String::new(),
            scanner: String::new(),
            choices: None,
            choices_error: None,
            busy: false,
            subs: Vec::new(),
            section: section.clone(),
        };
        this.subs.push(cx.subscribe_in(&name, window, |_, _, e: &InputEvent, _, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        }));
        let mut locations = section.map(|s| s.location.iter().filter_map(|l| l.path.clone()).filter(|p| !p.is_empty()).collect::<Vec<_>>()).unwrap_or_default();
        if locations.is_empty() {
            locations.push(String::new());
        }
        for l in locations {
            this.add_folder(l, window, cx);
        }
        name.update(cx, |s, cx| s.focus(window, cx));
        if this.section.is_none() {
            this.load_choices(cx);
        }
        this
    }

    fn add_folder(&mut self, value: String, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("/path/to/media").default_value(value));
        self.subs.push(cx.subscribe_in(&input, window, |this, _, e: &InputEvent, window, cx| match e {
            InputEvent::Change => cx.notify(),
            InputEvent::PressEnter { .. } => this.submit(window, cx),
            _ => {}
        }));
        self.folders.push(input);
    }

    /// Agents and scanners depend on the library type; preselect the first of each.
    fn load_choices(&mut self, cx: &mut Context<Self>) {
        self.choices = None;
        self.choices_error = None;
        let (app, kind) = (core(cx), self.kind.clone());
        cx.spawn(async move |this, cx| {
            let k = kind.clone();
            let result = bg(async move { ops::library_choices(&app, &k).await }).await;
            this.update(cx, |f, cx| {
                if f.kind != kind {
                    return;
                }
                match result {
                    Ok(c) => {
                        f.agent = c.agents.first().map(|a| a.0.clone()).unwrap_or_default();
                        f.scanner = c.scanners.first().cloned().unwrap_or_default();
                        f.choices = Some(c);
                    }
                    Err(e) => f.choices_error = Some(message(&e)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn locations(&self, cx: &App) -> Vec<String> {
        paths(&self.folders.iter().map(|f| f.read(cx).value().to_string()).collect::<Vec<_>>())
    }

    fn ready(&self, cx: &App) -> bool {
        !self.busy && !self.locations(cx).is_empty() && (self.section.is_some() || self.choices.is_some())
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ready(cx) {
            return;
        }
        let current = store::store(cx).read(cx).session.as_ref().and_then(|s| s.machine_identifier.clone());
        if current != self.server {
            window.close_dialog(cx);
            return;
        }
        let typed = self.name.read(cx).value().trim().to_string();
        let title = if typed.is_empty() { kind_label(&self.kind).to_string() } else { typed };
        let locations = self.locations(cx);
        let app = core(cx);
        self.busy = true;
        cx.notify();
        let (fut, ok): (std::pin::Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>, String) = match &self.section {
            Some(s) => {
                let id = s.key.clone().unwrap_or_default();
                let t = title.clone();
                (Box::pin(async move { ops::edit_library(&app, &id, &t, &locations).await }), "Library updated".into())
            }
            None => {
                let language = self.language.read(cx).value().trim().to_string();
                let form = LibraryForm {
                    name: title.clone(),
                    kind: self.kind.clone(),
                    agent: self.agent.clone(),
                    scanner: self.scanner.clone(),
                    language: if language.is_empty() { "en-US".into() } else { language },
                    locations,
                };
                (Box::pin(async move { ops::create_library(&app, &form).await }), format!("Added “{title}”"))
            }
        };
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let result = bg(fut).await;
            this.update(cx, |f, cx| {
                f.busy = false;
                match result {
                    Ok(()) => {
                        store::toast(cx, ok);
                        handle.update(cx, |_, window, cx| window.close_dialog(cx)).ok();
                    }
                    Err(e) => store::error(cx, message(&e)),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn footer(&self, cx: &mut Context<Self>) -> Div {
        h_flex()
            .w_full()
            .justify_end()
            .gap_2()
            .child(Button::new("library-cancel").ghost().label("Cancel").on_click(|_, window, cx| window.close_dialog(cx)))
            .child(
                Button::new("library-submit")
                    .primary()
                    .label(if self.section.is_some() { "Save" } else { "Add library" })
                    .disabled(!self.ready(cx))
                    .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
            )
    }
}

impl Render for Form {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let editing = self.section.is_some();
        let desc = if editing { "Rename the library or change the folders it scans." } else { "Point Plex at folders of media on the server." };
        let field = |l: &str, el: AnyElement| v_flex().gap_2().child(label(l)).child(el);
        let mut grid = div().grid().grid_cols(2).gap_x_4().gap_y_3p5().child(div().col_span(2).child(field("Name", Input::new(&self.name).into_any_element())));
        if !editing {
            let kinds = KINDS.iter().map(|(k, l)| (k.to_string(), l.to_string())).collect();
            let agents = self.choices.as_ref().map(|c| c.agents.iter().map(|(id, t)| (id.clone(), if t.is_empty() { id.clone() } else { t.clone() })).collect::<Vec<_>>()).unwrap_or_default();
            let scanners = self.choices.as_ref().map(|c| c.scanners.iter().map(|s| (s.clone(), s.clone())).collect::<Vec<_>>()).unwrap_or_default();
            let (no_agents, no_scanners) = (agents.is_empty(), scanners.is_empty());
            grid = grid
                .child(field(
                    "Type",
                    select(
                        "library-type",
                        &self.kind,
                        kinds,
                        false,
                        |f, v, cx| {
                            if f.kind != v {
                                f.kind = v;
                                f.load_choices(cx);
                                cx.notify();
                            }
                        },
                        cx,
                    )
                    .into_any_element(),
                ))
                .child(field(
                    "Agent",
                    select(
                        "library-agent",
                        &self.agent,
                        agents,
                        no_agents,
                        |f, v, cx| {
                            f.agent = v;
                            cx.notify();
                        },
                        cx,
                    )
                    .into_any_element(),
                ))
                .child(field(
                    "Scanner",
                    select(
                        "library-scanner",
                        &self.scanner,
                        scanners,
                        no_scanners,
                        |f, v, cx| {
                            f.scanner = v;
                            cx.notify();
                        },
                        cx,
                    )
                    .into_any_element(),
                ))
                .child(field("Language", Input::new(&self.language).into_any_element()))
                .when_some(self.choices_error.clone(), |g, e| {
                    g.child(div().col_span(2).px_4().py_3().rounded(px(8.)).border_1().border_color(theme.danger).text_sm().text_color(theme.danger).child(e))
                });
        }
        let only = self.folders.len() == 1;
        let folders = v_flex()
            .mt_5()
            .child(h_flex().mb_2().items_baseline().justify_between().gap_3().child(label("Folders")).child(div().text_xs().text_color(muted).child("Paths as the server sees them, e.g. /data/movies")))
            .children(self.folders.iter().enumerate().map(|(i, f)| {
                h_flex().mb_1p5().items_center().gap_1().child(div().flex_1().child(Input::new(f))).child(
                    widgets::icon_button(("remove-folder", i), IconName::X, "Remove folder").disabled(only).on_click(cx.listener(move |this, _, _, cx| {
                        if this.folders.len() > 1 {
                            this.folders.remove(i);
                            cx.notify();
                        }
                    })),
                )
            }))
            .child(h_flex().child(Button::new("add-folder").ghost().small().icon(IconName::Plus).label("Add folder").on_click(cx.listener(|this, _, window, cx| {
                this.add_folder(String::new(), window, cx);
                cx.notify();
            }))));
        v_flex()
            .child(div().mb_4().text_sm().text_color(muted).child(desc))
            .child(grid)
            .child(folders)
            .child(div().mt_6().child(self.footer(cx)))
    }
}

/// Add a library (`section` is `None`), or rename one and change its folders.
pub fn open(section: Option<Directory>, window: &mut Window, cx: &mut App) {
    let title: SharedString = match &section {
        Some(s) => format!("Edit {}", s.title.clone().unwrap_or_default()).into(),
        None => "Add library".into(),
    };
    let form = cx.new(|cx| Form::new(section, window, cx));
    window.open_dialog(cx, move |dialog, _, _| dialog.title(title.clone()).w(px(560.)).child(form.clone()));
}

// Library settings ------------------------------------------------------------------

pub struct Prefs {
    id: String,
    title: String,
    server: Option<String>,
    prefs: Res<Vec<Pref>>,
    advanced: bool,
    text: TextPrefs,
}

impl Prefs {
    fn new(section: &Directory, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            id: section.key.clone().unwrap_or_default(),
            title: section.title.clone().unwrap_or_default(),
            server: store::store(cx).read(cx).session.as_ref().and_then(|s| s.machine_identifier.clone()),
            prefs: Res::default(),
            advanced: false,
            text: TextPrefs::default(),
        };
        this.load(cx);
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.prefs.loading = true;
        self.prefs.error = None;
        let (app, id) = (core(cx), self.id.clone());
        cx.spawn(async move |this, cx| {
            let result = bg(async move { ops::library_prefs(&app, &id).await }).await;
            this.update(cx, |p, cx| {
                p.prefs.set(result);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn save(&mut self, pref: String, value: String, cx: &mut Context<Self>) {
        let current = store::store(cx).read(cx).session.as_ref().and_then(|s| s.machine_identifier.clone());
        if current != self.server {
            return;
        }
        let (app, id, weak) = (core(cx), self.id.clone(), cx.entity().downgrade());
        let (p, v) = (pref.clone(), value.clone());
        store::act_then(cx, async move { Ok::<_, anyhow::Error>(app.server()?.set_section_prefs(&id, &Params::new().set(&p, &v)).await?) }, move |_, cx| {
            store::toast(cx, "Setting saved");
            weak.update(cx, |s, cx| {
                if let Some(x) = s.prefs.data.iter_mut().flatten().find(|x| x.id == pref) {
                    x.value = value.clone();
                }
                cx.notify();
            })
            .ok();
        });
    }
}

impl Render for Prefs {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let all = self.prefs.data.clone();
        let has_advanced = all.iter().flatten().any(|p| p.advanced);
        let head = h_flex()
            .items_start()
            .justify_between()
            .gap_4()
            .child(v_flex().child(div().text_lg().font_semibold().child(format!("{} settings", self.title))).child(div().mt_1().text_sm().text_color(muted).child("Changes are saved as you make them.")))
            .when(has_advanced, |d| {
                d.child(div().pt_1().child(Switch::new("library-prefs-advanced").small().checked(self.advanced).label("Show advanced").on_click(cx.listener(|this, on: &bool, _, cx| {
                    this.advanced = *on;
                    cx.notify();
                }))))
            });
        let body = match all {
            None => match &self.prefs.error {
                Some(e) => widgets::failed(e.clone(), cx).child(Button::new("retry").outline().small().mt_3().label("Try again").on_click(cx.listener(|this, _, _, cx| this.load(cx)))).into_any_element(),
                None => widgets::loading().into_any_element(),
            },
            Some(prefs) => {
                let shown: Vec<Pref> = prefs.into_iter().filter(|p| self.advanced || !p.advanced).collect();
                if shown.is_empty() {
                    widgets::empty_box(IconName::Settings2, Some("No settings".into()), Some("This library has no settings.".into()), cx).into_any_element()
                } else {
                    let rows: Vec<AnyElement> = shown
                        .iter()
                        .enumerate()
                        .map(|(i, p)| {
                            let input = (p.kind != "bool" && p.options.is_empty()).then(|| self.text.input(p, window, cx, |s, id, value, cx| s.save(id, value, cx)));
                            let (id, weak) = (p.id.clone(), cx.entity().downgrade());
                            let save: Save = Rc::new(move |value, cx| {
                                weak.update(cx, |s, cx| s.save(id.clone(), value, cx)).ok();
                            });
                            pref_row(p, false, i == 0, input, save, cx)
                        })
                        .collect();
                    div()
                        .id("library-prefs")
                        .mt_4()
                        .max_h(window.viewport_size().height * 0.6)
                        .overflow_y_scroll()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(theme.border)
                        .children(rows)
                        .into_any_element()
                }
            }
        };
        v_flex()
            .gap_4()
            .child(head)
            .child(body)
            .child(h_flex().justify_end().child(Button::new("library-prefs-done").ghost().label("Done").on_click(|_, window, cx| window.close_dialog(cx))))
    }
}

/// One library's advanced settings, saved as they change.
pub fn open_prefs(section: Directory, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| Prefs::new(&section, cx));
    window.open_dialog(cx, move |dialog, window, _| {
        let w = (window.viewport_size().width - px(48.)).min(px(760.));
        dialog.w(w).child(view.clone())
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn folders_and_kinds() {
        assert_eq!(paths(&["  /data/movies ".into(), "".into(), "   ".into(), "/b".into()]), ["/data/movies", "/b"]);
        assert_eq!(kind_label("show"), "TV Shows");
        assert_eq!(kind_label("other"), "Library");
    }
}
