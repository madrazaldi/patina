//! "Pick something to watch": a few random candidates that fit what you feel like.
//!
//! The server does the heavy filtering through library query parameters
//! (unwatched, genre, content rating, year, rating, length, random order), so even
//! a large library only sends one page per section. Everything is checked again
//! here ([`matches`]), which keeps the result right when a server ignores a filter.

use std::collections::HashSet;

use anyhow::{Result, anyhow};
use futures::future::join_all;
use patina_plex::{Directory, LibraryQuery, MediaType, Metadata, Params, PlexServer};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

/// Candidates fetched per library; the random order makes each fetch a fresh sample.
const PER_SECTION: u32 = 120;
/// Used when the server refuses the numeric filters and everything is checked here.
const FALLBACK_PER_SECTION: u32 = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PickKind {
    #[default]
    Movie,
    Show,
    /// Movies and shows.
    Any,
}

impl PickKind {
    fn media_type(self, section_kind: &str) -> Option<MediaType> {
        match (self, section_kind) {
            (PickKind::Movie | PickKind::Any, "movie") => Some(MediaType::Movie),
            (PickKind::Show | PickKind::Any, "show") => Some(MediaType::Show),
            _ => None,
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "movie" | "movies" | "film" | "films" => Some(Self::Movie),
            "show" | "shows" | "series" | "tv" => Some(Self::Show),
            "any" | "all" => Some(Self::Any),
            _ => None,
        }
    }
}

/// What to watch. The defaults are an unwatched film, four candidates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PickQuery {
    pub kind: PickKind,
    /// Only things with something left to watch.
    pub unwatched: bool,
    /// Longest running time in minutes (a film, or one episode of a show).
    pub max_minutes: Option<u32>,
    pub min_minutes: Option<u32>,
    /// Any of these genres (by name).
    pub genres: Vec<String>,
    /// Any of these content ratings ("PG-13", "TV-MA", ...).
    pub content_ratings: Vec<String>,
    /// Library section ids; empty means every movie and show library.
    pub sections: Vec<String>,
    pub year_min: Option<i64>,
    pub year_max: Option<i64>,
    /// Lowest audience rating, 0 to 10.
    pub min_rating: Option<f64>,
    /// How many candidates to return.
    pub count: usize,
    /// Rating keys shown before, so a reroll offers something new.
    pub exclude: Vec<String>,
}

impl Default for PickQuery {
    fn default() -> Self {
        Self {
            kind: PickKind::Movie,
            unwatched: true,
            max_minutes: None,
            min_minutes: None,
            genres: Vec::new(),
            content_ratings: Vec::new(),
            sections: Vec::new(),
            year_min: None,
            year_max: None,
            min_rating: None,
            count: 4,
            exclude: Vec::new(),
        }
    }
}

impl PickQuery {
    fn count(&self) -> usize {
        self.count.clamp(1, 12)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickResult {
    pub items: Vec<Metadata>,
    /// How many titles fit the query in the libraries searched (a lower bound when a
    /// library has more than one sample page of them).
    pub matches: usize,
    /// How many of those have not been offered yet.
    pub remaining: usize,
    /// Everything that fits was already offered, so the choice started over.
    pub wrapped: bool,
    /// Names of the libraries that were searched.
    pub libraries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickSection {
    pub id: String,
    pub title: String,
    /// `movie` or `show`.
    pub kind: String,
}

/// The choices the form offers.
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PickOptions {
    pub sections: Vec<PickSection>,
    pub genres: Vec<String>,
    pub content_ratings: Vec<String>,
}

fn is_pickable(d: &Directory) -> bool {
    matches!(d.kind.as_deref(), Some("movie" | "show"))
}

/// Libraries, genres and content ratings to choose from.
pub async fn options(app: &AppState) -> Result<PickOptions> {
    let server = app.server()?;
    let sections: Vec<PickSection> = app
        .sections()
        .into_iter()
        .filter(is_pickable)
        .filter_map(|d| Some(PickSection { id: d.key.clone()?, title: d.title().to_string(), kind: d.kind.clone()? }))
        .collect();
    let lists = join_all(sections.iter().map(|s| {
        let server = server.clone();
        async move {
            let kind = if s.kind == "show" { MediaType::Show } else { MediaType::Movie }.id();
            let genres = server.section_filter_values(&s.id, "genre", Some(kind)).await.unwrap_or_default();
            let ratings = server.section_filter_values(&s.id, "contentRating", Some(kind)).await.unwrap_or_default();
            (genres, ratings)
        }
    }))
    .await;
    let mut genres: Vec<String> = Vec::new();
    let mut ratings: Vec<String> = Vec::new();
    for (g, r) in lists {
        for d in g {
            push_unique(&mut genres, d.title.or(d.tag));
        }
        for d in r {
            push_unique(&mut ratings, d.title.or(d.tag).or(d.key));
        }
    }
    genres.sort_by_key(|g| g.to_lowercase());
    ratings.sort_by_key(|r| rating_order(r));
    Ok(PickOptions { sections, genres, content_ratings: ratings })
}

fn push_unique(list: &mut Vec<String>, value: Option<String>) {
    if let Some(v) = value.filter(|v| !v.is_empty())
        && !list.iter().any(|x| x.eq_ignore_ascii_case(&v))
    {
        list.push(v);
    }
}

/// Orders ratings roughly from gentlest to strictest (G, PG, PG-13, R, TV-*, others).
fn rating_order(r: &str) -> (u8, String) {
    let rank = match r.to_ascii_uppercase().as_str() {
        "G" | "TV-Y" | "TV-G" => 0,
        "PG" | "TV-PG" | "TV-Y7" => 1,
        "PG-13" | "TV-14" => 2,
        "R" | "TV-MA" => 3,
        "NC-17" => 4,
        _ => 5,
    };
    (rank, r.to_lowercase())
}

/// Request parameters narrowing a section's listing on the server.
fn server_filters(q: &PickQuery, genre_ids: &[String], numeric: bool) -> Params {
    let mut p = Params::new();
    if !genre_ids.is_empty() {
        p.push("genre", genre_ids.join(","));
    }
    if !q.content_ratings.is_empty() {
        p.push("contentRating", q.content_ratings.join(","));
    }
    if numeric {
        // `key>>=value` means at least, `key<<=value` at most.
        if let Some(y) = q.year_min {
            p.push("year>>", y);
        }
        if let Some(y) = q.year_max {
            p.push("year<<", y);
        }
        if let Some(r) = q.min_rating {
            p.push("audienceRating>>", r);
        }
        if let Some(m) = q.max_minutes {
            p.push("duration<<", u64::from(m) * 60_000);
        }
        if let Some(m) = q.min_minutes.filter(|m| *m > 0) {
            p.push("duration>>", u64::from(m) * 60_000);
        }
    }
    p
}

/// The id Plex filters a genre by, from a `genre` filter value entry.
fn filter_id(d: &Directory) -> Option<String> {
    let key = d.key.as_deref()?;
    Some(key.rsplit_once("genre=").map_or(key, |(_, v)| v).to_string())
}

/// Genre filter ids for the genres named in the query, if this section has any of them.
async fn genre_ids(server: &PlexServer, section: &str, kind: MediaType, wanted: &[String]) -> Vec<String> {
    let values = server.section_filter_values(section, "genre", Some(kind.id())).await.unwrap_or_default();
    values
        .iter()
        .filter(|d| {
            let name = d.title.as_deref().or(d.tag.as_deref()).unwrap_or("");
            wanted.iter().any(|w| w.eq_ignore_ascii_case(name))
        })
        .filter_map(filter_id)
        .collect()
}

/// One library's candidates and how many titles fit in all.
async fn candidates(server: &PlexServer, section: &Directory, q: &PickQuery) -> Result<(Vec<Metadata>, usize)> {
    let id = section.key.clone().ok_or_else(|| anyhow!("library without an id"))?;
    let Some(kind) = q.kind.media_type(section.kind.as_deref().unwrap_or("")) else {
        return Ok((Vec::new(), 0));
    };
    let ids = if q.genres.is_empty() { Vec::new() } else { genre_ids(server, &id, kind, &q.genres).await };
    if !q.genres.is_empty() && ids.is_empty() {
        // No genre of this library is among the wanted ones.
        return Ok((Vec::new(), 0));
    }
    let fetch = |numeric: bool, size: u32| {
        let query = LibraryQuery {
            kind: Some(kind.id()),
            sort: Some("random".into()),
            unwatched: q.unwatched,
            filters: server_filters(q, &ids, numeric),
            start: 0,
            size,
        };
        let server = server.clone();
        let id = id.clone();
        async move { server.section_items(&id, &query).await }
    };
    let mc = match fetch(true, PER_SECTION).await {
        Ok(mc) => mc,
        // A server that rejects the numeric filters still gets a (larger) unfiltered sample.
        Err(e) => {
            tracing::debug!("pick filters refused by library {id}: {e}");
            fetch(false, FALLBACK_PER_SECTION).await?
        }
    };
    let total = mc.total_size.map_or(mc.metadata.len(), |t| t.max(0) as usize);
    Ok((mc.metadata, total))
}

/// Whether an item fits the query. The same checks the server was asked to make.
pub fn matches(item: &Metadata, q: &PickQuery) -> bool {
    match (q.kind, item.kind()) {
        (PickKind::Movie, "movie") | (PickKind::Show, "show") | (PickKind::Any, "movie" | "show") => {}
        _ => return false,
    }
    if q.unwatched && !has_unwatched(item) {
        return false;
    }
    if q.max_minutes.is_some() || q.min_minutes.is_some() {
        let Some(minutes) = item.duration.filter(|d| *d > 0).map(|d| d as f64 / 60_000.0) else { return false };
        if q.max_minutes.is_some_and(|m| minutes > f64::from(m)) || q.min_minutes.is_some_and(|m| minutes < f64::from(m)) {
            return false;
        }
    }
    if !q.genres.is_empty() && !item.genres.iter().any(|g| q.genres.iter().any(|w| w.eq_ignore_ascii_case(g.tag.as_deref().unwrap_or("")))) {
        return false;
    }
    if !q.content_ratings.is_empty() {
        let rating = item.content_rating.as_deref().unwrap_or("");
        if !q.content_ratings.iter().any(|w| w.eq_ignore_ascii_case(rating)) {
            return false;
        }
    }
    if q.year_min.is_some() || q.year_max.is_some() {
        let Some(year) = item.year else { return false };
        if q.year_min.is_some_and(|y| year < y) || q.year_max.is_some_and(|y| year > y) {
            return false;
        }
    }
    if let Some(min) = q.min_rating
        && item.audience_rating.or(item.rating).is_none_or(|r| r < min)
    {
        return false;
    }
    true
}

fn has_unwatched(item: &Metadata) -> bool {
    match item.kind() {
        "show" => item.leaf_count.unwrap_or(1) > item.viewed_leaf_count.unwrap_or(0),
        _ => item.view_count.unwrap_or(0) == 0,
    }
}

/// Choose from the candidates that fit: not offered before, in random order. When
/// everything that fits was offered already, the choice starts over (`wrapped`).
/// Returns the chosen items, how many fit, and how many of those are still unseen.
pub fn choose(mut fits: Vec<Metadata>, q: &PickQuery, rng: &mut fastrand::Rng) -> (Vec<Metadata>, usize, usize, bool) {
    let mut seen = HashSet::new();
    fits.retain(|m| seen.insert(m.rating_key().to_string()));
    let matches = fits.len();
    let excluded: HashSet<&str> = q.exclude.iter().map(String::as_str).collect();
    let (mut fresh, mut stale): (Vec<Metadata>, Vec<Metadata>) = fits.into_iter().partition(|m| !excluded.contains(m.rating_key()));
    let wrapped = fresh.is_empty() && !stale.is_empty();
    let remaining = fresh.len();
    let pool = if wrapped { &mut stale } else { &mut fresh };
    rng.shuffle(pool);
    pool.truncate(q.count());
    (std::mem::take(pool), matches, remaining, wrapped)
}

/// Pick a few things to watch.
pub async fn pick(app: &AppState, q: &PickQuery) -> Result<PickResult> {
    let server = app.server()?;
    let sections: Vec<Directory> = app
        .sections()
        .into_iter()
        .filter(is_pickable)
        .filter(|d| q.sections.is_empty() || d.key.as_ref().is_some_and(|k| q.sections.contains(k)))
        .filter(|d| q.kind.media_type(d.kind.as_deref().unwrap_or("")).is_some())
        .collect();
    if sections.is_empty() {
        return Err(anyhow!("There is no movie or show library to pick from."));
    }
    let fetched = join_all(sections.iter().map(|s| candidates(&server, s, q))).await;
    let mut fits = Vec::new();
    let mut total = 0;
    let mut libraries = Vec::new();
    let mut failure = None;
    for (section, result) in sections.iter().zip(fetched) {
        match result {
            Ok((items, count)) => {
                libraries.push(section.title().to_string());
                // The server's count covers the whole library; the sample is checked here.
                let kept = items.into_iter().filter(|m| matches(m, q)).collect::<Vec<_>>();
                total += if kept.is_empty() { 0 } else { count.max(kept.len()) };
                fits.extend(kept);
            }
            Err(e) => failure = Some(e),
        }
    }
    if libraries.is_empty() {
        return Err(failure.unwrap_or_else(|| anyhow!("Nothing could be searched.")));
    }
    let (items, fit, fresh, wrapped) = choose(fits, q, &mut fastrand::Rng::new());
    // A library with more matches than one sample page reports its real count.
    let matches = total.max(fit);
    let remaining = if wrapped { matches } else { matches.saturating_sub(fit - fresh) };
    Ok(PickResult { items, matches, remaining, wrapped, libraries })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn movie(rk: &str, minutes: i64, year: i64, genre: &str, rating: &str, audience: f64, views: i64) -> Metadata {
        serde_json::from_value(json!({
            "ratingKey": rk, "type": "movie", "title": format!("Movie {rk}"), "year": year,
            "duration": minutes * 60_000, "contentRating": rating, "audienceRating": audience, "viewCount": views,
            "Genre": [{ "tag": genre }]
        }))
        .unwrap()
    }

    fn show(rk: &str, leaves: i64, viewed: i64) -> Metadata {
        serde_json::from_value(json!({ "ratingKey": rk, "type": "show", "title": "S", "leafCount": leaves, "viewedLeafCount": viewed, "duration": 45 * 60_000 })).unwrap()
    }

    #[test]
    fn unwatched_short_film_with_genre_and_rating() {
        let q = PickQuery {
            max_minutes: Some(100),
            genres: vec!["comedy".into()],
            content_ratings: vec!["PG".into(), "PG-13".into()],
            min_rating: Some(7.0),
            year_min: Some(2000),
            year_max: Some(2020),
            ..Default::default()
        };
        assert!(matches(&movie("1", 95, 2010, "Comedy", "PG", 7.5, 0), &q));
        assert!(!matches(&movie("2", 101, 2010, "Comedy", "PG", 7.5, 0), &q), "too long");
        assert!(!matches(&movie("3", 95, 2010, "Drama", "PG", 7.5, 0), &q), "wrong genre");
        assert!(!matches(&movie("4", 95, 2010, "Comedy", "R", 7.5, 0), &q), "wrong rating");
        assert!(!matches(&movie("5", 95, 1999, "Comedy", "PG", 7.5, 0), &q), "too old");
        assert!(!matches(&movie("6", 95, 2021, "Comedy", "PG", 7.5, 0), &q), "too new");
        assert!(!matches(&movie("7", 95, 2010, "Comedy", "PG", 6.9, 0), &q), "rated too low");
        assert!(!matches(&movie("8", 95, 2010, "Comedy", "PG", 7.5, 2), &q), "already watched");
    }

    #[test]
    fn filters_off_accept_everything_of_the_kind() {
        let q = PickQuery { unwatched: false, ..Default::default() };
        assert!(matches(&movie("1", 300, 1950, "Drama", "R", 1.0, 5), &q));
        assert!(!matches(&show("2", 10, 0), &q), "shows are not movies");
        let any = PickQuery { kind: PickKind::Any, ..q };
        assert!(matches(&show("2", 10, 0), &any));
    }

    #[test]
    fn a_show_with_episodes_left_counts_as_unwatched() {
        let q = PickQuery { kind: PickKind::Show, ..Default::default() };
        assert!(matches(&show("1", 10, 0), &q));
        assert!(matches(&show("2", 10, 4), &q));
        assert!(!matches(&show("3", 10, 10), &q));
        let short = PickQuery { kind: PickKind::Show, max_minutes: Some(30), ..Default::default() };
        assert!(!matches(&show("4", 10, 0), &short), "45 minute episodes");
    }

    #[test]
    fn items_without_a_length_fail_a_length_filter() {
        let mut m = movie("1", 90, 2010, "Drama", "PG", 7.0, 0);
        m.duration = None;
        assert!(matches(&m, &PickQuery::default()));
        assert!(!matches(&m, &PickQuery { max_minutes: Some(120), ..Default::default() }));
    }

    #[test]
    fn choose_returns_the_requested_count_without_repeats() {
        let fits: Vec<_> = (0..20).map(|i| movie(&i.to_string(), 90, 2010, "Drama", "PG", 7.0, 0)).collect();
        let q = PickQuery { count: 4, ..Default::default() };
        let (items, matches, remaining, wrapped) = choose(fits, &q, &mut fastrand::Rng::with_seed(7));
        assert_eq!((items.len(), matches, remaining, wrapped), (4, 20, 20, false));
        let keys: HashSet<_> = items.iter().map(|m| m.rating_key().to_string()).collect();
        assert_eq!(keys.len(), 4);
    }

    #[test]
    fn reroll_skips_what_was_shown_then_starts_over() {
        let fits = || -> Vec<Metadata> { (0..5).map(|i| movie(&i.to_string(), 90, 2010, "Drama", "PG", 7.0, 0)).collect() };
        let mut rng = fastrand::Rng::with_seed(1);
        let mut q = PickQuery { count: 3, ..Default::default() };
        let (first, ..) = choose(fits(), &q, &mut rng);
        q.exclude = first.iter().map(|m| m.rating_key().to_string()).collect();
        let (second, matches, remaining, wrapped) = choose(fits(), &q, &mut rng);
        assert_eq!((second.len(), matches, remaining, wrapped), (2, 5, 2, false));
        assert!(second.iter().all(|m| !q.exclude.contains(&m.rating_key().to_string())));
        q.exclude.extend(second.iter().map(|m| m.rating_key().to_string()));
        let (third, _, _, wrapped) = choose(fits(), &q, &mut rng);
        assert!(wrapped, "everything was shown, so it starts over");
        assert_eq!(third.len(), 3);
    }

    #[test]
    fn choose_handles_nothing_and_duplicates() {
        let q = PickQuery::default();
        let (items, matches, remaining, wrapped) = choose(Vec::new(), &q, &mut fastrand::Rng::new());
        assert_eq!((items.len(), matches, remaining, wrapped), (0, 0, 0, false));
        let same = movie("1", 90, 2010, "Drama", "PG", 7.0, 0);
        let (items, matches, ..) = choose(vec![same.clone(), same], &q, &mut fastrand::Rng::new());
        assert_eq!((items.len(), matches), (1, 1));
    }

    #[test]
    fn server_filters_cover_every_criterion() {
        let q = PickQuery {
            max_minutes: Some(100),
            content_ratings: vec!["PG".into(), "R".into()],
            year_min: Some(1990),
            year_max: Some(1999),
            min_rating: Some(6.5),
            ..Default::default()
        };
        let p = server_filters(&q, &["3".into(), "9".into()], true).to_query_string();
        for part in ["genre=3%2C9", "contentRating=PG%2CR", "year%3E%3E=1990", "year%3C%3C=1999", "audienceRating%3E%3E=6.5", "duration%3C%3C=6000000"] {
            assert!(p.contains(part), "{part} missing from {p}");
        }
        assert_eq!(server_filters(&q, &[], false).to_query_string(), "contentRating=PG%2CR");
    }

    #[test]
    fn ratings_sort_from_gentle_to_strict() {
        let mut r = vec!["R".to_string(), "Unrated".into(), "G".into(), "PG-13".into(), "PG".into()];
        r.sort_by_key(|x| rating_order(x));
        assert_eq!(r, ["G", "PG", "PG-13", "R", "Unrated"]);
    }

    #[test]
    fn genre_filter_value_ids() {
        let plain = Directory { key: Some("12".into()), ..Default::default() };
        let full = Directory { key: Some("/library/sections/1/all?genre=12".into()), ..Default::default() };
        assert_eq!(filter_id(&plain).as_deref(), Some("12"));
        assert_eq!(filter_id(&full).as_deref(), Some("12"));
    }

    #[test]
    fn kind_names_parse() {
        assert_eq!(PickKind::parse("Film"), Some(PickKind::Movie));
        assert_eq!(PickKind::parse("tv"), Some(PickKind::Show));
        assert_eq!(PickKind::parse("any"), Some(PickKind::Any));
        assert_eq!(PickKind::parse("music"), None);
    }
}
