//! The window's content. Signed in: a sidebar that collapses to icons (⌘B),
//! a slim top bar, the page, and the now-playing bar. Otherwise the sign-in
//! gate.

use crate::prelude::*;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::sidebar::{Sidebar, SidebarCollapsible, SidebarFooter, SidebarGroup, SidebarHeader, SidebarItem, SidebarMenu, SidebarMenuItem, SidebarToggleButton};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use patina_core::session;
use patina_core::state::Stage;

use crate::art::art_of;
use crate::core::core;
use crate::format::initials;
use crate::gate::Gate;
use crate::nav::{self, Nav, Route, nav};
use crate::store::{StoreEvent, store};
use crate::{actions, pages, palette, player, theme};

actions!(patina, [CommandPalette, Undo, FocusSearch, Back, Forward, CloseOverlays, ToggleSidebar]);

const CONTEXT: &str = "PatinaShell";

/// A key with the platform's command modifier: ⌘ on macOS, Ctrl elsewhere.
#[cfg(target_os = "macos")]
macro_rules! cmd {
    ($k:literal) => {
        concat!("cmd-", $k)
    };
}
#[cfg(not(target_os = "macos"))]
macro_rules! cmd {
    ($k:literal) => {
        concat!("ctrl-", $k)
    };
}

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(cmd!("k"), CommandPalette, Some(CONTEXT)),
        KeyBinding::new(cmd!("z"), Undo, Some(CONTEXT)),
        KeyBinding::new(cmd!("f"), FocusSearch, Some(CONTEXT)),
        KeyBinding::new(cmd!("["), Back, Some(CONTEXT)),
        KeyBinding::new(cmd!("]"), Forward, Some(CONTEXT)),
        KeyBinding::new(cmd!("b"), ToggleSidebar, Some(CONTEXT)),
        KeyBinding::new("escape", CloseOverlays, Some(CONTEXT)),
    ]);
}

pub struct Main {
    gate: Option<Entity<Gate>>,
    search: Entity<InputState>,
    collapsed: bool,
    focus: FocusHandle,
    /// Set while the search box text is being changed from the store, so it doesn't echo back.
    syncing: bool,
    debug_done: bool,
    _subs: Vec<Subscription>,
}

impl Main {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        let shared = store(cx);
        let subs = vec![
            cx.observe_in(&shared, window, |this, store, window, cx| {
                // Keep the search box in step when another view clears the query.
                let text = store.read(cx).search.clone();
                if this.search.read(cx).value() != text {
                    this.syncing = true;
                    this.search.update(cx, |s, cx| s.set_value(text, window, cx));
                    this.syncing = false;
                }
                theme::set_tint(store.read(cx).tint(), cx);
                cx.notify();
            }),
            cx.subscribe_in(&shared, window, |_, _, event: &StoreEvent, window, cx| match event {
                StoreEvent::Toast { text, error, undo } => {
                    let mut note = if *error { Notification::error(text.clone()) } else { Notification::new().message(text.clone()) };
                    if let Some(id) = *undo {
                        note = note.action(move |_, _, _| {
                            Button::new("undo").outline().small().label("Undo").on_click(move |_, _, cx| actions::undo(Some(id), cx))
                        });
                    }
                    window.push_notification(note, cx);
                }
                StoreEvent::ScopeChanged => {
                    window.close_all_dialogs(cx);
                    nav(cx).update(cx, |n, cx| n.reset(cx));
                }
            }),
            cx.observe(&nav(cx), |_, _, cx| cx.notify()),
            cx.subscribe_in(&search, window, |this, state, e: &InputEvent, window, cx| match e {
                InputEvent::Change if !this.syncing => {
                    let value = state.read(cx).value();
                    store(cx).update(cx, |s, cx| {
                        s.search = value;
                        cx.notify();
                    });
                    if !nav::is(&Route::Search, cx) {
                        nav::push(Route::Search, cx);
                    }
                }
                InputEvent::PressEnter { .. } => {
                    if !nav::is(&Route::Search, cx) {
                        nav::push(Route::Search, cx);
                    }
                    let _ = window;
                }
                _ => {}
            }),
        ];
        theme::set_tint(shared.read(cx).tint(), cx);
        Self { gate: None, search, collapsed: false, focus: cx.focus_handle(), syncing: false, debug_done: false, _subs: subs }
    }

    /// Debug builds: `PATINA_ROUTE=/item/100` opens a page, `PATINA_PLAY=<ratingKey>` starts playback.
    fn debug_start(&mut self, cx: &mut Context<Self>) {
        if self.debug_done {
            return;
        }
        self.debug_done = true;
        let info = &store(cx).read(cx).info;
        let (route, play) = (info.debug_route.clone(), info.debug_play.clone());
        if let Some(route) = route.as_deref().and_then(Route::parse) {
            nav::push(route, cx);
        }
        if let Some(rk) = play {
            let app = core(cx);
            crate::store::act_then(cx, async move { Ok::<_, anyhow::Error>(app.server()?.metadata(&rk, patina_plex::MetadataOptions::full()).await?) }, |m, cx| {
                if let Some(m) = m {
                    actions::play(&m, cx);
                }
            });
        }
    }

    fn sidebar(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let s = store(cx).read(cx);
        let route = nav(cx).read(cx).route().clone();
        let collapsed = self.collapsed;
        let link = |label: &str, icon: IconName, target: Route, active: bool| {
            SidebarMenuItem::new(label.to_string()).icon(icon).active(active).on_click(move |_, _, cx| nav::push(target.clone(), cx))
        };
        let is = |r: Route| route == r;
        let active_downloads = s.downloads.iter().filter(|d| matches!(d.state, patina_core::downloads::State::Queued | patina_core::downloads::State::Downloading)).count();
        let tint = s.tint();

        let mut main = SidebarMenu::new()
            .child(link("Home", IconName::House, Route::Home, is(Route::Home)))
            .child(link("Search", IconName::Search, Route::Search, is(Route::Search)))
            .child(link("Pick something", IconName::Dices, Route::Pick, is(Route::Pick)));
        if s.has_tv() {
            main = main.child(link("Watchlist", IconName::Bookmark, Route::Watchlist, is(Route::Watchlist)));
        }
        main = main.child(link("Playlists", IconName::ListMusic, Route::Playlists, is(Route::Playlists)));
        if s.live_tv {
            main = main.child(link("Live TV", IconName::Radio, Route::LiveTv, is(Route::LiveTv)));
        }
        if s.seerr {
            let on = matches!(route, Route::Requests | Route::Seerr { .. });
            main = main.child(link("Requests", IconName::Inbox, Route::Requests, on));
        }
        if !s.downloads.is_empty() {
            let mut item = link("Downloads", IconName::Download, Route::Downloads, is(Route::Downloads));
            if active_downloads > 0 {
                item = item.suffix(move |_, _| div().text_xs().text_color(tint).child(active_downloads.to_string()));
            }
            main = main.child(item);
        }

        let scanning: Vec<String> = s.activities.iter().filter_map(|a| a.section_id.clone()).collect();
        let libraries = SidebarMenu::new().children(s.sections.iter().map(|d| {
            let key = d.key.clone().unwrap_or_default();
            let icon = match d.kind.as_deref() {
                Some("movie") => IconName::Clapperboard,
                Some("show") => IconName::Tv,
                Some("artist") => IconName::Music,
                Some("photo") => IconName::Image,
                _ => IconName::Folder,
            };
            let active = route == Route::Library { id: key.clone() };
            let mut item = link(d.title.as_deref().unwrap_or(""), icon, Route::Library { id: key.clone() }, active);
            if scanning.contains(&key) {
                item = item.suffix(|_, _| Spinner::new().xsmall());
            }
            item
        }));

        let mut tools = SidebarMenu::new().child(link("Activity", IconName::Activity, Route::Activity, is(Route::Activity)));
        if s.owned() {
            tools = tools.child(link("Server", IconName::Server, Route::Server, is(Route::Server)));
        }
        tools = tools.child(link("Settings", IconName::Settings, Route::Settings, is(Route::Settings)));

        let task = s.activities.first().cloned();
        let session = s.session.clone();
        let version = s.info.version;
        let user_name = session.as_ref().and_then(|x| x.user.as_ref()).map_or("Server owner".to_string(), |u| u.name.clone());
        let thumb = session.as_ref().and_then(|x| x.user.as_ref()).and_then(|u| u.thumb.clone());
        let theme = cx.theme().clone();
        let mac = cfg!(target_os = "macos");

        let mut groups = vec![SidebarGroup::new("").child(main)];
        if !s.sections.is_empty() {
            groups.push(SidebarGroup::new("Libraries").child(libraries));
        }

        let servers = session.as_ref().map(|x| (x.servers.clone(), x.server_id.clone())).unwrap_or_default();
        let user_button = h_flex()
            .id("user")
            .w_full()
            .gap_2()
            .p_2()
            .rounded(px(8.))
            .cursor_pointer()
            .hover(|st| st.bg(theme.sidebar_accent))
            .child(
                div()
                    .size_8()
                    .flex_none()
                    .rounded(px(8.))
                    .overflow_hidden()
                    .bg(theme.secondary)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .child(initials(&user_name))
                    .children(art_of(thumb.as_deref(), 32., 32., None).map(|i| i.absolute().inset_0().size_full())),
            )
            .when(!collapsed, |d| {
                d.child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(div().text_sm().font_medium().truncate().child(user_name.clone()))
                        .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(session.as_ref().map(|x| x.server_name.clone()).unwrap_or_default())),
                )
                .child(Icon::new(IconName::ChevronsUpDown).size_4().text_color(theme.muted_foreground))
            });
        let user_menu = Button::new("user-menu")
            .ghost()
            .w_full()
            .h_auto()
            .p_0()
            .child(user_button)
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                menu = menu.min_w(px(224.));
                if servers.0.len() > 1 {
                    menu = menu.label("Switch server");
                    for sv in &servers.0 {
                        let id = sv.id.clone();
                        let current = id == servers.1;
                        menu = menu.item(PopupMenuItem::new(sv.name.clone()).checked(current).on_click(move |_, _, cx| {
                            if current {
                                return;
                            }
                            nav(cx).update(cx, |n, cx| n.reset(cx));
                            let app = core(cx);
                            let id = id.clone();
                            crate::store::act(cx, async move { session::switch_server(&app, &id).await }, None, false);
                        }));
                    }
                    menu = menu.separator();
                }
                menu.item(PopupMenuItem::new("Switch user…").icon(IconName::UserRound).on_click(|_, _, cx| nav::push(Route::Settings, cx)))
                    .item(PopupMenuItem::new("Sign out").icon(IconName::LogOut).on_click(|_, _, cx| session::sign_out(&core(cx))))
            });

        let footer = v_flex()
            .w_full()
            .gap_2()
            .when_some(task.filter(|_| !collapsed), |d, task| {
                d.child(
                    v_flex()
                        .id("task")
                        .gap_2()
                        .px_3()
                        .py_2p5()
                        .rounded(px(8.))
                        .border_1()
                        .border_color(theme.border)
                        .bg(theme.sidebar_accent.opacity(0.4))
                        .cursor_pointer()
                        .hover(|st| st.bg(theme.sidebar_accent))
                        .on_click(|_, _, cx| nav::push(Route::Activity, cx))
                        .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(task.title.clone()))
                        .child(Progress::new("task-progress").value(task.progress as f32)),
                )
            })
            .child(SidebarItem::render(tools.collapsed(collapsed), "tools", window, cx))
            .child(div().h_px().bg(theme.sidebar_border))
            .child(user_menu);

        Sidebar::new("sidebar")
            .collapsible(SidebarCollapsible::Icon)
            .collapsed(collapsed)
            .w(px(240.))
            .header(
                SidebarHeader::new()
                    .pt(if mac { px(36.) } else { px(12.) })
                    .child(
                        div()
                            .size_8()
                            .flex_none()
                            .rounded(px(8.))
                            .bg(tint)
                            .text_color(theme.primary_foreground)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .font_semibold()
                            .child("P"),
                    )
                    .when(!collapsed, |d| {
                        d.child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().font_semibold().child("Patina"))
                                .child(div().text_xs().text_color(theme.muted_foreground).child(format!("Plex client · {version}"))),
                        )
                    }),
            )
            .children(groups)
            .footer(SidebarFooter::new().child(footer))
    }

    fn topbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let n = nav(cx).read(cx);
        let (can_back, can_forward) = (n.can_back(), n.can_forward());
        let theme = cx.theme().clone();
        let clear_lights = cfg!(target_os = "macos") && self.collapsed;
        h_flex()
            .h(px(52.))
            .flex_none()
            .items_center()
            .gap_0p5()
            .pr_4()
            .pl(if clear_lights { px(84.) } else { px(12.) })
            .border_b_1()
            .border_color(theme.border)
            .child(SidebarToggleButton::new().collapsed(self.collapsed).on_click(cx.listener(|this, _, _, cx| {
                this.collapsed = !this.collapsed;
                cx.notify();
            })))
            .child(div().w_px().h_4().mx_1p5().bg(theme.border))
            .child(Button::new("back").ghost().small().icon(IconName::ChevronLeft).tooltip("Back (⌘[)").disabled(!can_back).on_click(|_, _, cx| nav::back(cx)))
            .child(Button::new("forward").ghost().small().icon(IconName::ChevronRight).tooltip("Forward (⌘])").disabled(!can_forward).on_click(|_, _, cx| nav::forward(cx)))
            .child(
                div().ml_2().w(px(320.)).child(
                    Input::new(&self.search)
                        .small()
                        .prefix(Icon::new(IconName::Search).size_4().text_color(theme.muted_foreground))
                        .suffix(Kbd::new(Keystroke::parse(cmd!("f")).unwrap()))
                        .cleanable(true),
                ),
            )
            .child(div().flex_1())
            .child(
                Button::new("commands")
                    .outline()
                    .small()
                    .child(h_flex().gap_2().text_color(theme.muted_foreground).child("Commands").child(Kbd::new(Keystroke::parse(cmd!("k")).unwrap())))
                    .on_click(|_, window, cx| palette::open(window, cx)),
            )
    }

    fn connection_banner(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        h_flex()
            .justify_between()
            .gap_4()
            .py_2()
            .px(theme::GUTTER)
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.danger.opacity(0.08))
            .text_color(theme.muted_foreground)
            .child("Connection lost. Reconnecting… Saved results and downloads remain available.")
            .child(Button::new("retry-connection").outline().small().label("Retry").on_click(|_, _, cx| {
                store(cx).update(cx, |s, cx| s.refresh(cx));
                let app = core(cx);
                crate::store::act(cx, async move { session::reload_sections(&app).await }, None, false);
            }))
    }

    fn content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyView {
        let nav = nav(cx);
        let existing = nav.read(cx).stack_view();
        if let Some(view) = existing {
            return view;
        }
        let route = nav.read(cx).route().clone();
        let view = pages::build(&route, window, cx);
        nav.update(cx, |n: &mut Nav, _| n.current().view = Some(view.clone()));
        view
    }
}

impl Nav {
    /// The current page's view, if it is built.
    pub fn stack_view(&self) -> Option<AnyView> {
        self.current_ref().view.clone()
    }
}

impl Render for Main {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (ready, connected) = {
            let s = store(cx).read(cx);
            (matches!(s.stage, Stage::Ready) && s.session.is_some(), s.connected)
        };
        if !ready {
            let gate = self.gate.get_or_insert_with(|| cx.new(|cx| Gate::new(window, cx))).clone();
            return div().size_full().child(gate).into_any_element();
        }
        self.gate = None;
        self.debug_start(cx);
        let content = self.content(window, cx);
        h_flex()
            .size_full()
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &CommandPalette, window, cx| palette::open(window, cx)))
            .on_action(cx.listener(|_, _: &Undo, _, cx| actions::undo_latest(cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.search.update(cx, |s, cx| {
                    s.focus(window, cx);
                    s.select_all(window, cx);
                })
            }))
            .on_action(cx.listener(|_, _: &Back, _, cx| nav::back(cx)))
            .on_action(cx.listener(|_, _: &Forward, _, cx| nav::forward(cx)))
            .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| {
                this.collapsed = !this.collapsed;
                cx.notify();
            }))
            .on_action(cx.listener(|_, _: &CloseOverlays, _, cx| {
                store(cx).update(cx, |s, cx| {
                    s.panel = None;
                    cx.notify();
                })
            }))
            .on_mouse_down(MouseButton::Navigate(NavigationDirection::Back), |_, _, cx| nav::back(cx))
            .on_mouse_down(MouseButton::Navigate(NavigationDirection::Forward), |_, _, cx| nav::forward(cx))
            .bg(cx.theme().colors.sidebar)
            .child(self.sidebar(window, cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .bg(cx.theme().background)
                    .child(self.topbar(cx))
                    .when(!connected, |d| d.child(self.connection_banner(cx)))
                    .child(player::failure_banner(window, cx))
                    .child(player::resume_banner(window, cx))
                    .child(div().flex_1().min_h_0().overflow_hidden().child(content))
                    .child(player::now_playing_bar(window, cx)),
            )
            .into_any_element()
    }
}

