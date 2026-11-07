//! The command palette (⌘K): fuzzy-filtered navigation, playback, saved views,
//! servers and settings. Every open starts fresh.

use crate::prelude::*;
use std::rc::Rc;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use patina_core::player::{self, Control};
use patina_core::{session, undo};
use patina_plex::Metadata;

use crate::core::{bg, core};
use crate::nav::{self, Route, nav};
use crate::pages::library::{Library, SavedView};
use crate::store::{self, Panel, store};
use crate::{actions, pages};

type Run = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone)]
struct Command {
    id: String,
    label: String,
    group: &'static str,
    /// Extra words the filter also looks at.
    keywords: &'static str,
    icon: Option<IconName>,
    run: Run,
}

/// Open the command palette.
pub fn open(window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| Palette::new(window, cx));
    let input = view.read(cx).input.clone();
    window.open_dialog(cx, move |dialog, _, _| dialog.w(px(600.)).p_0().close_button(false).child(view.clone()));
    window.defer(cx, move |window, cx| input.update(cx, |s, cx| s.focus(window, cx)));
}


pub struct Palette {
    input: Entity<InputState>,
    active: usize,
    playlists: Vec<Metadata>,
    scroll: ScrollHandle,
    _subs: Vec<Subscription>,
}

impl Palette {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Type a command or search…"));
        let subs = vec![cx.subscribe_in(&input, window, |this, _, e: &InputEvent, _, cx| {
            if let InputEvent::Change = e {
                // The highlight stays on the first match as the query changes.
                this.active = 0;
                this.scroll.scroll_to_item(0);
                cx.notify();
            }
        })];
        let app = core(cx);
        let scope = store(cx).read(cx).scope();
        cx.spawn(async move |this, cx| {
            let found = bg(async move { Ok::<_, anyhow::Error>(app.server()?.playlists(None, None).await?) }).await;
            if let Ok(list) = found {
                this.update(cx, |p, cx| {
                    if store(cx).read(cx).scope() == scope {
                        p.playlists = list;
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
        Self { input, active: 0, playlists: Vec::new(), scroll: ScrollHandle::new(), _subs: subs }
    }

    fn commands(&self, cx: &App) -> Vec<Command> {
        let s = store(cx).read(cx);
        let n = nav(cx).read(cx);
        let mut out: Vec<Command> = Vec::new();
        let mut cmd = |id: String, label: String, group: &'static str, keywords: &'static str, icon: Option<IconName>, run: Run| {
            out.push(Command { id, label, group, keywords, icon, run });
        };
        let go = |route: Route| -> Run { Rc::new(move |_, cx| nav::push(route.clone(), cx)) };

        cmd("home".into(), "Home".into(), "Go to", "", Some(IconName::House), go(Route::Home));
        cmd("pick".into(), "Pick something to watch".into(), "Actions", "random choose decide movie night surprise me what to watch", Some(IconName::Dices), go(Route::Pick));
        cmd("playlists".into(), "Playlists".into(), "Go to", "", Some(IconName::ListMusic), go(Route::Playlists));
        if s.has_tv() {
            cmd("watchlist".into(), "Watchlist".into(), "Go to", "plex discover bookmarks", Some(IconName::Bookmark), go(Route::Watchlist));
        }
        if s.live_tv {
            cmd("livetv".into(), "Live TV".into(), "Go to", "dvr channels guide", Some(IconName::Tv), go(Route::LiveTv));
        }
        if !s.downloads.is_empty() {
            cmd("downloads".into(), "Downloads".into(), "Go to", "offline", Some(IconName::Download), go(Route::Downloads));
        }
        if s.seerr {
            cmd("requests".into(), "Requests".into(), "Go to", "seerr overseerr jellyseerr request discover missing", Some(IconName::Inbox), go(Route::Requests));
        }
        cmd("activity".into(), "Activity".into(), "Go to", "sessions history tasks", Some(IconName::Activity), go(Route::Activity));
        if s.owned() {
            cmd("server".into(), "Server".into(), "Go to", "administration libraries maintenance", Some(IconName::Server), go(Route::Server));
        }
        cmd("settings".into(), "Settings".into(), "Go to", "preferences player mpv appearance accent", Some(IconName::Settings), go(Route::Settings));
        if n.can_back() {
            cmd("back".into(), "Go back".into(), "Go to", "previous page history", Some(IconName::ArrowLeft), Rc::new(|_, cx| nav::back(cx)));
        }
        if n.can_forward() {
            cmd("forward".into(), "Go forward".into(), "Go to", "next page history", Some(IconName::ArrowRight), Rc::new(|_, cx| nav::forward(cx)));
        }

        for d in &s.sections {
            let id = d.key.clone().unwrap_or_default();
            cmd(format!("library:{id}"), d.title.clone().unwrap_or_else(|| "Library".into()), "Libraries", "library section", Some(IconName::Library), go(Route::Library { id }));
        }
        for p in &self.playlists {
            let id = p.rating_key().to_string();
            cmd(format!("playlist:{id}"), p.title.clone().unwrap_or_else(|| "Playlist".into()), "Playlists", "playlist", Some(IconName::ListMusic), go(Route::Playlist { id }));
        }

        if let Some(np) = &s.now_playing {
            let control = |c: Control| -> Run { Rc::new(move |_, _| player::control(c)) };
            let (label, icon) = if np.paused { ("Play", IconName::Play) } else { ("Pause", IconName::Pause) };
            cmd("pause".into(), label.into(), "Playback", "resume toggle", Some(icon), control(Control::TogglePause));
            if np.queue_pos + 1 < np.queue_len {
                cmd("next".into(), "Next".into(), "Playback", "skip forward track episode", Some(IconName::SkipForward), control(Control::Next));
            }
            if np.queue_pos > 0 {
                cmd("previous".into(), "Previous".into(), "Playback", "back track episode", Some(IconName::SkipBack), control(Control::Previous));
            }
            cmd("stop".into(), "Stop playback".into(), "Playback", "end quit", Some(IconName::Square), control(Control::Stop));
            if !np.live {
                cmd("back10".into(), "Back 10 seconds".into(), "Playback", "rewind seek", Some(IconName::Rewind), control(Control::SeekRelative(-10.)));
                cmd("forward30".into(), "Forward 30 seconds".into(), "Playback", "skip seek", Some(IconName::FastForward), control(Control::SeekRelative(30.)));
            }
            let panel = |p: Panel| -> Run {
                Rc::new(move |_, cx| {
                    store(cx).update(cx, |s, cx| {
                        s.panel = Some(p);
                        cx.notify();
                    })
                })
            };
            if s.queue.is_some() {
                cmd("queue".into(), "Show queue".into(), "Playback", "up next list", Some(IconName::ListOrdered), panel(Panel::Queue));
            }
            if np.item.kind() == "track" {
                cmd("lyrics".into(), "Show lyrics".into(), "Playback", "words song", Some(IconName::MicVocal), panel(Panel::Lyrics));
            }
        }

        let scope = s.scope();
        for v in s.settings.ui.saved_views.iter().filter_map(|v| serde_json::from_value::<SavedView>(v.clone()).ok()) {
            if v.scope != scope {
                continue;
            }
            let Some(library) = s.sections.iter().find(|d| d.key.as_deref() == Some(v.library.as_str())) else { continue };
            let label = format!("{}: {}", library.title.clone().unwrap_or_default(), v.name);
            let id = format!("view:{}", v.id);
            cmd(id, label, "Saved views", "view filter", Some(IconName::Film), Rc::new(move |window, cx| open_view(&v, window, cx)));
        }

        if let Some(session) = &s.session {
            for srv in &session.servers {
                if srv.id == session.server_id {
                    continue;
                }
                let id = srv.id.clone();
                cmd(
                    format!("server:{id}"),
                    format!("Switch to {}", srv.name),
                    "Servers",
                    "server change",
                    Some(IconName::ArrowLeftRight),
                    Rc::new(move |_, cx| {
                        nav(cx).update(cx, |n, cx| n.reset(cx));
                        let app = core(cx);
                        let id = id.clone();
                        store::act(cx, async move { session::switch_server(&app, &id).await }, None, false);
                    }),
                );
            }
            if session.servers.len() > 1 {
                cmd(
                    "search-all".into(),
                    "Search all servers".into(),
                    "Servers",
                    "find every server account copies",
                    Some(IconName::Search),
                    Rc::new(|window, cx| {
                        store(cx).update(cx, |s, cx| {
                            s.search_all = true;
                            cx.notify();
                        });
                        focus_search(None, window, cx);
                    }),
                );
            }
        }
        if let Some((id, label)) = undo::last(&core(cx)) {
            cmd("undo".into(), format!("Undo: {label}"), "Actions", "revert reverse watched playlist continue watching", Some(IconName::Undo2), Rc::new(move |_, cx| actions::undo(Some(id), cx)));
        }
        cmd("user".into(), "Switch user…".into(), "Account", "profile account", Some(IconName::User), go(Route::Settings));

        let spoilers = s.spoilers();
        cmd(
            "spoilers".into(),
            if spoilers { "Show spoilers" } else { "Hide spoilers" }.into(),
            "Settings",
            "unwatched thumbnails summaries",
            Some(if spoilers { IconName::Eye } else { IconName::EyeOff }),
            Rc::new(move |_, cx| store(cx).update(cx, |s, cx| s.update_ui(cx, move |ui| ui.hide_spoilers = !spoilers))),
        );
        cmd(
            "refresh".into(),
            "Refresh libraries".into(),
            "Settings",
            "reload reconnect retry",
            Some(IconName::RefreshCw),
            Rc::new(|_, cx| {
                store(cx).update(cx, |s, cx| s.refresh(cx));
                let app = core(cx);
                store::act(cx, async move { session::reload_sections(&app).await }, None, false);
            }),
        );
        out
    }

    /// The matching commands, best first, then "Search for …" (what ⌘K did before the palette existed).
    fn options(&self, cx: &App) -> (Vec<Command>, bool) {
        let q = self.input.read(cx).value().trim().to_string();
        let commands = self.commands(cx);
        let mut results: Vec<Command> = if q.is_empty() {
            commands
        } else {
            let mut scored: Vec<(Command, f64)> = commands.into_iter().filter_map(|c| matches(&q, &c.label, &format!("{} {}", c.keywords, c.group)).map(|s| (c, s))).collect();
            scored.sort_by(|a, b| b.1.total_cmp(&a.1));
            scored.into_iter().map(|(c, _)| c).collect()
        };
        let text = (!q.is_empty()).then(|| q.clone());
        results.push(Command {
            id: "search".into(),
            label: if q.is_empty() { "Search".into() } else { format!("Search for “{q}”") },
            group: "Search",
            keywords: "",
            icon: Some(IconName::Search),
            run: Rc::new(move |window, cx| focus_search(text.clone(), window, cx)),
        });
        (results, !q.is_empty())
    }

    fn run(&mut self, c: &Command, window: &mut Window, cx: &mut Context<Self>) {
        window.close_dialog(cx);
        let run = c.run.clone();
        window.defer(cx, move |window, cx| run(window, cx));
    }

    fn key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        let (options, _) = self.options(cx);
        let n = options.len();
        let command = k.modifiers.platform || k.modifiers.control;
        match k.key.as_str() {
            "k" if command => window.close_dialog(cx),
            "down" => self.active = (self.active + 1) % n,
            "up" => self.active = (self.active + n - 1) % n,
            "pagedown" => self.active = (self.active + 8).min(n - 1),
            "pageup" => self.active = self.active.saturating_sub(8),
            "enter" => {
                if let Some(c) = options.get(self.active) {
                    self.run(c, window, cx);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        self.scroll_to_active(&options, cx);
        cx.notify();
    }

    /// Keep the highlighted option in view (group headings are rows too when not filtering).
    fn scroll_to_active(&self, options: &[Command], cx: &App) {
        let filtering = !self.input.read(cx).value().trim().is_empty();
        let headings = if filtering { 0 } else { (1..=self.active).filter(|&i| options[i - 1].group != options[i].group).count() + 1 };
        self.scroll.scroll_to_item(self.active + headings);
    }
}

/// Open a library with a saved view applied.
fn open_view(v: &SavedView, window: &mut Window, cx: &mut App) {
    let route = Route::Library { id: v.library.clone() };
    nav::push(route.clone(), cx);
    let nav = nav(cx);
    let view = match nav.read(cx).stack_view() {
        Some(view) => view,
        None => {
            let view = pages::build(&route, window, cx);
            nav.update(cx, |n, _| n.current().view = Some(view.clone()));
            view
        }
    };
    if let Ok(library) = view.downcast::<Library>() {
        library.update(cx, |l, cx| l.use_view(v, cx));
    }
}

/// Go to the search page and put the cursor in the top bar's search box.
fn focus_search(text: Option<String>, window: &mut Window, cx: &mut App) {
    if let Some(text) = text {
        store(cx).update(cx, |s, cx| {
            s.search = text.into();
            cx.notify();
        });
    }
    if !nav::is(&Route::Search, cx) {
        nav::push(Route::Search, cx);
    }
    window.dispatch_action(Box::new(crate::shell::FocusSearch), cx);
}

impl Render for Palette {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (options, filtering) = self.options(cx);
        self.active = self.active.min(options.len().saturating_sub(1));
        let mut rows: Vec<AnyElement> = Vec::new();
        for (i, c) in options.iter().enumerate() {
            if !filtering && (i == 0 || options[i - 1].group != c.group) {
                rows.push(div().px_2().py_1p5().when(i > 0, |d| d.mt_1()).text_xs().font_medium().text_color(theme.muted_foreground).child(c.group).into_any_element());
            }
            let on = i == self.active;
            let command = c.clone();
            rows.push(
                h_flex()
                    .id(SharedString::from(format!("palette-{}", c.id)))
                    .gap_2()
                    .px_2()
                    .py_1p5()
                    .rounded(px(6.))
                    .text_sm()
                    .when(on, |d| d.bg(theme.accent).text_color(theme.accent_foreground))
                    .on_mouse_move(cx.listener(move |p, _, _, cx| {
                        if p.active != i {
                            p.active = i;
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |p, _, window, cx| p.run(&command, window, cx)))
                    .when_some(c.icon, |d, icon| d.child(Icon::new(icon).size_4().flex_none().text_color(theme.muted_foreground)))
                    .child(div().flex_1().min_w_0().truncate().child(c.label.clone()))
                    .when(filtering, |d| d.child(div().ml_3().text_xs().text_color(theme.muted_foreground).child(c.group)))
                    .into_any_element(),
            );
        }
        let kbd = |text: &'static str| div().px_1().min_w_5().h_5().flex().items_center().justify_center().rounded(px(4.)).bg(theme.muted).text_color(theme.foreground).text_xs().child(text);
        v_flex()
            .w_full()
            .capture_key_down(cx.listener(|p, e: &KeyDownEvent, window, cx| p.key(e, window, cx)))
            .child(
                h_flex()
                    .h_12()
                    .px_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(Input::new(&self.input).appearance(false).prefix(Icon::new(IconName::Search).size_4().text_color(theme.muted_foreground))),
            )
            .child(div().id("palette-list").max_h(px(420.)).overflow_y_scroll().track_scroll(&self.scroll).p_1().children(rows))
            .child(
                h_flex()
                    .gap_4()
                    .px_3p5()
                    .py_2()
                    .border_t_1()
                    .border_color(theme.border)
                    .bg(theme.muted.opacity(0.4))
                    .rounded_b(px(12.))
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(h_flex().gap_1p5().child(kbd("↑↓")).child("move"))
                    .child(h_flex().gap_1p5().child(kbd("↵")).child("run"))
                    .child(h_flex().gap_1p5().child(kbd("esc")).child("close")),
            )
    }
}

/// A fuzzy score (higher is better), or `None` when `query` doesn't match `text`: every
/// character of the query must appear in the text in order; runs, word starts and early matches
/// score higher.
fn fuzzy(query: &str, text: &str) -> Option<f64> {
    let q: Vec<char> = query.to_lowercase().chars().filter(|c| !c.is_whitespace()).collect();
    if q.is_empty() {
        return Some(0.);
    }
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut score, mut from, mut previous) = (0., 0usize, -2isize);
    for ch in &q {
        let at = (from..t.len()).find(|&i| t[i] == *ch)?;
        score += 1.;
        if at as isize == previous + 1 {
            score += 4.;
        }
        if at == 0 || matches!(t[at - 1], ' ' | '\t' | '-' | '_' | '/' | ':' | '.' | ',') {
            score += 6.;
        }
        score -= (at - from).min(6) as f64 * 0.3;
        previous = at as isize;
        from = at + 1;
    }
    let (qs, ts): (String, String) = (q.iter().collect(), t.iter().collect());
    if ts.starts_with(&qs) {
        score += 12.;
    } else if ts.contains(&qs) {
        score += 6.;
    }
    Some(score - t.len() as f64 * 0.02)
}

/// Match a possibly multi-word query: each word has to match the label fuzzily, or failing that
/// the start of one of the keywords (which counts for less). The scores add up.
fn matches(query: &str, label: &str, keywords: &str) -> Option<f64> {
    let extra: Vec<String> = keywords.to_lowercase().split_whitespace().map(str::to_string).collect();
    let mut total = 0.;
    for word in query.split_whitespace() {
        if let Some(s) = fuzzy(word, label) {
            total += s;
            continue;
        }
        let lower = word.to_lowercase();
        if !extra.iter().any(|k| k.starts_with(&lower)) {
            return None;
        }
        total += 2.;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn fuzzy_needs_every_character_in_order() {
        assert!(fuzzy("stg", "Settings").is_some());
        assert!(fuzzy("gts", "Settings").is_none());
        assert_eq!(fuzzy("", "Anything"), Some(0.));
    }

    #[test]
    fn prefixes_and_word_starts_rank_higher() {
        let set = fuzzy("set", "Settings").unwrap_or(f64::MIN);
        let inside = fuzzy("set", "Reset view").unwrap_or(f64::MIN);
        assert!(set > inside);
        let start = fuzzy("p", "Go to Playlists").unwrap_or(f64::MIN);
        let middle = fuzzy("l", "Go to Playlists").unwrap_or(f64::MIN);
        assert!(start > middle);
    }

    #[test]
    fn words_fall_back_to_keyword_prefixes() {
        assert!(matches("surprise", "Pick something to watch", "random surprise me").is_some());
        assert_eq!(matches("surp", "Home", "random surprise"), Some(2.));
        assert!(matches("zzz", "Home", "random").is_none());
        assert!(matches("go home", "Home", "Go to").is_some());
    }
}
