//! Request a movie, or seasons of a show, from Seerr. Users allowed advanced
//! requests also choose the Radarr/Sonarr server, quality profile and folder.

use crate::prelude::*;
use std::rc::Rc;

use gpui_kit::component::checkbox::Checkbox;
use patina_core::seerr::{self, NewRequest, RequestOptionServer, RequestState, SeerrCan, SeerrDetails, SeerrMediaType, SeerrSeason};

use crate::core::{bg, core};
use crate::format;
use crate::pages::browse::ui;
use crate::pages::requests::seerr::open_seasons;
use crate::store::{self, store};
use crate::widgets;

/// Open the request dialog; `on_done` runs once the request is made.
pub fn open(details: SeerrDetails, is_4k: bool, can: SeerrCan, on_done: impl Fn(&mut App) + 'static, window: &mut Window, cx: &mut App) {
    let on_done: Rc<dyn Fn(&mut App)> = Rc::new(on_done);
    let title = format!("Request {}{}", details.title, if is_4k { " in 4K" } else { "" });
    let wide = details.media_type == SeerrMediaType::Tv;
    let view = cx.new(|cx| SeerrRequest::new(details, is_4k, can, on_done, cx));
    window.open_dialog(cx, move |dialog, _, _| dialog.title(title.clone()).w(px(if wide { 640. } else { 440. })).child(view.clone()));
}

pub struct SeerrRequest {
    d: SeerrDetails,
    is_4k: bool,
    can: SeerrCan,
    seasons: Vec<SeerrSeason>,
    picked: Vec<i64>,
    busy: bool,
    scope: String,
    /// Advanced options: servers of the right kind and quality, with their defaults preselected.
    servers: Option<Vec<RequestOptionServer>>,
    options_error: Option<String>,
    server_id: Option<i64>,
    profile_id: Option<i64>,
    root_folder: Option<String>,
    on_done: Rc<dyn Fn(&mut App)>,
}

impl SeerrRequest {
    fn new(d: SeerrDetails, is_4k: bool, can: SeerrCan, on_done: Rc<dyn Fn(&mut App)>, cx: &mut Context<Self>) -> Self {
        let seasons = if d.media_type == SeerrMediaType::Tv { open_seasons(&d, is_4k) } else { Vec::new() };
        let this = Self {
            seasons,
            picked: Vec::new(),
            busy: false,
            scope: store(cx).read(cx).scope(),
            servers: None,
            options_error: None,
            server_id: None,
            profile_id: None,
            root_folder: None,
            on_done,
            can,
            is_4k,
            d,
        };
        if can.advanced {
            let app = core(cx);
            let (media_type, is_4k) = (this.d.media_type, is_4k);
            cx.spawn(async move |this, cx| {
                let result = bg(async move { seerr::request_options(&app, media_type, is_4k).await }).await;
                this.update(cx, |v, cx| {
                    match result {
                        Ok(list) => {
                            let first = list.iter().find(|s| s.is_default).or(list.first()).cloned();
                            v.servers = Some(list);
                            v.choose(first.as_ref());
                        }
                        Err(e) => v.options_error = Some(e.to_string()),
                    }
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        this
    }

    fn choose(&mut self, s: Option<&RequestOptionServer>) {
        self.server_id = s.map(|s| s.id);
        self.profile_id = s.and_then(|s| s.active_profile_id.or(s.profiles.first().map(|p| p.id)));
        self.root_folder = s.and_then(|s| s.active_directory.clone().or(s.root_folders.first().map(|f| f.path.clone())));
    }

    fn server(&self) -> Option<&RequestOptionServer> {
        self.servers.as_ref()?.iter().find(|s| Some(s.id) == self.server_id)
    }

    fn toggle(&mut self, n: i64, on: bool) {
        self.picked.retain(|x| *x != n);
        if on {
            self.picked.push(n);
            self.picked.sort_unstable();
        }
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if store(cx).read(cx).scope() != self.scope {
            window.close_dialog(cx);
            return;
        }
        self.busy = true;
        cx.notify();
        let has_server = self.server().is_some();
        let r = NewRequest {
            media_type: self.d.media_type,
            tmdb_id: self.d.tmdb_id,
            tvdb_id: self.d.tvdb_id,
            seasons: if self.d.media_type == SeerrMediaType::Tv { self.picked.clone() } else { Vec::new() },
            is_4k: self.is_4k,
            server_id: if has_server { self.server_id } else { None },
            profile_id: if has_server { self.profile_id } else { None },
            root_folder: if has_server { self.root_folder.clone() } else { None },
        };
        let title = self.d.title.clone();
        let app = core(cx);
        cx.spawn_in(window, async move |this, cx| {
            let result = bg(async move { seerr::request(&app, &r).await }).await;
            this.update_in(cx, |v, window, cx| {
                v.busy = false;
                match result {
                    Ok(state) => {
                        store::toast(cx, if state == RequestState::Approved { format!("{title} requested and approved") } else { format!("{title} requested") });
                        window.close_dialog(cx);
                        (v.on_done)(cx);
                    }
                    Err(e) => store::error(cx, e.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn advanced(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let theme = cx.theme().clone();
        if !self.can.advanced {
            return None;
        }
        if let Some(e) = &self.options_error {
            return Some(div().mt_4().text_xs().text_color(theme.danger).child(e.clone()).into_any_element());
        }
        let servers = self.servers.as_ref().filter(|s| !s.is_empty())?;
        let me = cx.entity().downgrade();
        let field = |label: String, control: AnyElement| v_flex().flex_1().min_w_0().gap_1p5().child(div().text_xs().text_color(theme.muted_foreground).child(label)).child(control);
        let server_options = servers.iter().map(|s| (Some(s.id), SharedString::from(format!("{}{}", s.name, if s.is_default { " (default)" } else { "" })))).collect();
        let server = self.server();
        let profiles = server.map(|s| s.profiles.iter().map(|p| (Some(p.id), SharedString::from(p.name.clone()))).collect()).unwrap_or_default();
        let folders = server
            .map(|s| {
                s.root_folders
                    .iter()
                    .map(|f| {
                        let free = f.free_space.map(|b| format!(" ({} free)", format::bytes(b as i64))).unwrap_or_default();
                        (Some(f.path.clone()), SharedString::from(format!("{}{free}", f.path)))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let (m1, m2, m3) = (me.clone(), me.clone(), me);
        let kind = if self.d.media_type == SeerrMediaType::Tv { "Sonarr" } else { "Radarr" };
        Some(
            h_flex()
                .mt_4()
                .gap_3()
                .items_start()
                .child(field(
                    format!("{kind} server"),
                    ui::select("seerr-server", None, server_options, &self.server_id, self.busy, move |id, _, cx| {
                        m1.update(cx, |v, cx| {
                            let s = v.servers.as_ref().and_then(|l| l.iter().find(|s| Some(s.id) == id)).cloned();
                            v.choose(s.as_ref());
                            cx.notify();
                        })
                        .ok();
                    }),
                ))
                .child(field(
                    "Quality profile".into(),
                    ui::select("seerr-profile", None, profiles, &self.profile_id, self.busy, move |id, _, cx| {
                        m2.update(cx, |v, cx| {
                            v.profile_id = id;
                            cx.notify();
                        })
                        .ok();
                    }),
                ))
                .child(field(
                    "Folder".into(),
                    ui::select("seerr-folder", None, folders, &self.root_folder, self.busy, move |path, _, cx| {
                        m3.update(cx, |v, cx| {
                            v.root_folder = path;
                            cx.notify();
                        })
                        .ok();
                    }),
                ))
                .into_any_element(),
        )
    }
}

impl Render for SeerrRequest {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let tv = self.d.media_type == SeerrMediaType::Tv;
        let all = !self.seasons.is_empty() && self.picked.len() == self.seasons.len();
        let busy = self.busy;
        let desc = if tv { "Choose the seasons to add to the server." } else { "Seerr asks Radarr to find it and adds it to the server once downloaded." };
        let n = self.picked.len();
        let label = if tv && n > 0 { format!("Request {}", format::plural(n as i64, "season")) } else { "Request".to_string() };
        let advanced = self.advanced(cx);
        v_flex()
            .gap_0()
            .child(div().text_sm().text_color(theme.muted_foreground).child(desc))
            .when(tv, |d| {
                let numbers: Vec<i64> = self.seasons.iter().map(|s| s.number).collect();
                d.child(
                    widgets::rows(cx)
                        .id("seerr-seasons")
                        .mt_4()
                        .max_h(px(320.))
                        .overflow_y_scroll()
                        .child(widgets::list_row(true, cx).child(Checkbox::new("all-seasons").label("All seasons").checked(all).disabled(busy).on_click(cx.listener(move |v, on: &bool, _, cx| {
                            v.picked = if *on { numbers.clone() } else { Vec::new() };
                            cx.notify();
                        }))))
                        .children(self.seasons.iter().map(|s| {
                            let num = s.number;
                            let note = ui::dots([s.episodes.map(|e| format::plural(e, "episode")), s.air_date.as_deref().and_then(|a| a.get(..4)).map(str::to_string)]);
                            widgets::list_row(false, cx)
                                .child(
                                    div().flex_1().min_w_0().child(Checkbox::new(("season", num as u64)).label(s.name.clone()).checked(self.picked.contains(&num)).disabled(busy).on_click(cx.listener(
                                        move |v, on: &bool, _, cx| {
                                            v.toggle(num, *on);
                                            cx.notify();
                                        },
                                    ))),
                                )
                                .child(div().flex_none().text_xs().text_color(theme.muted_foreground).child(note))
                        })),
                )
            })
            .children(advanced)
            .child(
                h_flex()
                    .mt_6()
                    .gap_2()
                    .justify_end()
                    .child(Button::new("seerr-cancel").outline().label("Cancel").disabled(busy).on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(Button::new("seerr-submit").primary().label(label).disabled(busy || (tv && n == 0)).on_click(cx.listener(|v, _, window, cx| v.submit(window, cx)))),
            )
    }
}
