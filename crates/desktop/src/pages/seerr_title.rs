//! A movie or show as Seerr knows it: request it (or more seasons), follow its
//! requests, or open the copy on the Plex server once it is there.

use crate::prelude::*;
use std::collections::HashSet;
use std::rc::Rc;

use patina_core::seerr::{self, Availability, SeerrDetails, SeerrMediaType, SeerrStatus};

use crate::art::art_of;
use crate::core::{bg, core};
use crate::dialogs;
use crate::format;
use crate::nav::{self, Route};
use crate::pages::browse::ui::{self, Tone};
use crate::pages::requests::seerr::{self as s, RowEvents};
use crate::store;
use crate::theme::GUTTER;
use crate::widgets;

pub struct SeerrTitle {
    media_type: String,
    tmdb_id: i64,
    data: Option<SeerrDetails>,
    error: Option<String>,
    status: Option<SeerrStatus>,
    busy: HashSet<i64>,
    generation: u64,
}

impl SeerrTitle {
    pub fn new(media_type: String, tmdb_id: i64, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = Self { media_type, tmdb_id, data: None, error: None, status: None, busy: HashSet::new(), generation: 0 };
        this.load(cx);
        if let Some(fut) = s::status(cx) {
            cx.spawn(async move |this, cx| {
                let result = fut.await;
                this.update(cx, |p, cx| match result {
                    Ok(st) => {
                        p.status = Some(st);
                        cx.notify();
                    }
                    Err(_) => s::forget_status(cx),
                })
                .ok();
            })
            .detach();
        }
        this
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        let mine = self.generation;
        self.error = None;
        let app = core(cx);
        let (media_type, tmdb_id) = (self.media_type.clone(), self.tmdb_id);
        cx.spawn(async move |this, cx| {
            let result = bg(async move {
                let media_type: SeerrMediaType = media_type.parse()?;
                seerr::details(&app, media_type, tmdb_id).await
            })
            .await;
            this.update(cx, |p, cx| {
                if mine != p.generation {
                    return;
                }
                match result {
                    Ok(d) => p.data = Some(d),
                    Err(e) => p.error = Some(e.to_string()),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn reloader(&self, cx: &mut Context<Self>) -> Rc<dyn Fn(&mut App)> {
        let me = cx.entity().downgrade();
        Rc::new(move |cx| {
            me.update(cx, |p, cx| p.load(cx)).ok();
        })
    }

    fn header(&self, d: &SeerrDetails, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let tv = d.media_type == SeerrMediaType::Tv;
        let can = self.status.as_ref().map(|st| st.can);
        let label = s::availability_label(d.status);
        let label_4k = s::availability_label(d.status_4k);
        let mut meta: Vec<String> = vec![];
        meta.extend(d.rating.filter(|r| *r > 0.).map(|r| format!("★ {r:.1}")));
        meta.extend(d.year.map(|y| y.to_string()));
        meta.extend(d.runtime.filter(|r| *r > 0).map(|r| format::duration(r * 60_000)));
        if tv {
            meta.push(format::plural(d.seasons.len() as i64, "season"));
        }
        meta.extend(d.genres.iter().take(3).filter(|g| !g.is_empty()).cloned());
        let meta_row = h_flex()
            .mt_3p5()
            .flex_wrap()
            .items_center()
            .gap_x_2()
            .gap_y_2()
            .text_sm()
            .text_color(theme.muted_foreground)
            .children(meta.into_iter().enumerate().flat_map(|(i, m)| {
                let sep = (i > 0).then(|| div().text_color(theme.muted_foreground.opacity(0.5)).child("·").into_any_element());
                sep.into_iter().chain(std::iter::once(div().child(m).into_any_element()))
            }))
            .when_some(label, |row, l| row.child(ui::badge(l, if matches!(d.status, Some(Availability::Available | Availability::Partial)) { Tone::Tint } else { Tone::Plain }, cx)))
            .when_some(label_4k, |row, l| row.child(ui::badge(format!("4K · {l}"), Tone::Plain, cx)));

        let mut buttons = h_flex().mt_6().flex_wrap().items_center().gap_2();
        if let Some(rk) = d.rating_key.clone() {
            buttons = buttons.child(Button::new("open-plex").primary().large().icon(IconName::Play).label("Open in Plex").on_click(move |_, _, cx| nav::push(Route::Item { rating_key: rk.clone(), server: None }, cx)));
        }
        let reload = self.reloader(cx);
        let ask = |is_4k: bool| {
            let details = d.clone();
            let reload = reload.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                let Some(can) = can else { return };
                let reload = reload.clone();
                dialogs::seerr_request::open(details.clone(), is_4k, can, move |cx| reload(cx), window, cx);
            }
        };
        if can.is_some_and(|c| s::requestable(d, &c, false)) {
            let more = tv && d.seasons.iter().any(|x| x.status.is_some() || x.requested.is_some());
            let b = Button::new("request").large().icon(IconName::Plus).label(if more { "Request more" } else { "Request" }).on_click(ask(false));
            buttons = buttons.child(if d.rating_key.is_some() { b.outline() } else { b.primary() });
        }
        if can.is_some_and(|c| s::requestable(d, &c, true)) {
            buttons = buttons.child(Button::new("request-4k").outline().large().label("Request in 4K").on_click(ask(true)));
        }
        let (mt, id) = (d.media_type, d.tmdb_id);
        buttons = buttons.child(Button::new("open-seerr").outline().large().icon(IconName::ExternalLink).label("Open in Seerr").on_click(move |_, _, cx| match seerr::page_url(&core(cx), mt, id) {
            Ok(url) => cx.open_url(&url),
            Err(e) => store::error(cx, e.to_string()),
        }));
        let may_not = self.status.as_ref().filter(|st| !(if tv { st.can.request_tv } else { st.can.request_movie })).map(|_| format!("Your Seerr account may not request {}.", if tv { "shows" } else { "movies" }));

        h_flex()
            .items_end()
            .gap(px(36.))
            .child(
                div()
                    .flex_none()
                    .w(px(210.))
                    .h(px(315.))
                    .overflow_hidden()
                    .rounded(px(12.))
                    .border_1()
                    .border_color(theme.border)
                    .bg(theme.muted)
                    .children(art_of(d.poster.as_deref(), 500., 750., None).map(|i| i.size_full().object_fit(ObjectFit::Cover))),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .pb_1()
                    .child(div().mb_3().text_sm().text_color(theme.muted_foreground).child(format!("{} · Seerr", if tv { "Show" } else { "Movie" })))
                    .child(div().text_size(px(36.)).line_height(px(39.)).font_semibold().child(d.title.clone()))
                    .child(meta_row)
                    .child(buttons)
                    .when_some(may_not, |c, t| c.child(div().mt_3().text_xs().text_color(theme.muted_foreground).child(t))),
            )
    }
}

impl Render for SeerrTitle {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let me = cx.entity().downgrade();
        let retry: ui::Handler = Rc::new(move |_, cx| {
            me.update(cx, |p, cx| p.load(cx)).ok();
        });
        let Some(d) = self.data.clone() else {
            return widgets::page("seerr-title")
                .child(match &self.error {
                    Some(e) => ui::status_error("seerr-retry", e.clone(), Some(retry), cx).min_h(px(480.)),
                    None => ui::status_loading().min_h(px(480.)),
                })
                .into_any_element();
        };
        let can = self.status.as_ref().map(|st| st.can);
        let bg = theme.background;
        let me = cx.entity().downgrade();
        let me2 = me.clone();
        let events = RowEvents {
            busy: Rc::new(move |id, on, cx| {
                me.update(cx, |p, cx| {
                    if on {
                        p.busy.insert(id);
                    } else {
                        p.busy.remove(&id);
                    }
                    cx.notify();
                })
                .ok();
            }),
            changed: Rc::new(move |cx| {
                me2.update(cx, |p, cx| p.load(cx)).ok();
            }),
        };
        let header = self.header(&d, cx);
        let mut content = v_flex().relative().px(GUTTER).pt(px(128.)).pb(px(64.));
        if let Some(e) = &self.error {
            content = content.child(ui::stale_alert("seerr-retry", e.clone(), Some(retry), cx));
        }
        content = content.child(header).child(
            v_flex()
                .mt_10()
                .max_w(px(680.))
                .when(!d.tagline.is_empty(), |c| c.child(div().mb_3().text_lg().italic().text_color(theme.muted_foreground).child(d.tagline.clone())))
                .when(!d.overview.is_empty(), |c| c.child(div().text_size(px(15.)).line_height(px(24.)).child(d.overview.clone()))),
        );
        if !d.seasons.is_empty() {
            content = content.child(widgets::section_head("Seasons", None)).child(widgets::rows(cx).children(d.seasons.iter().enumerate().map(|(i, season)| {
                let note = ui::dots([season.episodes.map(|e| format!("{e} episodes")), season.air_date.as_deref().map(format::air_date)]);
                widgets::list_row(i == 0, cx)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_sm().truncate().child(season.name.clone()))
                            .when(!note.is_empty(), |c| c.child(div().text_xs().text_color(theme.muted_foreground).truncate().child(note))),
                    )
                    .children(s::season_label(season).map(|l| ui::badge(l, if season.status == Some(Availability::Available) { Tone::Tint } else { Tone::Plain }, cx)))
            })));
        }
        if !d.requests.is_empty() {
            content = content
                .child(widgets::section_head("Requests", None))
                .child(widgets::rows(cx).children(d.requests.iter().enumerate().map(|(i, r)| s::request_row(r, can, false, i == 0, self.busy.contains(&r.id), events.clone(), cx))));
        }
        div()
            .id("seerr-title")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .relative()
                    .child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .right_0()
                            .h(px(520.))
                            .overflow_hidden()
                            .opacity(0.5)
                            .children(art_of(d.backdrop.as_deref(), 1920., 1080., None).map(|i| i.size_full().object_fit(ObjectFit::Cover))),
                    )
                    // Outside the faded layer, so the bottom blends fully into the page.
                    .child(div().absolute().top_0().left_0().right_0().h(px(520.)).bg(linear_gradient(180., linear_color_stop(bg.opacity(0.1), 0.), linear_color_stop(bg, 1.))))
                    .child(content),
            )
            .into_any_element()
    }
}
