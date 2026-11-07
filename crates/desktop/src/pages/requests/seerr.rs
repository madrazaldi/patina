//! Seerr pieces the pages share: labels for statuses, what can still be
//! requested, the cached sign-in status, the title card and the request row.

use crate::prelude::*;
use std::rc::Rc;

use futures::FutureExt as _;
use futures::future::{BoxFuture, Shared};
use gpui_kit::Global;
use patina_core::seerr::{self, Availability, RequestAction, RequestState, SeerrCan, SeerrDetails, SeerrMediaType, SeerrRequest, SeerrSeason, SeerrStatus, SeerrTitle};

use crate::art::art_of;
use crate::core::{bg, core};
use crate::nav::{self, Route};
use crate::pages::browse::ui::{self, Tone};
use crate::store::{self, store};
use crate::widgets;

/// "Available", "Requested", ...; `None` when there's nothing to say.
pub fn availability_label(a: Option<Availability>) -> Option<&'static str> {
    Some(match a? {
        Availability::Pending => "Requested",
        Availability::Processing => "Processing",
        Availability::Partial => "Partly available",
        Availability::Available => "Available",
        Availability::Blocklisted => "Blocklisted",
        Availability::Deleted => "Deleted",
        Availability::Unknown => return None,
    })
}

pub fn request_label(s: RequestState) -> &'static str {
    match s {
        RequestState::Pending => "Pending approval",
        RequestState::Approved => "Approved",
        RequestState::Declined => "Declined",
        RequestState::Failed => "Failed",
        RequestState::Completed => "Completed",
    }
}

/// Nothing has been requested or added yet.
fn is_new(a: Option<Availability>) -> bool {
    matches!(a, None | Some(Availability::Unknown | Availability::Deleted))
}

fn can_request_kind(can: &SeerrCan, media_type: SeerrMediaType, is_4k: bool) -> bool {
    match (media_type, is_4k) {
        (SeerrMediaType::Movie, false) => can.request_movie,
        (SeerrMediaType::Movie, true) => can.request_4k_movie,
        (SeerrMediaType::Tv, false) => can.request_tv,
        (SeerrMediaType::Tv, true) => can.request_4k_tv,
    }
}

/// Seasons of a show that can still be requested in this quality.
pub fn open_seasons(d: &SeerrDetails, is_4k: bool) -> Vec<SeerrSeason> {
    d.seasons
        .iter()
        .filter(|s| {
            let (status, requested) = if is_4k { (s.status_4k, s.requested_4k) } else { (s.status, s.requested) };
            // Seerr keeps a failed request's seasons until it is retried or deleted.
            !matches!(status, Some(Availability::Available | Availability::Partial | Availability::Processing))
                && !matches!(requested, Some(RequestState::Pending | RequestState::Approved | RequestState::Failed))
        })
        .cloned()
        .collect()
}

/// Whether a request button makes sense for this title in this quality.
pub fn requestable(d: &SeerrDetails, can: &SeerrCan, is_4k: bool) -> bool {
    if !can_request_kind(can, d.media_type, is_4k) {
        return false;
    }
    if d.media_type == SeerrMediaType::Tv {
        return !open_seasons(d, is_4k).is_empty();
    }
    is_new(if is_4k { d.status_4k } else { d.status })
}

/// What a season row says about it.
pub fn season_label(s: &SeerrSeason) -> Option<&'static str> {
    match s.status {
        Some(Availability::Available) => Some("Available"),
        Some(Availability::Partial) => Some("Partly available"),
        Some(Availability::Processing) => Some("Processing"),
        _ => s.requested.map(request_label),
    }
}

/// "Season 1" / "Seasons 1, 2", "4K", who and when, for a request row.
pub fn request_details(r: &SeerrRequest) -> String {
    let what = match r.media_type {
        SeerrMediaType::Tv if r.seasons.is_empty() => "Show".to_string(),
        SeerrMediaType::Tv => {
            let list = r.seasons.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ");
            format!("Season{} {list}", if r.seasons.len() == 1 { "" } else { "s" })
        }
        SeerrMediaType::Movie => "Movie".to_string(),
    };
    ui::dots([Some(what), r.is_4k.then(|| "4K".to_string()), Some(r.requested_by.clone()), r.created_at.as_deref().map(short_date)])
}

/// Once approved, how far the title got says more than the request itself.
pub fn request_shown(r: &SeerrRequest) -> &'static str {
    let progressed = matches!(r.status, Some(Availability::Processing | Availability::Partial | Availability::Available));
    if matches!(r.state, RequestState::Approved | RequestState::Completed) && progressed {
        availability_label(r.status).unwrap_or_else(|| request_label(r.state))
    } else {
        request_label(r.state)
    }
}

/// "Mar 4, 2024" from an ISO date or timestamp; the input as is when it isn't one.
pub fn short_date(iso: &str) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let mut parts = iso.get(..10).unwrap_or("").split('-');
    let (Some(y), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) else { return iso.to_string() };
    match (y.parse::<i32>(), m.parse::<usize>(), d.parse::<u32>()) {
        (Ok(y), Ok(m @ 1..=12), Ok(d)) => format!("{} {d}, {y}", MONTHS[m - 1]),
        _ => iso.to_string(),
    }
}

type StatusFuture = Shared<BoxFuture<'static, Result<SeerrStatus, String>>>;

/// The status (user and permissions), fetched once per instance and account and shared by the pages.
#[derive(Default)]
struct StatusCache(Option<(String, StatusFuture)>);

impl Global for StatusCache {}

/// Which Seerr instance and sign-in the status is for: the URL, the API key (if any) and the
/// Plex account (a Plex Home switch signs in to Seerr as someone else).
pub fn seerr_key(s: &crate::store::Store) -> String {
    format!("{}|{}|{}", s.settings.seerr.url.trim(), s.settings.seerr.api_key, s.scope())
}

fn status_key(cx: &App) -> Option<String> {
    let s = store(cx).read(cx);
    s.seerr.then(|| seerr_key(s))
}

/// The Seerr status, from the cache when it was asked for this instance and account before.
/// `None` when Seerr is not set up. A failure is not remembered: the next page asks again.
pub fn status(cx: &mut App) -> Option<impl Future<Output = Result<SeerrStatus, String>> + 'static> {
    let key = status_key(cx)?;
    if let Some((k, fut)) = cx.try_global::<StatusCache>().and_then(|c| c.0.clone())
        && k == key
    {
        return Some(fut);
    }
    let app = core(cx);
    let fut = bg(async move { seerr::status(&app).await.map_err(|e| e.to_string()) }).boxed().shared();
    cx.set_global(StatusCache(Some((key, fut.clone()))));
    Some(fut)
}

/// Forget a failed status so the next page asks again.
pub fn forget_status(cx: &mut App) {
    cx.set_global(StatusCache::default());
}

/// Poster card for a Seerr title (search, discover). Opens its Seerr page.
pub fn card(id: impl Into<ElementId>, t: &SeerrTitle, width: f32, cx: &App) -> AnyElement {
    let theme = cx.theme().clone();
    let tint = store(cx).read(cx).tint();
    let label = availability_label(t.status);
    let kind = if t.media_type == SeerrMediaType::Tv { "Show" } else { "Movie" };
    let id: ElementId = id.into();
    let group: SharedString = format!("seerr-card-{id:?}").into();
    let route = Route::Seerr { media_type: t.media_type.as_str().to_string(), tmdb_id: t.tmdb_id };
    let art = div()
        .relative()
        .w_full()
        .aspect_ratio(2. / 3.)
        .overflow_hidden()
        .rounded(px(10.))
        .bg(theme.colors.group_box)
        .border_1()
        .border_color(theme.foreground.opacity(0.1))
        .group_hover(group.clone(), |s| s.border_color(theme.foreground.opacity(0.25)))
        .child(match art_of(t.poster.as_deref(), 300., 450., None) {
            Some(img) => img.size_full().object_fit(ObjectFit::Cover).into_any_element(),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p_3()
                .text_center()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(t.title.clone())
                .into_any_element(),
        })
        .map(|d| {
            if t.status == Some(Availability::Available) {
                d.child(
                    div()
                        .absolute()
                        .top_2()
                        .right_2()
                        .size_5()
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(theme.background.opacity(0.8))
                        .text_color(tint)
                        .child(Icon::new(IconName::Check).size_3()),
                )
            } else if let Some(label) = label {
                d.child(
                    div()
                        .absolute()
                        .top_2()
                        .right_2()
                        .px_2()
                        .py_0p5()
                        .rounded(px(6.))
                        .bg(theme.background.opacity(0.8))
                        .text_xs()
                        .font_medium()
                        .when(t.status == Some(Availability::Partial), |d| d.text_color(tint))
                        .child(label),
                )
            } else {
                d
            }
        });
    div()
        .id(id)
        .group(group)
        .w(px(width))
        .flex_none()
        .min_w_0()
        .cursor_pointer()
        .on_click(move |_, _, cx| nav::push(route.clone(), cx))
        .child(art)
        .child(div().mt_2().text_sm().font_medium().truncate().child(t.title.clone()))
        .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(ui::dots([t.year.map(|y| y.to_string()), Some(kind.to_string())])))
        .into_any_element()
}

/// Marks a request busy (`true`) while its action runs, or done.
pub type Busy = Rc<dyn Fn(i64, bool, &mut App)>;

/// What a request row tells its page.
#[derive(Clone)]
pub struct RowEvents {
    /// A request's action started (`true`) or finished.
    pub busy: Busy,
    /// An action succeeded: reload.
    pub changed: Rc<dyn Fn(&mut App)>,
}

/// One Seerr request, with what the user may do to it: approve or decline (managers),
/// retry a failed one, or withdraw it. `linked` rows open the title's Seerr page.
pub fn request_row(r: &SeerrRequest, can: Option<SeerrCan>, linked: bool, first: bool, busy: bool, events: RowEvents, cx: &App) -> AnyElement {
    let theme = cx.theme().clone();
    let manage = can.is_some_and(|c| c.manage);
    let withdraw = manage || (r.mine && r.state == RequestState::Pending);
    let tone = if matches!(r.state, RequestState::Failed | RequestState::Declined) {
        Tone::Danger
    } else if r.status == Some(Availability::Available) {
        Tone::Tint
    } else {
        Tone::Plain
    };
    let id = r.id;
    let button = |name: &'static str, icon: IconName, tip: &'static str, action: RequestAction, done: &'static str| {
        let req = r.clone();
        let events = events.clone();
        Button::new((name, id as u64))
            .ghost()
            .small()
            .icon(icon)
            .tooltip(tip)
            .disabled(busy)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                run(&req, action, done, events.clone(), window, cx);
            })
    };
    let route = Route::Seerr { media_type: r.media_type.as_str().to_string(), tmdb_id: r.tmdb_id };
    widgets::list_row(first, cx)
        .id(("seerr-request", id as u64))
        .when(linked, |d| d.cursor_pointer().hover(|s| s.bg(theme.muted.opacity(0.4))).on_click(move |_, _, cx| nav::push(route.clone(), cx)))
        .child(
            div()
                .flex_none()
                .w(px(32.))
                .h(px(48.))
                .overflow_hidden()
                .rounded(px(4.))
                .bg(theme.muted)
                .children(art_of(r.poster.as_deref(), 32., 48., None).map(|i| i.size_full().object_fit(ObjectFit::Cover))),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    h_flex()
                        .min_w_0()
                        .text_sm()
                        .child(div().truncate().child(r.title.clone()))
                        .when_some(r.year, |d, y| d.child(div().ml_1().flex_none().text_color(theme.muted_foreground).child(format!("({y})")))),
                )
                .child(div().text_xs().text_color(theme.muted_foreground).truncate().child(request_details(r))),
        )
        .child(
            h_flex()
                .flex_none()
                .gap_1()
                .items_center()
                .child(ui::badge(request_shown(r), tone, cx))
                .when(manage && r.state == RequestState::Pending, |d| {
                    d.child(button("approve", IconName::Check, "Approve", RequestAction::Approve, "Request approved"))
                        .child(button("decline", IconName::X, "Decline", RequestAction::Decline, "Request declined"))
                })
                .when(manage && r.state == RequestState::Failed, |d| d.child(button("retry", IconName::RotateCcw, "Retry", RequestAction::Retry, "Sent again")))
                .when(withdraw, |d| d.child(button("delete", IconName::Trash, "Delete request", RequestAction::Delete, "Request deleted"))),
        )
        .into_any_element()
}

fn run(r: &SeerrRequest, action: RequestAction, done: &'static str, events: RowEvents, window: &mut Window, cx: &mut App) {
    let id = r.id;
    match action {
        RequestAction::Delete | RequestAction::Decline => {
            let delete = action == RequestAction::Delete;
            let title = if delete { format!("Delete the request for “{}”?", r.title) } else { format!("Decline “{}”?", r.title) };
            let text = if delete { "The request is removed from Seerr.".to_string() } else { format!("{} is told it was declined.", r.requested_by) };
            widgets::confirm(title, Some(text.into()), if delete { "Delete" } else { "Decline" }, true, window, cx, move |_, cx| {
                go(id, action, done, events.clone(), cx)
            });
        }
        _ => go(id, action, done, events, cx),
    }
}

fn go(id: i64, action: RequestAction, done: &'static str, events: RowEvents, cx: &mut App) {
    (events.busy)(id, true, cx);
    let app = core(cx);
    cx.spawn(async move |cx| {
        let result = bg(async move { seerr::request_action(&app, id, action).await }).await;
        cx.update(|cx| {
            (events.busy)(id, false, cx);
            match result {
                Ok(()) => {
                    store::toast(cx, done);
                    (events.changed)(cx);
                }
                Err(e) => store::error(cx, e.to_string()),
            }
        });
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::prelude::v1::test;

    fn season(number: i64, status: Option<Availability>, requested: Option<RequestState>) -> SeerrSeason {
        SeerrSeason { number, name: format!("Season {number}"), episodes: Some(8), air_date: None, status, status_4k: None, requested, requested_4k: None }
    }

    fn show(seasons: Vec<SeerrSeason>) -> SeerrDetails {
        SeerrDetails {
            base: SeerrTitle {
                media_type: SeerrMediaType::Tv,
                tmdb_id: 1,
                title: "Show".into(),
                year: None,
                overview: String::new(),
                poster: None,
                backdrop: None,
                rating: None,
                status: None,
                status_4k: None,
                rating_key: None,
            },
            tagline: String::new(),
            genres: vec![],
            runtime: None,
            tvdb_id: None,
            seasons,
            requests: vec![],
        }
    }

    const ALL: SeerrCan = SeerrCan { request_movie: true, request_tv: true, request_4k_movie: false, request_4k_tv: false, advanced: false, manage: false };

    #[test]
    fn only_untouched_seasons_are_open() {
        let d = show(vec![
            season(1, Some(Availability::Available), None),
            season(2, None, Some(RequestState::Failed)),
            season(3, None, Some(RequestState::Declined)),
            season(4, None, None),
        ]);
        let open: Vec<i64> = open_seasons(&d, false).iter().map(|s| s.number).collect();
        assert_eq!(open, vec![3, 4]);
        assert!(requestable(&d, &ALL, false));
        assert!(!requestable(&d, &ALL, true));
    }

    #[test]
    fn a_movie_is_requestable_until_someone_asks() {
        let mut d = show(vec![]);
        d.base.media_type = SeerrMediaType::Movie;
        assert!(requestable(&d, &ALL, false));
        d.base.status = Some(Availability::Deleted);
        assert!(requestable(&d, &ALL, false));
        d.base.status = Some(Availability::Pending);
        assert!(!requestable(&d, &ALL, false));
    }

    #[test]
    fn dates_read_like_the_web_app() {
        assert_eq!(short_date("2024-03-04T10:00:00.000Z"), "Mar 4, 2024");
        assert_eq!(short_date("2019-12-31"), "Dec 31, 2019");
        assert_eq!(short_date("soon"), "soon");
    }

    #[test]
    fn an_approved_request_shows_how_far_it_got() {
        let mut r = SeerrRequest {
            id: 1,
            state: RequestState::Approved,
            media_type: SeerrMediaType::Tv,
            tmdb_id: 1,
            title: "T".into(),
            year: None,
            poster: None,
            is_4k: true,
            seasons: vec![1, 2],
            requested_by: "Alice".into(),
            mine: false,
            created_at: Some("2024-03-04".into()),
            status: Some(Availability::Processing),
        };
        assert_eq!(request_shown(&r), "Processing");
        assert_eq!(request_details(&r), "Seasons 1, 2 · 4K · Alice · Mar 4, 2024");
        r.state = RequestState::Pending;
        assert_eq!(request_shown(&r), "Pending approval");
        r.seasons = vec![3];
        r.is_4k = false;
        assert_eq!(request_details(&r), "Season 3 · Alice · Mar 4, 2024");
    }
}
