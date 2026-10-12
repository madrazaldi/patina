//! Search across every server of the signed-in account.
//!
//! Each server is searched on its own connection, a few at a time and with a
//! timeout, so a slow or unreachable server never holds the others back. Results
//! arrive as a growing snapshot ([`Results`]): after every server that finishes,
//! the same title found on several servers is shown once, listing each copy with
//! its quality. Items found on a server other than the current one are tagged
//! with it (see [`Metadata::server_id`]), which keeps navigation, playback and
//! reporting on the server that has the copy.

use std::time::Duration;

use futures::StreamExt;
use patina_plex::{Media, Metadata};
use serde::Serialize;

use crate::servers;
use crate::state::AppState;

/// Servers searched at once.
pub const CONCURRENCY: usize = 4;
/// How long one server gets to connect and answer before it is given up on.
pub const PER_SERVER: Duration = Duration::from_secs(12);
const HUB_LIMIT: u32 = 24;
const PER_SERVER_ITEMS: usize = 40;

/// One file of a copy: what a person compares when choosing where to watch.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// "1080p · H264 · MKV · 6.2 Mbps · 2.1 GB".
    pub label: String,
    pub resolution: Option<String>,
    pub video_codec: Option<String>,
    pub container: Option<String>,
    /// Kilobits per second.
    pub bitrate: Option<i64>,
    /// Bytes.
    pub size: Option<i64>,
    pub hdr: Option<String>,
}

/// A title as one server has it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerCopy {
    pub server_id: String,
    pub server_name: String,
    /// On the server this session is connected to.
    pub current: bool,
    pub library: Option<String>,
    /// The item, tagged with its server unless it is the current one.
    pub item: Metadata,
    pub versions: Vec<Version>,
}

/// One title and every server that has it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub key: String,
    pub title: String,
    pub year: Option<i64>,
    pub kind: String,
    /// The show and episode number for an episode, the artist for an album, ...
    pub subtitle: Option<String>,
    pub copies: Vec<ServerCopy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ServerState {
    Pending,
    Done,
    Failed,
    TimedOut,
    /// plex.tv reports the server as not running; it is not tried.
    Offline,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub id: String,
    pub name: String,
    pub current: bool,
    pub state: ServerState,
    /// Titles this server contributed.
    pub count: usize,
    pub error: Option<String>,
}

/// Everything found so far.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Results {
    pub query: String,
    pub groups: Vec<Group>,
    pub servers: Vec<ServerStatus>,
    /// Every server has answered, failed or timed out.
    pub done: bool,
}

/// A server to search.
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub id: String,
    pub name: String,
    pub current: bool,
    pub online: bool,
}

/// The current server first, then the account's other servers.
pub fn targets(app: &AppState) -> Vec<Target> {
    let Some(session) = app.session() else { return Vec::new() };
    let mut out = vec![Target { id: session.server_id.clone(), name: session.server_name.clone(), current: true, online: true }];
    out.extend(
        session
            .servers
            .iter()
            .filter(|r| r.id() != session.server_id && r.is_server())
            .map(|r| Target { id: r.id().to_string(), name: r.name().to_string(), current: false, online: r.presence }),
    );
    out
}

impl Results {
    pub fn new(query: &str, targets: &[Target]) -> Self {
        Self {
            query: query.to_string(),
            groups: Vec::new(),
            servers: targets
                .iter()
                .map(|t| ServerStatus {
                    id: t.id.clone(),
                    name: t.name.clone(),
                    current: t.current,
                    state: if t.online { ServerState::Pending } else { ServerState::Offline },
                    count: 0,
                    error: None,
                })
                .collect(),
            done: false,
        }
    }

    /// Fold in what one server found.
    pub fn add(&mut self, target: &Target, items: Vec<Metadata>) {
        let count = items.len();
        for mut item in items {
            item.set_server_id((!target.current).then_some(target.id.as_str()));
            let copy = ServerCopy {
                server_id: target.id.clone(),
                server_name: target.name.clone(),
                current: target.current,
                library: item.library_section_title.clone(),
                versions: item.media.iter().map(version).collect(),
                item,
            };
            insert(&mut self.groups, copy);
        }
        self.finish(target, ServerState::Done, count, None);
    }

    /// Note that a server could not be searched.
    pub fn fail(&mut self, target: &Target, state: ServerState, error: String) {
        self.finish(target, state, 0, Some(error));
    }

    fn finish(&mut self, target: &Target, state: ServerState, count: usize, error: Option<String>) {
        if let Some(s) = self.servers.iter_mut().find(|s| s.id == target.id) {
            s.state = state;
            s.count = count;
            s.error = error;
        }
        self.done = self.servers.iter().all(|s| s.state != ServerState::Pending);
    }
}

/// Put a copy into the group of its title, starting a new group when it is the first.
/// The current server's copy is listed first, then the others by server name.
fn insert(groups: &mut Vec<Group>, copy: ServerCopy) {
    let key = group_key(&copy.item);
    let Some(group) = groups.iter_mut().find(|g| g.key == key) else {
        groups.push(Group {
            key,
            title: display_title(&copy.item),
            year: copy.item.year,
            kind: copy.item.kind().to_string(),
            subtitle: subtitle(&copy.item),
            copies: vec![copy],
        });
        return;
    };
    // A server sometimes lists the same item under more than one hub.
    if group.copies.iter().any(|c| c.server_id == copy.server_id && c.item.rating_key() == copy.item.rating_key()) {
        return;
    }
    group.copies.push(copy);
    group.copies.sort_by(|a, b| b.current.cmp(&a.current).then_with(|| a.server_name.to_lowercase().cmp(&b.server_name.to_lowercase())));
}

fn display_title(m: &Metadata) -> String {
    m.title().to_string()
}

fn subtitle(m: &Metadata) -> Option<String> {
    match m.kind() {
        "episode" => {
            let show = m.grandparent_title.clone().unwrap_or_default();
            let number = match (m.parent_index, m.index) {
                (Some(s), Some(e)) => format!("S{s:02}E{e:02}"),
                _ => String::new(),
            };
            let text = format!("{show} {number}").trim().to_string();
            (!text.is_empty()).then_some(text)
        }
        "album" | "track" => m.parent_title.clone().or_else(|| m.grandparent_title.clone()),
        _ => None,
    }
}

/// What identifies a title across servers: its Plex guid, or without one (local or
/// legacy matches) the kind, title and year.
pub fn group_key(m: &Metadata) -> String {
    if let Some(guid) = m.guid.as_deref().filter(|g| !g.is_empty() && !g.starts_with("local://") && !g.contains("agents.none")) {
        return guid.to_string();
    }
    let norm = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect::<String>();
    match m.kind() {
        "episode" => format!(
            "episode|{}|{}|{}",
            norm(m.grandparent_title.as_deref().unwrap_or("")),
            m.parent_index.unwrap_or(-1),
            m.index.unwrap_or(-1)
        ),
        kind => format!("{kind}|{}|{}", norm(m.title()), m.year.unwrap_or(0)),
    }
}

/// A short description of a file: resolution, codec, container, bitrate and size.
pub fn version(media: &Media) -> Version {
    let resolution = media.video_resolution.as_deref().map(|r| match r {
        "4k" | "4K" => "4K".to_string(),
        "sd" | "SD" => "SD".to_string(),
        r if r.chars().all(|c| c.is_ascii_digit()) => format!("{r}p"),
        r => r.to_uppercase(),
    });
    let part = media.parts.first();
    let container = media.container.clone().or_else(|| part.and_then(|p| p.container.clone())).map(|c| c.to_uppercase());
    let video_codec = media.video_codec.as_ref().map(|c| c.to_uppercase());
    let hdr = part.and_then(|p| p.video_stream()).and_then(|s| s.hdr_label()).map(str::to_string);
    let size = part.and_then(|p| p.size);
    let mut bits = Vec::new();
    if let Some(edition) = media.edition_title.as_deref().or(media.title.as_deref()) {
        bits.push(edition.to_string());
    }
    bits.extend(resolution.clone());
    bits.extend(video_codec.clone());
    bits.extend(hdr.clone());
    bits.extend(container.clone());
    if let Some(kbps) = media.bitrate.filter(|b| *b > 0) {
        bits.push(if kbps >= 1000 { format!("{:.1} Mbps", kbps as f64 / 1000.0) } else { format!("{kbps} kbps") });
    }
    if let Some(bytes) = size.filter(|b| *b > 0) {
        bits.push(size_label(bytes));
    }
    let label = if bits.is_empty() { "Unknown quality".to_string() } else { bits.join(" · ") };
    Version { label, resolution, video_codec, container, bitrate: media.bitrate, size, hdr }
}

fn size_label(bytes: i64) -> String {
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    let b = bytes as f64;
    if b >= GB { format!("{:.1} GB", b / GB) } else { format!("{:.0} MB", b / MB) }
}

/// Search one server; `None` is the current one.
async fn search_one(app: &AppState, target: &Target, query: &str) -> anyhow::Result<Vec<Metadata>> {
    let server = servers::server_for(app, (!target.current).then_some(target.id.as_str())).await?;
    let hubs = server.search_hubs(query, HUB_LIMIT, None).await?;
    let mut seen = std::collections::HashSet::new();
    Ok(hubs
        .into_iter()
        .flat_map(|h| h.metadata)
        .filter(|m| m.kind() != "collection" && seen.insert((m.kind().to_string(), m.rating_key().to_string())))
        .take(PER_SERVER_ITEMS)
        .collect())
}

/// Search every server of the account. `on_update` sees the results after each server
/// that finishes (and once at the start), so a front end can show them as they come.
pub async fn search_all(app: &AppState, query: &str, on_update: impl FnMut(&Results)) -> Results {
    search_all_within(app, query, PER_SERVER, on_update).await
}

/// [`search_all`] with its own limit on how long one server may take.
pub async fn search_all_within(app: &AppState, query: &str, per_server: Duration, mut on_update: impl FnMut(&Results)) -> Results {
    let all = targets(app);
    let mut results = Results::new(query, &all);
    on_update(&results);
    if results.servers.iter().all(|s| s.state != ServerState::Pending) {
        results.done = true;
        on_update(&results);
        return results;
    }
    let mut stream = futures::stream::iter(all.into_iter().filter(|t| t.online))
        .map(|target| async move {
            let outcome = tokio::time::timeout(per_server, search_one(app, &target, query)).await;
            (target, outcome)
        })
        .buffer_unordered(CONCURRENCY);
    while let Some((target, outcome)) = stream.next().await {
        match outcome {
            Ok(Ok(items)) => results.add(&target, items),
            Ok(Err(e)) => {
                tracing::debug!("searching {} failed: {e:#}", target.name);
                if !target.current {
                    servers::forget(app, &target.id);
                }
                results.fail(&target, ServerState::Failed, format!("{e:#}"));
            }
            Err(_) => {
                if !target.current {
                    servers::forget(app, &target.id);
                }
                results.fail(&target, ServerState::TimedOut, "Took too long to answer.".into());
            }
        }
        on_update(&results);
    }
    results.done = true;
    on_update(&results);
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(kind: &str, rk: &str, title: &str, guid: Option<&str>, year: i64) -> Metadata {
        serde_json::from_value(json!({ "ratingKey": rk, "type": kind, "title": title, "guid": guid, "year": year })).unwrap()
    }

    fn target(id: &str, current: bool) -> Target {
        Target { id: id.into(), name: format!("Server {id}"), current, online: true }
    }

    #[test]
    fn same_guid_is_one_group_with_a_copy_per_server() {
        let (a, b) = (target("a", true), target("b", false));
        let mut r = Results::new("copper", &[a.clone(), b.clone()]);
        r.add(&b, vec![item("movie", "7", "Copper Sky", Some("plex://movie/1"), 2015)]);
        r.add(&a, vec![item("movie", "100", "Copper Sky", Some("plex://movie/1"), 2015), item("movie", "101", "Other", Some("plex://movie/2"), 2000)]);
        assert_eq!(r.groups.len(), 2);
        let g = &r.groups[0];
        assert_eq!(g.copies.len(), 2);
        // The current server's copy comes first even though it answered second.
        assert!(g.copies[0].current);
        assert_eq!(g.copies[0].item.server_id(), None);
        assert_eq!(g.copies[1].item.server_id(), Some("b"));
        assert!(r.done);
    }

    #[test]
    fn items_without_a_usable_guid_group_by_title_and_year() {
        let (a, b) = (target("a", true), target("b", false));
        let mut r = Results::new("x", &[a.clone(), b.clone()]);
        r.add(&a, vec![item("movie", "1", "Paper Moons!", Some("local://5"), 2019)]);
        r.add(&b, vec![item("movie", "9", "paper moons", None, 2019), item("movie", "10", "Paper Moons", None, 1999)]);
        assert_eq!(r.groups.len(), 2);
        assert_eq!(r.groups[0].copies.len(), 2);
        assert_eq!(r.groups[1].copies.len(), 1);
    }

    #[test]
    fn episodes_group_by_show_and_number_without_a_guid() {
        let mk = |rk: &str| {
            serde_json::from_value::<Metadata>(json!({ "ratingKey": rk, "type": "episode", "title": "Relay",
                "grandparentTitle": "Harbor Lights", "parentIndex": 2, "index": 3 }))
            .unwrap()
        };
        assert_eq!(group_key(&mk("1")), group_key(&mk("2")));
        assert_eq!(subtitle(&mk("1")).as_deref(), Some("Harbor Lights S02E03"));
    }

    #[test]
    fn duplicates_on_one_server_are_ignored_but_other_items_are_kept() {
        let a = target("a", true);
        let mut r = Results::new("x", std::slice::from_ref(&a));
        let m = item("movie", "1", "Same", Some("plex://movie/1"), 2000);
        r.add(&a, vec![m.clone(), m, item("movie", "2", "Same", Some("plex://movie/1"), 2000)]);
        assert_eq!(r.groups.len(), 1);
        assert_eq!(r.groups[0].copies.len(), 2);
    }

    #[test]
    fn a_failing_server_does_not_hide_the_others() {
        let (a, b, c) = (target("a", true), target("b", false), target("c", false));
        let mut r = Results::new("x", &[a.clone(), b.clone(), c.clone()]);
        r.fail(&b, ServerState::TimedOut, "slow".into());
        assert!(!r.done);
        r.add(&a, vec![item("movie", "1", "Found", Some("plex://movie/1"), 2000)]);
        assert!(!r.done);
        r.fail(&c, ServerState::Failed, "refused".into());
        assert!(r.done);
        assert_eq!(r.groups.len(), 1);
        let states: Vec<_> = r.servers.iter().map(|s| s.state).collect();
        assert_eq!(states, [ServerState::Done, ServerState::TimedOut, ServerState::Failed]);
        assert_eq!(r.servers[1].error.as_deref(), Some("slow"));
    }

    #[test]
    fn offline_servers_are_not_waited_for() {
        let mut off = target("b", false);
        off.online = false;
        let r = Results::new("x", &[target("a", true), off]);
        assert_eq!(r.servers[1].state, ServerState::Offline);
        assert!(!r.done);
    }

    #[test]
    fn versions_describe_quality() {
        let media: Media = serde_json::from_value(json!({
            "videoResolution": "1080", "videoCodec": "hevc", "container": "mkv", "bitrate": 6200,
            "Part": [{ "size": 2_254_857_830u64 }]
        }))
        .unwrap();
        let v = version(&media);
        assert_eq!(v.label, "1080p · HEVC · MKV · 6.2 Mbps · 2.1 GB");
        assert_eq!(v.resolution.as_deref(), Some("1080p"));
        let bare = version(&Media::default());
        assert_eq!(bare.label, "Unknown quality");
        let uhd: Media = serde_json::from_value(json!({ "videoResolution": "4k", "bitrate": 800 })).unwrap();
        assert_eq!(version(&uhd).label, "4K · 800 kbps");
    }
}
