//! Smart downloads: keep the next few unwatched episodes of a show, season or
//! playlist downloaded, and clean up after watching.
//!
//! A [`Policy`] belongs to one account on one server. [`reconcile`] compares
//! what each enabled policy wants with what is downloaded and queues or
//! removes files to match. It only ever looks at the connected server and
//! account, and it only removes downloads a policy fetched itself
//! ([`Download::policy`]); downloads someone asked for by hand are never
//! touched. An optional storage budget limits how much a policy may add.
//!
//! Reconciling runs on sign-in, whenever watch state may have changed (after
//! playback, a scrobble, an offline sync or a live server update, through
//! [`AppState::bump`]) and every few minutes.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, ensure};
use patina_plex::{Metadata, MetadataOptions, PlexServer};
use serde::{Deserialize, Serialize};

use crate::downloads::{self, Download};
use crate::state::{AppState, CoreEvent};
use crate::{config, offline, storage};

/// What a policy keeps downloaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Target {
    Show,
    Season,
    Playlist,
}

impl Target {
    pub fn label(self) -> &'static str {
        match self {
            Self::Show => "show",
            Self::Season => "season",
            Self::Playlist => "playlist",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub id: String,
    /// Account and server the policy belongs to; it is inert anywhere else.
    pub account: String,
    pub server_id: String,
    pub server_name: String,
    /// The show, season or playlist.
    pub rating_key: String,
    pub target: Target,
    pub title: String,
    /// How many unwatched items to keep downloaded.
    pub keep: u32,
    /// Remove downloads this policy fetched once they are watched.
    pub delete_watched: bool,
    pub enabled: bool,
}

pub const MAX_KEEP: u32 = 50;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    /// Most bytes all downloads together may use before policies stop adding.
    budget: Option<u64>,
    policies: Vec<Policy>,
}

fn path() -> PathBuf {
    config::config_dir().join("smart-downloads.json")
}

fn load() -> Saved {
    storage::read_json(&path()).unwrap_or_else(|e| {
        tracing::error!("smart downloads: {e:#}");
        Saved::default()
    })
}

fn update<R>(f: impl FnOnce(&mut Saved) -> R) -> Result<R> {
    storage::update_json(&path(), f)
}

/// The saved policies as text, to notice another process changing them.
pub fn fingerprint() -> String {
    std::fs::read_to_string(path()).unwrap_or_default()
}

fn notify(app: &AppState) {
    app.emit(CoreEvent::Downloads(downloads::list()));
}

// Evaluation -----------------------------------------------------------------

/// One item a policy could keep, in the order it would be watched.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub rating_key: String,
    /// Id of its download.
    pub id: String,
    pub watched: bool,
    pub size: i64,
}

/// A policy together with its items, as fetched from the server.
pub struct Fetched<'a> {
    pub policy: &'a Policy,
    pub candidates: Vec<Candidate>,
}

/// A download of the connected account and server.
#[derive(Clone, Debug, PartialEq)]
pub struct Held {
    pub id: String,
    pub policy: Option<String>,
    pub size: i64,
}

/// What to change.
#[derive(Debug, Default, PartialEq)]
pub struct Plan {
    /// `(policy id, rating key)` to download, in order.
    pub add: Vec<(String, String)>,
    /// Download ids to remove.
    pub remove: Vec<String>,
    /// Downloads whose owner is gone: adopted by a policy that wants them, or released.
    pub assign: Vec<(String, Option<String>)>,
    /// Items left out because the budget is full.
    pub over_budget: usize,
}

/// Work out what the policies need.
///
/// - Each policy wants the first `keep` unwatched items.
/// - A download a policy fetched is removed when no policy wants it any more
///   and it is not a watched item that policy is told to keep. Downloads of
///   policies that are disabled, or whose items could not be fetched, are left alone.
/// - New items are added in order until `budget` (counting `outside_bytes`, the
///   downloads of other servers and accounts, and removals just freed) is reached.
pub fn evaluate(fetched: &[Fetched], held: &[Held], known: &HashSet<String>, outside_bytes: u64, budget: Option<u64>) -> Plan {
    let mut plan = Plan::default();
    let evaluated: HashSet<&str> = fetched.iter().map(|f| f.policy.id.as_str()).collect();

    let mut wanted: HashMap<&str, &str> = HashMap::new();
    let mut keep_watched: HashSet<&str> = HashSet::new();
    let mut windows: Vec<Vec<&Candidate>> = Vec::new();
    for f in fetched {
        let window: Vec<&Candidate> = f.candidates.iter().filter(|c| !c.watched).take(f.policy.keep as usize).collect();
        for c in &window {
            wanted.entry(c.id.as_str()).or_insert(f.policy.id.as_str());
        }
        if !f.policy.delete_watched {
            keep_watched.extend(f.candidates.iter().filter(|c| c.watched).map(|c| c.id.as_str()));
        }
        windows.push(window);
    }

    let mut used = outside_bytes;
    let mut present: HashSet<&str> = HashSet::new();
    for h in held {
        let size = h.size.max(0) as u64;
        match h.policy.as_deref() {
            Some(owner) if evaluated.contains(owner) && !wanted.contains_key(h.id.as_str()) && !keep_watched.contains(h.id.as_str()) => {
                plan.remove.push(h.id.clone());
                continue;
            }
            Some(owner) if !known.contains(owner) => {
                plan.assign.push((h.id.clone(), wanted.get(h.id.as_str()).map(|p| p.to_string())));
            }
            _ => {}
        }
        used += size;
        present.insert(h.id.as_str());
    }

    for (f, window) in fetched.iter().zip(&windows) {
        for c in window {
            if present.contains(c.id.as_str()) {
                continue;
            }
            let size = c.size.max(0) as u64;
            if budget.is_some_and(|b| used + size > b) {
                // Later items must not jump the queue.
                plan.over_budget += window.iter().skip_while(|w| w.id != c.id).filter(|w| !present.contains(w.id.as_str())).count();
                break;
            }
            used += size;
            present.insert(c.id.as_str());
            plan.add.push((f.policy.id.clone(), c.rating_key.clone()));
        }
    }
    plan
}

/// Put the items of a policy in the order they are watched: a show's
/// episodes by season and number (specials excluded), a season's by number, a
/// playlist as it is.
pub fn watch_order(target: Target, mut items: Vec<Metadata>) -> Vec<Metadata> {
    match target {
        Target::Show => {
            items.retain(|m| m.parent_index != Some(0));
            items.sort_by_key(|m| (m.parent_index.unwrap_or(0), m.index.unwrap_or(0)));
        }
        Target::Season => items.sort_by_key(|m| m.index.unwrap_or(0)),
        Target::Playlist => {}
    }
    items
}

fn candidates(items: &[Metadata], scope: &crate::prefs::Scope, offline_watched: &HashSet<String>) -> Vec<Candidate> {
    items
        .iter()
        .filter(|m| m.is_playable() && !m.media.is_empty())
        .map(|m| {
            let id = downloads::id_for(&scope.server, &scope.account, m.rating_key());
            Candidate {
                rating_key: m.rating_key().to_string(),
                watched: m.is_watched() || offline_watched.contains(&id),
                size: m.media.first().and_then(|m| m.parts.first()).and_then(|p| p.size).unwrap_or(0),
                id,
            }
        })
        .collect()
}

/// Downloads of this account and server, and the bytes of all the others.
fn split(all: &[Download], scope: &crate::prefs::Scope) -> (Vec<Held>, u64) {
    let (mut held, mut outside) = (Vec::new(), 0u64);
    for d in all {
        if d.server_id == scope.server && d.account == scope.account {
            held.push(Held { id: d.id.clone(), policy: d.policy.clone(), size: d.size });
        } else {
            outside += d.size.max(0) as u64;
        }
    }
    (held, outside)
}

fn in_scope(p: &Policy, scope: &crate::prefs::Scope) -> bool {
    p.account == scope.account && p.server_id == scope.server
}

// Reconciling ----------------------------------------------------------------

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub added: usize,
    pub removed: usize,
    /// Items not queued because the storage budget is full.
    pub over_budget: usize,
    pub errors: Vec<String>,
}

static RUNNING: AtomicBool = AtomicBool::new(false);
static PENDING: AtomicBool = AtomicBool::new(false);
static LAST: Mutex<Option<Instant>> = Mutex::new(None);

struct Running;
impl Running {
    fn start() -> Option<Self> {
        (!RUNNING.swap(true, Ordering::SeqCst)).then_some(Running)
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::SeqCst);
    }
}

async fn fetch(server: &PlexServer, p: &Policy) -> Result<Vec<Metadata>> {
    Ok(match p.target {
        Target::Show => server.all_leaves(&p.rating_key).await?.metadata,
        Target::Season => server.children(&p.rating_key).await?.metadata,
        Target::Playlist => server.playlist_items(&p.rating_key, 0, 5000).await?.metadata,
    })
}

/// Make the downloads match the policies of the connected account and server.
pub async fn reconcile(app: &AppState) -> Result<Report> {
    let mut report = Report::default();
    let Some(session) = app.session() else { return Ok(report) };
    let Some(_running) = Running::start() else { return Ok(report) };
    // Another Patina process may be reconciling the same files.
    let Some(_lease) = storage::try_lock(&config::config_dir().join("smart-downloads-run"))? else { return Ok(report) };
    *LAST.lock().unwrap() = Some(Instant::now());

    let scope = crate::prefs::Scope { account: offline::account(&session), server: session.server_id.clone() };
    let saved = load();
    let mine: Vec<&Policy> = saved.policies.iter().filter(|p| in_scope(p, &scope)).collect();
    let known: HashSet<String> = mine.iter().map(|p| p.id.clone()).collect();
    let offline_watched = offline::watched_offline();

    let mut items: Vec<(&Policy, Vec<Metadata>)> = Vec::new();
    for p in mine.iter().copied().filter(|p| p.enabled) {
        match fetch(&session.server, p).await {
            Ok(list) => items.push((p, watch_order(p.target, list))),
            // Never remove anything on a guess: this policy sits the round out.
            Err(e) => report.errors.push(format!("{}: {e}", p.title)),
        }
    }
    // The session may have changed while the server was asked.
    if !app.session().is_some_and(|s| s.server == session.server) {
        return Ok(report);
    }

    let all = downloads::list();
    let (held, outside) = split(&all, &scope);
    let fetched: Vec<Fetched> =
        items.iter().map(|(p, list)| Fetched { policy: p, candidates: candidates(list, &scope, &offline_watched) }).collect();
    let plan = evaluate(&fetched, &held, &known, outside, saved.budget);
    report.over_budget = plan.over_budget;

    if !plan.assign.is_empty() {
        downloads::assign(&plan.assign)?;
    }
    let playing = app.now_playing().filter(|n| n.local).and_then(|n| n.item.rating_key);
    for id in &plan.remove {
        // A file being played is cleaned up on a later round.
        if all.iter().any(|d| &d.id == id && playing.as_deref() == d.item.rating_key.as_deref()) {
            continue;
        }
        downloads::remove(app, id);
        report.removed += 1;
    }
    let mut by_policy: Vec<(&str, Vec<Metadata>)> = Vec::new();
    for (policy, rating_key) in &plan.add {
        let Some(item) = items.iter().flat_map(|(_, l)| l).find(|m| m.rating_key() == rating_key) else { continue };
        match by_policy.iter_mut().find(|(p, _)| p == policy) {
            Some((_, list)) => list.push(item.clone()),
            None => by_policy.push((policy, vec![item.clone()])),
        }
    }
    for (policy, list) in by_policy {
        report.added += downloads::add(app, &session, list, Some(policy))?;
    }
    if report.added + report.removed + plan.assign.len() > 0 {
        notify(app);
    }
    Ok(report)
}

/// Reconcile shortly, coalescing requests; no more than once every 20 seconds.
pub fn reconcile_soon(app: &AppState) {
    if PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let wait = {
        let since = LAST.lock().unwrap().map(|t| t.elapsed()).unwrap_or(Duration::from_secs(3600));
        Duration::from_secs(20).saturating_sub(since).max(Duration::from_secs(2))
    };
    let app = app.clone();
    app.clone().spawn(async move {
        tokio::time::sleep(wait).await;
        PENDING.store(false, Ordering::SeqCst);
        match reconcile(&app).await {
            Ok(r) => r.errors.iter().for_each(|e| tracing::debug!("smart downloads: {e}")),
            Err(e) => tracing::debug!("smart downloads: {e:#}"),
        }
    });
}

// Managing policies ----------------------------------------------------------

/// A policy as front ends show it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyView {
    #[serde(flatten)]
    pub policy: Policy,
    /// Belongs to the connected account and server (others wait until you switch to them).
    pub active: bool,
    /// Files this policy fetched, and their size.
    pub downloads: usize,
    pub bytes: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub policies: Vec<PolicyView>,
    /// Storage budget in bytes; none when unlimited.
    pub budget: Option<u64>,
    /// Bytes of every download on this computer.
    pub used: u64,
}

pub fn overview(app: &AppState) -> Overview {
    let saved = load();
    let all = downloads::list();
    let scope = crate::prefs::Scope::of(app);
    let policies = saved
        .policies
        .into_iter()
        .map(|policy| {
            let mine = all.iter().filter(|d| d.policy.as_deref() == Some(policy.id.as_str()));
            let (downloads, bytes) = mine.fold((0, 0), |(n, b), d| (n + 1, b + d.size));
            PolicyView { active: scope.as_ref().is_some_and(|s| in_scope(&policy, s)), policy, downloads, bytes }
        })
        .collect();
    Overview { policies, budget: saved.budget, used: all.iter().map(|d| d.size.max(0) as u64).sum() }
}

fn target_of(item: &Metadata) -> Result<Target> {
    match item.kind() {
        "show" => Ok(Target::Show),
        "season" => Ok(Target::Season),
        "playlist" => Ok(Target::Playlist),
        _ => Err(anyhow!("Smart downloads work on shows, seasons and playlists.")),
    }
}

fn title_of(item: &Metadata) -> String {
    match item.kind() {
        "season" => format!("{} · {}", item.parent_title.as_deref().unwrap_or("Show"), item.title()),
        _ => item.title().to_string(),
    }
}

/// Keep the next `keep` unwatched items of a show, season or playlist
/// downloaded. Replaces the settings of an existing policy for it.
pub async fn set_policy(app: &AppState, rating_key: &str, keep: u32, delete_watched: bool) -> Result<Policy> {
    ensure!((1..=MAX_KEEP).contains(&keep), "Keep between 1 and {MAX_KEEP} items.");
    let session = app.session().ok_or_else(|| anyhow!("Not connected."))?;
    let item = session.server.metadata(rating_key, MetadataOptions::default()).await?.ok_or_else(|| anyhow!("Item not found."))?;
    let target = target_of(&item)?;
    let scope = crate::prefs::Scope { account: offline::account(&session), server: session.server_id.clone() };
    let new = Policy {
        id: uuid::Uuid::new_v4().to_string(),
        account: scope.account.clone(),
        server_id: scope.server.clone(),
        server_name: session.server_name.clone(),
        rating_key: rating_key.to_string(),
        target,
        title: title_of(&item),
        keep,
        delete_watched,
        enabled: true,
    };
    let policy = update(|saved| match saved.policies.iter_mut().find(|p| in_scope(p, &scope) && p.rating_key == rating_key) {
        Some(existing) => {
            existing.keep = keep;
            existing.delete_watched = delete_watched;
            existing.enabled = true;
            existing.title = new.title;
            existing.clone()
        }
        None => {
            saved.policies.push(new.clone());
            new
        }
    })?;
    notify(app);
    reconcile_soon(app);
    Ok(policy)
}

/// What to change in a policy; unset fields stay.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Patch {
    pub keep: Option<u32>,
    pub delete_watched: Option<bool>,
    pub enabled: Option<bool>,
}

pub fn update_policy(app: &AppState, id: &str, patch: Patch) -> Result<()> {
    if let Some(keep) = patch.keep {
        ensure!((1..=MAX_KEEP).contains(&keep), "Keep between 1 and {MAX_KEEP} items.");
    }
    let found = update(|saved| {
        let p = saved.policies.iter_mut().find(|p| p.id == id)?;
        p.keep = patch.keep.unwrap_or(p.keep);
        p.delete_watched = patch.delete_watched.unwrap_or(p.delete_watched);
        p.enabled = patch.enabled.unwrap_or(p.enabled);
        Some(())
    })?;
    ensure!(found.is_some(), "That policy no longer exists.");
    notify(app);
    reconcile_soon(app);
    Ok(())
}

/// Delete a policy. Its downloads stay as ordinary downloads, or are removed
/// too with `delete_downloads`.
pub fn remove_policy(app: &AppState, id: &str, delete_downloads: bool) -> Result<()> {
    update(|saved| saved.policies.retain(|p| p.id != id))?;
    let owned: Vec<String> = downloads::list().into_iter().filter(|d| d.policy.as_deref() == Some(id)).map(|d| d.id).collect();
    if delete_downloads {
        for download in &owned {
            downloads::remove(app, download);
        }
    } else {
        downloads::assign(&owned.into_iter().map(|d| (d, None)).collect::<Vec<_>>())?;
    }
    notify(app);
    Ok(())
}

/// Limit all downloads together to `bytes`; `None` removes the limit.
pub fn set_budget(app: &AppState, bytes: Option<u64>) -> Result<()> {
    update(|saved| saved.budget = bytes.filter(|b| *b > 0))?;
    notify(app);
    reconcile_soon(app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(id: &str, keep: u32, delete_watched: bool) -> Policy {
        Policy {
            id: id.into(),
            account: "alice".into(),
            server_id: "home".into(),
            server_name: "Home".into(),
            rating_key: format!("show-{id}"),
            target: Target::Show,
            title: id.into(),
            keep,
            delete_watched,
            enabled: true,
        }
    }

    fn episodes(prefix: &str, watched: usize, total: usize, size: i64) -> Vec<Candidate> {
        (1..=total)
            .map(|n| Candidate { rating_key: format!("{prefix}{n}"), id: format!("home:alice:{prefix}{n}"), watched: n <= watched, size })
            .collect()
    }

    fn held(prefix: &str, numbers: &[usize], owner: Option<&str>, size: i64) -> Vec<Held> {
        numbers.iter().map(|n| Held { id: format!("home:alice:{prefix}{n}"), policy: owner.map(str::to_string), size }).collect()
    }

    fn known(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn added(plan: &Plan) -> Vec<&str> {
        plan.add.iter().map(|(_, k)| k.as_str()).collect()
    }

    #[test]
    fn keeps_the_next_unwatched_episodes() {
        let p = policy("a", 3, true);
        let fetched = [Fetched { policy: &p, candidates: episodes("e", 1, 6, 100) }];
        let plan = evaluate(&fetched, &[], &known(&["a"]), 0, None);
        assert_eq!(added(&plan), ["e2", "e3", "e4"]);
        assert!(plan.remove.is_empty());
        // Already downloaded episodes are not fetched again.
        let plan = evaluate(&fetched, &held("e", &[2, 3], Some("a"), 100), &known(&["a"]), 0, None);
        assert_eq!(added(&plan), ["e4"]);
    }

    #[test]
    fn removes_watched_downloads_and_fetches_the_next_ones() {
        let p = policy("a", 3, true);
        // e2 was watched since the last round: it goes, the window moves on to e5.
        let fetched = [Fetched { policy: &p, candidates: episodes("e", 2, 6, 100) }];
        let plan = evaluate(&fetched, &held("e", &[2, 3, 4], Some("a"), 100), &known(&["a"]), 0, None);
        assert_eq!(plan.remove, ["home:alice:e2"]);
        assert_eq!(added(&plan), ["e5"]);
    }

    #[test]
    fn keeps_watched_downloads_unless_the_policy_says_otherwise() {
        let p = policy("a", 2, false);
        let fetched = [Fetched { policy: &p, candidates: episodes("e", 2, 6, 100) }];
        let plan = evaluate(&fetched, &held("e", &[1, 2, 3], Some("a"), 100), &known(&["a"]), 0, None);
        assert!(plan.remove.is_empty(), "watched episodes stay: {plan:?}");
        assert_eq!(added(&plan), ["e4"]);
    }

    #[test]
    fn downloads_that_fell_out_of_the_window_are_removed() {
        // Keep was lowered from 4 to 2: the two furthest episodes go.
        let p = policy("a", 2, false);
        let fetched = [Fetched { policy: &p, candidates: episodes("e", 0, 6, 100) }];
        let plan = evaluate(&fetched, &held("e", &[1, 2, 3, 4], Some("a"), 100), &known(&["a"]), 0, None);
        assert_eq!(plan.remove, ["home:alice:e3", "home:alice:e4"]);
        assert!(plan.add.is_empty());
    }

    #[test]
    fn manual_downloads_are_never_removed() {
        let p = policy("a", 2, true);
        // e1 was watched and downloaded by hand: it stays. It does not count towards the window.
        let fetched = [Fetched { policy: &p, candidates: episodes("e", 1, 5, 100) }];
        let plan = evaluate(&fetched, &held("e", &[1], None, 100), &known(&["a"]), 0, None);
        assert!(plan.remove.is_empty());
        assert_eq!(added(&plan), ["e2", "e3"]);
    }

    #[test]
    fn a_policy_that_could_not_be_fetched_removes_nothing() {
        // No Fetched entry for "a" (the server was unreachable, or it is disabled).
        let plan = evaluate(&[], &held("e", &[1, 2], Some("a"), 100), &known(&["a"]), 0, None);
        assert_eq!(plan, Plan::default());
    }

    #[test]
    fn the_budget_limits_what_is_added() {
        let p = policy("a", 4, true);
        let fetched = [Fetched { policy: &p, candidates: episodes("e", 0, 6, 100) }];
        // 250 bytes: two fit, and the rest wait in order (no skipping ahead).
        let plan = evaluate(&fetched, &[], &known(&["a"]), 0, Some(250));
        assert_eq!(added(&plan), ["e1", "e2"]);
        assert_eq!(plan.over_budget, 2);
        // Other servers' downloads count against the budget too.
        let plan = evaluate(&fetched, &[], &known(&["a"]), 150, Some(250));
        assert_eq!(added(&plan), ["e1"]);
        // Existing downloads count; nothing is removed to make room.
        let plan = evaluate(&fetched, &held("e", &[1, 2], Some("a"), 100), &known(&["a"]), 0, Some(250));
        assert!(added(&plan).is_empty());
        assert_eq!(plan.over_budget, 2);
        assert!(plan.remove.is_empty());
    }

    #[test]
    fn removing_watched_files_frees_budget_for_the_next_ones() {
        let p = policy("a", 2, true);
        let fetched = [Fetched { policy: &p, candidates: episodes("e", 1, 5, 100) }];
        // e1 (watched) is removed, which leaves room for e3 within 200 bytes.
        let plan = evaluate(&fetched, &held("e", &[1, 2], Some("a"), 100), &known(&["a"]), 0, Some(200));
        assert_eq!(plan.remove, ["home:alice:e1"]);
        assert_eq!(added(&plan), ["e3"]);
        assert_eq!(plan.over_budget, 0);
    }

    #[test]
    fn two_policies_share_the_budget_and_do_not_fight_over_files() {
        let a = policy("a", 2, true);
        let b = policy("b", 2, true);
        let fetched = [
            Fetched { policy: &a, candidates: episodes("x", 0, 3, 100) },
            // The second policy lists the same episode x1 plus its own.
            Fetched { policy: &b, candidates: [episodes("x", 0, 1, 100), episodes("y", 0, 3, 100)].concat() },
        ];
        let plan = evaluate(&fetched, &[], &known(&["a", "b"]), 0, None);
        assert_eq!(added(&plan), ["x1", "x2", "y1"], "x1 is fetched once");
        // A file wanted by b stays even though a no longer wants it.
        let a_only = [Fetched { policy: &a, candidates: episodes("x", 1, 3, 100) }, fetched.into_iter().nth(1).unwrap()];
        let plan = evaluate(&a_only, &held("x", &[1], Some("a"), 100), &known(&["a", "b"]), 0, None);
        assert!(plan.remove.is_empty());
    }

    #[test]
    fn downloads_of_a_deleted_policy_are_adopted_or_released() {
        let b = policy("b", 2, true);
        let fetched = [Fetched { policy: &b, candidates: episodes("e", 0, 4, 100) }];
        // "gone" no longer exists. e1 is wanted by b and adopted; e4 is nobody's and released.
        let mut files = held("e", &[1], Some("gone"), 100);
        files.extend(held("e", &[4], Some("gone"), 100));
        let plan = evaluate(&fetched, &files, &known(&["b"]), 0, None);
        assert_eq!(plan.assign, [("home:alice:e1".to_string(), Some("b".to_string())), ("home:alice:e4".to_string(), None)]);
        assert!(plan.remove.is_empty());
    }

    #[test]
    fn only_this_account_and_server_are_considered() {
        let scope = crate::prefs::Scope { account: "alice".into(), server: "home".into() };
        let mk = |server: &str, account: &str, size: i64| Download {
            id: format!("{server}:{account}"),
            server_id: server.into(),
            server_name: String::new(),
            item: Metadata::default(),
            file: None,
            poster: None,
            size,
            received: 0,
            state: downloads::State::Done,
            error: None,
            added_at: 0,
            account: account.into(),
            validator: None,
            source_key: None,
            policy: Some("a".into()),
        };
        let all = [mk("home", "alice", 10), mk("home", "bob", 20), mk("away", "alice", 40)];
        let (held, outside) = split(&all, &scope);
        assert_eq!(held.iter().map(|h| h.id.as_str()).collect::<Vec<_>>(), ["home:alice"]);
        assert_eq!(outside, 60);

        let mut other = policy("a", 1, true);
        assert!(in_scope(&other, &scope));
        other.server_id = "away".into();
        assert!(!in_scope(&other, &scope));
        other.server_id = "home".into();
        other.account = "bob".into();
        assert!(!in_scope(&other, &scope));
    }

    fn episode(season: i64, number: i64, key: &str) -> Metadata {
        Metadata { rating_key: Some(key.into()), parent_index: Some(season), index: Some(number), ..Default::default() }
    }

    #[test]
    fn episodes_are_watched_in_order_without_specials() {
        let items = vec![episode(2, 1, "c"), episode(0, 1, "special"), episode(1, 2, "b"), episode(1, 1, "a")];
        let keys = |v: &[Metadata]| v.iter().map(|m| m.rating_key().to_string()).collect::<Vec<_>>();
        assert_eq!(keys(&watch_order(Target::Show, items.clone())), ["a", "b", "c"]);
        // Asking for the specials season itself keeps them.
        let specials = vec![episode(0, 2, "s2"), episode(0, 1, "s1")];
        assert_eq!(keys(&watch_order(Target::Season, specials.clone())), ["s1", "s2"]);
        // A playlist keeps its own order.
        assert_eq!(keys(&watch_order(Target::Playlist, specials)), ["s2", "s1"]);
    }

    #[test]
    fn candidates_follow_server_and_offline_watches() {
        let scope = crate::prefs::Scope { account: "alice".into(), server: "home".into() };
        let media = vec![patina_plex::Media { parts: vec![patina_plex::Part { size: Some(500), ..Default::default() }], ..Default::default() }];
        let mut seen = episode(1, 1, "e1");
        seen.kind = Some("episode".into());
        seen.media = media.clone();
        seen.view_count = Some(1);
        let mut offline = episode(1, 2, "e2");
        offline.kind = Some("episode".into());
        offline.media = media.clone();
        let mut fresh = episode(1, 3, "e3");
        fresh.kind = Some("episode".into());
        fresh.media = media;
        let mut no_file = episode(1, 4, "e4");
        no_file.kind = Some("episode".into());
        let watched_offline: HashSet<String> = ["home:alice:e2".to_string()].into();
        let c = candidates(&[seen, offline, fresh, no_file], &scope, &watched_offline);
        assert_eq!(c.iter().map(|c| (c.rating_key.as_str(), c.watched, c.size)).collect::<Vec<_>>(), [("e1", true, 500), ("e2", true, 500), ("e3", false, 500)]);
    }
}
