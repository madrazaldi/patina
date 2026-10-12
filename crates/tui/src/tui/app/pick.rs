//! "Pick something to watch": criteria rows, a few random candidates, and a reroll.
//! The choosing is core's ([`patina_core::ops::pick`]); this is the screen around it.

use patina_core::ops::pick::{self, PickKind, PickOptions, PickQuery, PickResult};

use super::*;
use crate::tui::rows::{self, Act, PickField, Row, Tone};

/// The criteria, and what was last picked.
#[derive(Default)]
pub struct PickState {
    pub query: PickQuery,
    pub options: Option<PickOptions>,
    pub result: Option<PickResult>,
    pub loading: bool,
    pub error: Option<String>,
    /// Ignores a pick that a newer one has replaced.
    generation: u64,
}

fn join(list: &[String], empty: &str) -> String {
    if list.is_empty() { empty.to_string() } else { list.join(", ") }
}

/// "Comedy, Drama" → ["Comedy", "Drama"].
pub fn parse_list(text: &str) -> Vec<String> {
    text.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()
}

/// "1990-1999", "2005" (that year), "2010-" (from) or "-1999" (until); blank clears both.
pub fn parse_years(text: &str) -> Option<(Option<i64>, Option<i64>)> {
    let text = text.trim();
    if text.is_empty() {
        return Some((None, None));
    }
    let year = |s: &str| -> Option<Option<i64>> {
        let s = s.trim();
        if s.is_empty() { return Some(None) }
        s.parse::<i64>().ok().filter(|y| (1800..=2200).contains(y)).map(Some)
    };
    match text.split_once('-') {
        Some((from, to)) => Some((year(from)?, year(to)?)),
        None => year(text).map(|y| (y, y)),
    }
}

fn years_label(q: &PickQuery) -> String {
    match (q.year_min, q.year_max) {
        (None, None) => "any".into(),
        (Some(a), Some(b)) if a == b => a.to_string(),
        (Some(a), Some(b)) => format!("{a}–{b}"),
        (Some(a), None) => format!("{a} or later"),
        (None, Some(b)) => format!("{b} or earlier"),
    }
}

fn kind_label(kind: PickKind) -> &'static str {
    match kind {
        PickKind::Movie => "a film",
        PickKind::Show => "a show",
        PickKind::Any => "a film or a show",
    }
}

/// Rows and candidates of the screen.
pub fn screen(state: &PickState) -> Vec<Item> {
    let q = &state.query;
    let opts = state.options.as_ref();
    let setting = |field: PickField, title: &str, value: String, text: &str| {
        Row::new(format!("pick:{field:?}"), Act::Pick(field)).glyph("•", Tone::Accent).note(value.clone()).kv("Now", value).text(text).item(title)
    };
    let library = if q.sections.is_empty() {
        "all".to_string()
    } else {
        let names: Vec<String> = q
            .sections
            .iter()
            .filter_map(|id| opts.and_then(|o| o.sections.iter().find(|s| &s.id == id)).map(|s| s.title.clone()))
            .collect();
        join(&names, "all")
    };
    let genres_hint = match opts.map(|o| &o.genres).filter(|g| !g.is_empty()) {
        Some(g) => format!("Comma separated, any of them. Available: {}.", g.join(", ")),
        None => "Comma separated, any of them.".to_string(),
    };
    let ratings_hint = match opts.map(|o| &o.content_ratings).filter(|g| !g.is_empty()) {
        Some(g) => format!("Comma separated, any of them. Available: {}.", g.join(", ")),
        None => "Comma separated, any of them (PG-13, R, TV-MA…).".to_string(),
    };
    let mut out = vec![
        rows::heading("What do you feel like?"),
        Row::new("pick:Roll", Act::Pick(PickField::Roll))
            .glyph("↻", Tone::Good)
            .note(if state.loading { "picking…" } else { "enter" })
            .text("Offers a few candidates. Picking again skips the ones already shown (r does the same from anywhere on this screen).")
            .item(if state.result.is_some() { "Pick again" } else { "Pick something" }),
        setting(PickField::Kind, "Looking for", kind_label(q.kind).into(), "Enter switches between a film, a show, or either."),
        setting(PickField::Unwatched, "Only unwatched", if q.unwatched { "yes" } else { "no" }.into(), "Enter toggles. Shows count when they have an episode left."),
        setting(
            PickField::Length,
            "Shorter than",
            q.max_minutes.map_or("any".into(), |m| format!("{m} min")),
            "Minutes, for a film or one episode of a show. Leave empty for any length.",
        ),
        setting(PickField::Genres, "Genres", join(&q.genres, "any"), &genres_hint),
        setting(PickField::Ratings, "Content ratings", join(&q.content_ratings, "any"), &ratings_hint),
        setting(PickField::Years, "Years", years_label(q), "1990-1999, 2005 (that year), 2010- (from) or -1999 (until). Empty for any."),
        setting(PickField::MinRating, "Audience rating at least", q.min_rating.map_or("any".into(), |r| format!("{r:.1}")), "From 0 to 10. Leave empty for any."),
        setting(PickField::Library, "Library", library, "Enter moves through your movie and show libraries, then back to all of them."),
    ];
    out.push(rows::heading(&match (&state.result, &state.error) {
        (_, Some(_)) => "Couldn't pick".to_string(),
        (Some(r), _) if r.items.is_empty() => "Nothing fits".to_string(),
        (Some(r), _) if r.wrapped => format!("Candidates · all {} shown, starting over", r.matches),
        (Some(r), _) => format!("Candidates · {} of {} fit", r.items.len(), r.matches.max(r.items.len())),
        (None, None) if state.loading => "Picking…".to_string(),
        (None, None) => "Candidates".to_string(),
    }));
    if let Some(e) = &state.error {
        out.push(Row::new("pick:error", Act::Info).glyph("!", Tone::Error).note(e.clone()).kv("Error", e.clone()).item("Something went wrong"));
    } else if let Some(r) = &state.result {
        if r.items.is_empty() {
            out.push(Row::new("pick:none", Act::Info).glyph("·", Tone::Dim).text("Loosen a criterion — a shorter list of genres, a longer running time or no rating limit.").item("No match"));
        }
        out.extend(r.items.iter().cloned().map(Item::from));
    }
    out
}

impl App {
    /// The Pick screen was opened: load what the criteria can be chosen from, and pick once.
    pub(super) fn pick_opened(&mut self) {
        if self.pick.options.is_none() {
            let (core, tx) = (self.core.clone(), self.tx.clone());
            tokio::spawn(async move {
                let _ = tx.send(Msg::PickOptions(pick::options(&core).await.map_err(err)));
            });
        }
        if self.pick.result.is_none() && !self.pick.loading {
            self.roll(false);
        }
    }

    /// Pick again. A reroll skips what was already offered.
    pub(super) fn roll(&mut self, reroll: bool) {
        if !reroll {
            self.pick.query.exclude.clear();
        }
        self.pick.generation += 1;
        self.pick.loading = true;
        self.pick.error = None;
        let (core, tx, generation, query) = (self.core.clone(), self.tx.clone(), self.pick.generation, self.pick.query.clone());
        tokio::spawn(async move {
            let _ = tx.send(Msg::Picked { generation, result: pick::pick(&core, &query).await.map_err(err) });
        });
        self.refresh_local(&Source::Pick);
    }

    pub(super) fn on_picked(&mut self, generation: u64, result: Result<PickResult, String>) {
        if generation != self.pick.generation {
            return;
        }
        self.pick.loading = false;
        match result {
            Ok(r) => {
                if r.wrapped {
                    self.pick.query.exclude.clear();
                }
                self.pick.query.exclude.extend(r.items.iter().map(|m| m.rating_key().to_string()));
                self.pick.result = Some(r);
            }
            Err(e) => {
                self.pick.result = None;
                self.pick.error = Some(e);
            }
        }
        self.refresh_local(&Source::Pick);
        self.after_selection_change();
    }

    /// Enter on a criteria row.
    pub(super) fn pick_row(&mut self, field: PickField) {
        let q = &mut self.pick.query;
        let input = |title: &str, hint: &str, value: String| Modal::Input(Input { title: title.into(), hint: hint.into(), value, purpose: InputPurpose::Pick(field) });
        match field {
            PickField::Roll => return self.roll(self.pick.result.is_some()),
            PickField::Kind => {
                q.kind = match q.kind {
                    PickKind::Movie => PickKind::Show,
                    PickKind::Show => PickKind::Any,
                    PickKind::Any => PickKind::Movie,
                };
                q.sections.clear();
            }
            PickField::Unwatched => q.unwatched = !q.unwatched,
            PickField::Library => {
                let ids: Vec<String> = self
                    .pick
                    .options
                    .iter()
                    .flat_map(|o| &o.sections)
                    .filter(|s| match q.kind {
                        PickKind::Movie => s.kind == "movie",
                        PickKind::Show => s.kind == "show",
                        PickKind::Any => true,
                    })
                    .map(|s| s.id.clone())
                    .collect();
                // all → first → second → … → all
                let next = match q.sections.first().and_then(|cur| ids.iter().position(|i| i == cur)) {
                    None if q.sections.is_empty() => ids.first().cloned(),
                    Some(i) => ids.get(i + 1).cloned(),
                    None => None,
                };
                q.sections = next.into_iter().collect();
            }
            PickField::Length => {
                let value = q.max_minutes.map(|m| m.to_string()).unwrap_or_default();
                self.modal = Some(input("Shorter than (minutes)", "Empty for any length", value));
                return;
            }
            PickField::Genres => {
                self.modal = Some(input("Genres", "Comma separated; empty for any", q.genres.join(", ")));
                return;
            }
            PickField::Ratings => {
                self.modal = Some(input("Content ratings", "Comma separated (PG-13, R…); empty for any", q.content_ratings.join(", ")));
                return;
            }
            PickField::Years => {
                self.modal = Some(input("Years", "1990-1999, 2005, 2010- or -1999; empty for any", match (q.year_min, q.year_max) {
                    (None, None) => String::new(),
                    (Some(a), Some(b)) if a == b => a.to_string(),
                    (a, b) => format!("{}-{}", a.map(|y| y.to_string()).unwrap_or_default(), b.map(|y| y.to_string()).unwrap_or_default()),
                }));
                return;
            }
            PickField::MinRating => {
                self.modal = Some(input("Audience rating at least", "0 to 10; empty for any", q.min_rating.map(|r| r.to_string()).unwrap_or_default()));
                return;
            }
        }
        self.roll(false);
    }

    /// A criterion typed in the input dialog.
    pub(super) fn pick_submit(&mut self, field: PickField, text: &str) {
        let q = &mut self.pick.query;
        match field {
            PickField::Length => match text.trim() {
                "" => q.max_minutes = None,
                t => match t.parse::<u32>() {
                    Ok(m) if m > 0 => q.max_minutes = Some(m),
                    _ => return self.error("Minutes must be a whole number"),
                },
            },
            PickField::Genres => q.genres = parse_list(text),
            PickField::Ratings => q.content_ratings = parse_list(text),
            PickField::Years => match parse_years(text) {
                Some((a, b)) if a.zip(b).is_none_or(|(a, b)| a <= b) => (q.year_min, q.year_max) = (a, b),
                _ => return self.error("Years look like 1990-1999, 2005, 2010- or -1999"),
            },
            PickField::MinRating => match text.trim() {
                "" => q.min_rating = None,
                t => match t.parse::<f64>() {
                    Ok(r) if (0.0..=10.0).contains(&r) => q.min_rating = Some(r),
                    _ => return self.error("The rating must be a number from 0 to 10"),
                },
            },
            _ => return,
        }
        self.roll(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_split_on_commas() {
        assert_eq!(parse_list(" Comedy , Drama,,"), ["Comedy", "Drama"]);
        assert!(parse_list("  ").is_empty());
    }

    #[test]
    fn years_accept_ranges_and_open_ends() {
        assert_eq!(parse_years("1990-1999"), Some((Some(1990), Some(1999))));
        assert_eq!(parse_years("2005"), Some((Some(2005), Some(2005))));
        assert_eq!(parse_years("2010-"), Some((Some(2010), None)));
        assert_eq!(parse_years("-1999"), Some((None, Some(1999))));
        assert_eq!(parse_years(""), Some((None, None)));
        assert_eq!(parse_years("soon"), None);
        assert_eq!(parse_years("12"), None);
    }

    #[test]
    fn the_screen_lists_criteria_then_candidates() {
        let mut state = PickState::default();
        let rows = screen(&state);
        assert_eq!(rows[0].title(), "What do you feel like?");
        assert!(rows.iter().any(|r| r.title() == "Pick something"));
        let item: Metadata = serde_json::from_value(serde_json::json!({ "ratingKey": "5", "type": "movie", "title": "Copper Sky" })).unwrap();
        state.result = Some(PickResult { items: vec![item], matches: 9, remaining: 8, wrapped: false, libraries: vec!["Movies".into()] });
        let rows = screen(&state);
        assert!(rows.iter().any(|r| r.title() == "Pick again"));
        assert!(rows.iter().any(|r| r.title().starts_with("Candidates · 1 of 9")));
        assert_eq!(rows.last().map(|r| r.title()), Some("Copper Sky"));
        state.result = Some(PickResult { items: vec![], matches: 0, remaining: 0, wrapped: false, libraries: vec![] });
        assert!(screen(&state).iter().any(|r| r.title() == "Nothing fits"));
    }
}
