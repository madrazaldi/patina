//! Rows of the tool screens (downloads, Live TV, activity, server tools,
//! settings): what each shows and what enter, `x` and the menu do with it.

use patina_core::config::{ACCENTS, QUALITIES, Settings};
use patina_core::downloads::{Download, State};
use patina_core::livetv::{Channel, Program, Recordings};
use patina_core::ops::server::{Overview, Pref, Stats, Tasks, UsersDevices};
use patina_core::state::{ServerActivity, SessionView};
use patina_plex::{ButlerTask, Directory};

use super::source::{Link, Source};
use crate::format;
use crate::item::Item;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Normal,
    Accent,
    Dim,
    Error,
    Good,
}

#[derive(Debug, Clone)]
pub struct Row {
    /// Stable identity, so the cursor stays put across reloads.
    pub id: String,
    pub glyph: &'static str,
    pub tone: Tone,
    /// Right-aligned note.
    pub note: String,
    /// Label/value pairs for the details panel.
    pub detail: Vec<(String, String)>,
    pub text: Option<String>,
    pub act: Act,
}

#[derive(Debug, Clone)]
pub enum Act {
    Info,
    Heading,
    Download(Box<Download>),
    Channel(Box<Channel>),
    Program { program: Box<Program> },
    Rule(String),
    Activity { uuid: String, cancellable: bool },
    Transcode(String),
    /// Load older history.
    MoreHistory,
    Butler(String),
    Server(ServerOp),
    OpenUrl(String),
    Library(Box<Directory>),
    Pref { pref: Box<Pref>, section: Option<String> },
    Setting(SettingKey),
    Account(AccountOp),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerOp {
    CheckUpdates,
    InstallUpdate,
    UpdateTonight,
    ScanAll,
    ReloadSections,
    AddLibrary,
    CleanBundles,
    OptimizeDatabase,
    SaveLogs,
    SaveDatabase,
    ButlerStartAll,
    ButlerStopAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountOp {
    SwitchServer,
    SwitchUser,
    SignOut,
    /// Leave a direct connection for the plex.tv sign-in.
    SignIn,
    Reconnect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKey {
    MpvPath,
    MpvConfig,
    ExtraArgs,
    Quality,
    Continuous,
    SyncStreams,
    SkipIntro,
    SkipCredits,
    SkipKey,
    WatchedThreshold,
    MediaControls,
    Discord,
    DiscordAppId,
    DownloadsDir,
    Inspector,
    WatchStatus,
    LibraryCounts,
    HideWatched,
    Accent,
}

pub const THRESHOLDS: [f64; 4] = [0.8, 0.85, 0.9, 0.95];

impl SettingKey {
    /// Choices of a setting with a fixed list, as (value, label).
    pub fn choices(self) -> Vec<(String, String)> {
        match self {
            SettingKey::Quality => QUALITIES.iter().map(|q| (q.id.to_string(), q.label.to_string())).collect(),
            SettingKey::WatchedThreshold => THRESHOLDS.iter().map(|t| (t.to_string(), format!("{:.0}%", t * 100.0))).collect(),
            SettingKey::Accent => std::iter::once((String::new(), "Terminal default".to_string()))
                .chain(ACCENTS.iter().map(|a| (a.id.to_string(), a.label.to_string())))
                .collect(),
            _ => Vec::new(),
        }
    }

    pub fn is_text(self) -> bool {
        matches!(self, SettingKey::MpvPath | SettingKey::ExtraArgs | SettingKey::SkipKey | SettingKey::DiscordAppId | SettingKey::DownloadsDir)
    }

    /// The setting's value as edited (text or choice value).
    pub fn value(self, s: &Settings) -> String {
        let p = &s.player;
        match self {
            SettingKey::MpvPath => p.mpv_path.clone(),
            SettingKey::ExtraArgs => p.extra_args.clone(),
            SettingKey::Quality => p.quality.clone(),
            SettingKey::SkipKey => p.skip_key.clone(),
            SettingKey::WatchedThreshold => p.watched_threshold.to_string(),
            SettingKey::DiscordAppId => p.discord_app_id.clone(),
            SettingKey::DownloadsDir => p.downloads_dir.clone(),
            SettingKey::Accent => s.tui.accent.clone(),
            _ => String::new(),
        }
    }

    /// Store a text or choice value.
    pub fn set(self, s: &mut Settings, v: &str) {
        let p = &mut s.player;
        match self {
            SettingKey::MpvPath => p.mpv_path = v.trim().into(),
            SettingKey::ExtraArgs => p.extra_args = v.trim().into(),
            SettingKey::Quality => p.quality = v.into(),
            SettingKey::SkipKey => p.skip_key = if v.trim().is_empty() { "TAB".into() } else { v.trim().into() },
            SettingKey::WatchedThreshold => p.watched_threshold = v.parse().unwrap_or(p.watched_threshold),
            SettingKey::DiscordAppId => p.discord_app_id = v.trim().into(),
            SettingKey::DownloadsDir => p.downloads_dir = v.trim().into(),
            SettingKey::Accent => s.tui.accent = v.into(),
            _ => {}
        }
    }

    /// Flip a switch; `None` for settings that aren't one.
    pub fn toggle(self, s: &mut Settings) -> Option<bool> {
        let flag = match self {
            SettingKey::Continuous => &mut s.player.continuous,
            SettingKey::SyncStreams => &mut s.player.sync_streams,
            SettingKey::SkipIntro => &mut s.player.auto_skip_intro,
            SettingKey::SkipCredits => &mut s.player.auto_skip_credits,
            SettingKey::MediaControls => &mut s.player.media_controls,
            SettingKey::Discord => &mut s.player.discord,
            SettingKey::Inspector => &mut s.tui.show_inspector,
            SettingKey::WatchStatus => &mut s.tui.show_watch_status,
            SettingKey::LibraryCounts => &mut s.tui.show_library_counts,
            SettingKey::HideWatched => &mut s.tui.hide_watched,
            _ => return None,
        };
        *flag = !*flag;
        Some(*flag)
    }
}

impl Row {
    pub fn new(id: impl Into<String>, act: Act) -> Self {
        Self { id: id.into(), glyph: "·", tone: Tone::Dim, note: String::new(), detail: Vec::new(), text: None, act }
    }

    pub fn glyph(mut self, glyph: &'static str, tone: Tone) -> Self {
        self.glyph = glyph;
        self.tone = tone;
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.note = note.into();
        self
    }

    pub fn kv(mut self, k: &str, v: impl Into<String>) -> Self {
        let v = v.into();
        if !v.is_empty() {
            self.detail.push((k.into(), v));
        }
        self
    }

    pub fn text(mut self, t: impl Into<String>) -> Self {
        let t = t.into();
        self.text = Some(t).filter(|t| !t.is_empty());
        self
    }

    pub fn item(self, title: impl Into<String>) -> Item {
        Item::row(&title.into(), self)
    }
}

pub fn heading(title: &str) -> Item {
    Row::new(format!("heading:{title}"), Act::Heading).glyph(" ", Tone::Dim).item(title)
}

fn info(label: &str, value: impl Into<String>) -> Item {
    let value = value.into();
    Row::new(format!("info:{label}"), Act::Info).note(value.clone()).kv(label, value).item(label)
}

fn switch(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

fn pct(n: f64) -> String {
    format!("{n:.0}%")
}

// ---- downloads -------------------------------------------------------------

/// The group a download is listed under, as the GUI's Downloads page groups them.
pub fn download_group(d: &Download) -> (u8, String) {
    match d.item.kind() {
        "episode" | "track" => (1, d.item.grandparent_title.clone().unwrap_or_else(|| "Unknown".into())),
        "movie" => (0, "Movies".into()),
        _ => (2, "Other".into()),
    }
}

fn download_order(d: &Download) -> (u8, String, String, i64, i64) {
    let (rank, group) = download_group(d);
    let m = &d.item;
    let title = m.title_sort.clone().unwrap_or_else(|| m.title().to_string());
    match m.kind() {
        "episode" => (rank, group, String::new(), m.parent_index.unwrap_or(0), m.index.unwrap_or(0)),
        "track" => (rank, group, m.parent_title.clone().unwrap_or_default(), m.parent_index.unwrap_or(0), m.index.unwrap_or(0)),
        _ => (rank, group, title, 0, 0),
    }
}

/// Downloads grouped (movies, then shows and artists by name, then the rest), each with its state.
pub fn downloads(list: &[Download]) -> Vec<Item> {
    let mut list: Vec<&Download> = list.iter().collect();
    list.sort_by_key(|d| download_order(d));
    let mut out = Vec::new();
    let mut last: Option<String> = None;
    for d in list {
        let (_, group) = download_group(d);
        if last.as_ref() != Some(&group) {
            out.push(heading(&group));
            last = Some(group);
        }
        let m = &d.item;
        let title = match m.kind() {
            "episode" => format!("{} · {}", m.episode_code(), m.title()),
            "track" => format!("{} · {}", m.parent_title.as_deref().unwrap_or(""), m.title()),
            _ => Item::from(m.clone()).full_title(),
        };
        let pct = if d.size > 0 { (d.received * 100 / d.size).min(100) } else { 0 };
        let (glyph, tone, note) = match d.state {
            State::Done => ("✓", Tone::Dim, format::bytes(d.size)),
            State::Downloading => ("↓", Tone::Accent, format!("{pct}%")),
            State::Queued => ("…", Tone::Dim, "queued".into()),
            State::Failed => ("!", Tone::Error, "failed".into()),
        };
        let size = match d.state {
            State::Downloading => format!("{} of {}", format::bytes(d.received), format::bytes(d.size)),
            _ if d.size > 0 => format::bytes(d.size),
            _ => String::new(),
        };
        let state = match d.state {
            State::Done => "Downloaded".to_string(),
            State::Downloading => format!("Downloading · {pct}%"),
            State::Queued => "Waiting".into(),
            State::Failed => "Failed".into(),
        };
        let row = Row::new(d.id.clone(), Act::Download(Box::new(d.clone())))
            .glyph(glyph, tone)
            .note(note)
            .kv("State", state)
            .kv("Size", size)
            .kv("Server", d.server_name.clone())
            .kv("Added", format::date(d.added_at))
            .kv("File", d.file.clone().unwrap_or_default())
            .text(d.error.clone().unwrap_or_default());
        out.push(row.item(title));
    }
    out
}

// ---- live tv ------------------------------------------------------------------

pub fn clock(ts: i64) -> String {
    use chrono::{Local, TimeZone};
    Local.timestamp_opt(ts, 0).single().map(|d| d.format("%H:%M").to_string()).unwrap_or_default()
}

fn when(ts: i64) -> String {
    use chrono::{Local, TimeZone};
    Local.timestamp_opt(ts, 0).single().map(|d| d.format("%a %b %-d, %H:%M").to_string()).unwrap_or_default()
}

fn range(p: &Program) -> String {
    format!("{}–{}", clock(p.begins_at), clock(p.ends_at))
}

fn program_row(p: &Program, id: String) -> Item {
    let row = Row::new(id, Act::Program { program: Box::new(p.clone()) })
        .glyph(if p.guid.is_some() { "○" } else { " " }, Tone::Dim)
        .note(p.subtitle.clone().unwrap_or_default())
        .kv("Time", range(p))
        .kv("Episode", p.subtitle.clone().unwrap_or_default())
        .text(p.summary.clone().unwrap_or_default());
    row.item(format!("{}  {}", range(p), p.title))
}

/// One row per channel with what's on now; its later listings open as a column.
pub fn guide(channels: &[Channel], now: i64) -> Vec<Item> {
    channels
        .iter()
        .map(|ch| {
            let list: Vec<&Program> = ch.programs.iter().filter(|p| p.ends_at > now).collect();
            let cur = list.first().filter(|p| p.begins_at <= now).copied();
            let next = if cur.is_some() { list.get(1) } else { list.first() }.copied();
            let mut row = Row::new(format!("{}:{}", ch.dvr, ch.id), Act::Channel(Box::new(ch.clone())))
                .glyph(if cur.is_some() { "▶" } else { " " }, Tone::Accent)
                .note(cur.map(|p| p.title.clone()).unwrap_or_else(|| if list.is_empty() { "No guide data".into() } else { "Off air".into() }));
            if let Some(p) = cur {
                let done = ((now - p.begins_at) as f64 / (p.ends_at - p.begins_at).max(1) as f64 * 100.0).clamp(0.0, 100.0);
                row = row.kv("Now", format!("{} · {}", p.title, range(p))).kv("", format!("{}{:.0}% in", p.subtitle.as_ref().map(|s| format!("{s} · ")).unwrap_or_default(), done));
            }
            if let Some(p) = next {
                row = row.kv("Next", format!("{} {}", clock(p.begins_at), p.title));
            }
            row = row.text(cur.and_then(|p| p.summary.clone()).unwrap_or_default());
            let programs = list.iter().enumerate().map(|(i, p)| program_row(p, format!("{}:{}:{i}", ch.dvr, ch.id))).collect();
            let title = match &ch.number {
                Some(n) => format!("{n:>5}  {}", ch.title),
                None => ch.title.clone(),
            };
            let mut it = row.item(title);
            it.link = Some(Box::new(Link { source: Source::Static, items: programs }));
            it
        })
        .collect()
}

pub fn upcoming(r: &Recordings) -> Vec<Item> {
    r.upcoming
        .iter()
        .map(|u| {
            let row = Row::new(format!("rec:{}", u.id), Act::Info)
                .glyph("●", Tone::Accent)
                .note(u.begins_at.map(when).unwrap_or_default())
                .kv("Episode", u.subtitle.clone().unwrap_or_default())
                .kv("When", u.begins_at.map(when).unwrap_or_default())
                .kv("Channel", u.channel.clone().unwrap_or_default())
                .kv("Status", u.status.clone().unwrap_or_default());
            row.item(&u.title)
        })
        .collect()
}

pub fn rules(r: &Recordings) -> Vec<Item> {
    r.rules
        .iter()
        .map(|rule| {
            let row = Row::new(format!("rule:{}", rule.id), Act::Rule(rule.id.clone()))
                .glyph("↻", Tone::Dim)
                .note(rule.kind.clone().unwrap_or_default())
                .kv("Type", rule.kind.clone().unwrap_or_default())
                .text("Deleting a rule stops future recordings; existing recordings are kept.");
            row.item(if rule.title.is_empty() { "Untitled rule" } else { &rule.title })
        })
        .collect()
}

// ---- activity -----------------------------------------------------------------

fn up(s: &Option<String>) -> String {
    s.as_deref().unwrap_or("").to_uppercase()
}

/// Background activities (live from the core) and transcoder sessions.
pub fn tasks(activities: &[ServerActivity], t: &Tasks) -> Vec<Item> {
    let mut out = vec![heading("Background activity")];
    if activities.is_empty() {
        out.push(Row::new("idle", Act::Info).item("The server is idle"));
    }
    for a in activities {
        let row = Row::new(format!("act:{}", a.uuid), Act::Activity { uuid: a.uuid.clone(), cancellable: a.cancellable })
            .glyph("⟳", Tone::Accent)
            .note(if a.progress >= 0 { format!("{}%", a.progress) } else { String::new() })
            .kv("Task", a.subtitle.clone().unwrap_or_default())
            .kv("Type", a.kind.clone())
            .kv("Cancel", if a.cancellable { "x" } else { "not cancellable" });
        out.push(row.item(&a.title));
    }
    out.push(heading("Transcoder sessions"));
    if t.transcodes.is_empty() {
        out.push(Row::new("no-transcodes", Act::Info).item("No transcodes running"));
    }
    for (i, tr) in t.transcodes.iter().enumerate() {
        let title = format!("{} → {} · {}", up(&tr.source_video_codec), up(&tr.video_codec), up(&tr.audio_codec));
        let row = Row::new(format!("tr:{}", tr.key.clone().unwrap_or_else(|| i.to_string())), Act::Transcode(tr.key.clone().unwrap_or_default()))
            .glyph("⚙", Tone::Accent)
            .note(tr.progress.map(|p| format!("{p:.0}%")).unwrap_or_default())
            .kv("Context", tr.context.clone().unwrap_or_default())
            .kv("Protocol", tr.protocol.clone().unwrap_or_default())
            .kv("Video", tr.video_decision.clone().unwrap_or_default())
            .kv("Audio", tr.audio_decision.clone().unwrap_or_default())
            .kv("Speed", format!("{:.1}×{}", tr.speed.unwrap_or(0.0), if tr.throttled { " (throttled)" } else { "" }))
            .kv("HW", if tr.transcode_hw_requested { "requested" } else { "" });
        out.push(row.item(title));
    }
    out
}

// ---- server tools ----------------------------------------------------------------

fn op(id: &str, title: &str, op: ServerOp, text: &str) -> Item {
    Row::new(id, Act::Server(op)).glyph("›", Tone::Accent).text(text).item(title)
}

pub fn overview(o: &Overview) -> Vec<Item> {
    let mut out = vec![
        heading("Resources"),
        info("Host CPU", pct(o.host_cpu)),
        info("Plex CPU", pct(o.plex_cpu)),
        info("Host memory", pct(o.host_memory)),
        info("Plex memory", pct(o.plex_memory)),
        heading("Server"),
        info("Version", o.version.clone().unwrap_or("—".into())),
        info("Platform", o.platform.clone().unwrap_or("—".into())),
        info("Owner", o.owner.clone().unwrap_or("—".into())),
        info("Plex Pass", if o.plex_pass { "Active" } else { "No" }),
        info("Connected via", o.connected_via.clone()),
        info("Machine ID", o.machine_identifier.clone().unwrap_or("—".into())),
        info("Transcoder", if o.transcoder { "Available" } else { "—" }),
        heading("Updates"),
    ];
    match &o.release {
        Some(r) => {
            let notes = r.added.clone().unwrap_or_default();
            out.push(Row::new("release", Act::Info).glyph("↑", Tone::Accent).text(notes).item(format!("Version {} is available", r.version.as_deref().unwrap_or("?"))));
            if o.can_install {
                out.push(op("install", "Install now", ServerOp::InstallUpdate, "The server restarts to install the update."));
                out.push(op("tonight", "Install tonight", ServerOp::UpdateTonight, "Installs during the maintenance window."));
            } else if let Some(url) = &r.download_url {
                out.push(Row::new("download-update", Act::OpenUrl(url.clone())).glyph("›", Tone::Accent).text(url.clone()).item("Download the update"));
            }
        }
        None => out.push(Row::new("uptodate", Act::Info).glyph("✓", Tone::Good).item("The server is up to date")),
    }
    out.push(op("check", "Check for updates now", ServerOp::CheckUpdates, ""));
    out
}

fn hours(secs: i64) -> String {
    let h = secs as f64 / 3600.0;
    if h < 10.0 { format!("{h:.1}") } else { format!("{:.0}", h.round()) }
}

/// Streaming statistics, with bandwidth per time bucket as a bar chart.
pub fn stats(s: &Stats, month: bool) -> Vec<Item> {
    let period = if month { "in the last 30 days" } else { "in the last 24 hours" };
    if s.bandwidth.is_empty() && s.plays == 0 {
        return vec![Row::new("nothing", Act::Info).item(format!("Nothing was streamed {period}"))];
    }
    let total: i64 = s.bandwidth.iter().map(|b| b.lan + b.wan).sum();
    let mut out = vec![
        heading("Totals"),
        info("Data sent", format::bytes(total)),
        info("Plays", s.plays.to_string()),
        info("Hours played", hours(s.play_seconds)),
    ];
    if !s.bandwidth.is_empty() {
        out.push(heading("Bandwidth · LAN / WAN"));
        let max = s.bandwidth.iter().map(|b| b.lan + b.wan).max().unwrap_or(1).max(1);
        for b in &s.bandwidth {
            use chrono::{Local, TimeZone};
            let at = Local.timestamp_opt(b.at, 0).single();
            let label = at.map(|d| d.format(if month { "%b %-d" } else { "%H:%M" }).to_string()).unwrap_or_default();
            let n = ((b.lan + b.wan) * 20 / max) as usize;
            let bar = format!("{label:>6}  {}", "▇".repeat(n.max(usize::from(b.lan + b.wan > 0))));
            let row = Row::new(format!("bw:{}", b.at), Act::Info)
                .note(format::bytes(b.lan + b.wan))
                .kv("LAN", format::bytes(b.lan))
                .kv("WAN", format::bytes(b.wan));
            out.push(row.item(bar));
        }
    }
    for (title, list, empty) in [("Top users", &s.accounts, "No data sent to users"), ("Top devices", &s.devices, "No data sent to devices")] {
        out.push(heading(title));
        if list.is_empty() {
            out.push(Row::new(format!("{title}:none"), Act::Info).item(empty));
        }
        for u in list {
            out.push(Row::new(format!("{title}:{}", u.name), Act::Info).note(format::bytes(u.bytes)).item(&u.name));
        }
    }
    out
}

pub fn maintenance(butler: &[ButlerTask]) -> Vec<Item> {
    let mut out = vec![
        heading("Database"),
        op("bundles", "Clean bundles", ServerOp::CleanBundles, "Delete metadata bundles for media that no longer exists."),
        op("optimize", "Optimize database", ServerOp::OptimizeDatabase, "Compact and re-index the library database."),
        heading("Diagnostics"),
        op("logs", "Download logs", ServerOp::SaveLogs, "Server logs as a zip, for troubleshooting or a support request. Saved to your Downloads folder."),
        op("database", "Download database backup", ServerOp::SaveDatabase, "A copy of the library database. Large libraries take a while. Saved to your Downloads folder."),
        heading("Scheduled tasks"),
        op("start-all", "Start all tasks", ServerOp::ButlerStartAll, "Plex's butler runs these during the maintenance window."),
        op("stop-all", "Stop all tasks", ServerOp::ButlerStopAll, ""),
    ];
    for t in butler {
        let name = t.name.clone().unwrap_or_default();
        let row = Row::new(format!("butler:{name}"), Act::Butler(name.clone()))
            .glyph("◷", if t.enabled { Tone::Accent } else { Tone::Dim })
            .note(if t.enabled { "" } else { "disabled" })
            .kv("Name", name.clone())
            .kv("Every", t.interval.map(|d| format!("{d} days")).unwrap_or_default())
            .text(t.description.clone().unwrap_or_default());
        out.push(row.item(t.title.clone().unwrap_or(name)));
    }
    out
}

/// The server's libraries with their scan state, then library-wide actions.
pub fn libraries(sections: &[Item], activities: &[ServerActivity]) -> Vec<Item> {
    let mut out = Vec::new();
    for s in sections {
        let Some(d) = s.dir.clone() else { continue };
        let id = d.key.clone().unwrap_or_default();
        let task = activities.iter().find(|a| a.section_id.as_deref() == Some(id.as_str()));
        let note = match task {
            Some(t) => format!("scanning {}%", t.progress.max(0)),
            None if d.refreshing => "scanning".into(),
            None => d.scanned_at.map(|t| format!("scanned {}", format::relative(t))).unwrap_or_default(),
        };
        let folders: Vec<String> = d.location.iter().filter_map(|l| l.path.clone()).collect();
        let row = Row::new(format!("lib:{id}"), Act::Library(Box::new(d.clone())))
            .glyph("▢", if task.is_some() || d.refreshing { Tone::Accent } else { Tone::Dim })
            .note(note)
            .kv("Type", d.kind.clone().unwrap_or_default())
            .kv("Agent", d.agent.clone().unwrap_or_default())
            .kv("Scanner", d.scanner.clone().unwrap_or_default())
            .kv("Language", d.language.clone().unwrap_or_default())
            .kv("Folders", folders.join(", "))
            .text(task.map(|t| format!("{} · {}%", t.subtitle.clone().unwrap_or(t.title.clone()), t.progress)).unwrap_or_default());
        let mut it = row.item(d.title());
        it.link = Some(Box::new(Link { source: Source::LibraryPrefs(id), items: Vec::new() }));
        out.push(it);
    }
    if out.is_empty() {
        out.push(Row::new("nolibs", Act::Info).item("This server has no libraries yet"));
    }
    out.push(heading("All libraries"));
    out.push(op("add", "Add library…", ServerOp::AddLibrary, "A new library: its type, name, folders, agent and scanner."));
    out.push(op("scan-all", "Scan all libraries", ServerOp::ScanAll, ""));
    out.push(op("reload", "Reload the list", ServerOp::ReloadSections, ""));
    out
}

/// Settings of the server or a library; `/` searches them.
pub fn prefs(list: Vec<Pref>, section: Option<&str>) -> Vec<Item> {
    list.into_iter()
        .map(|p| {
            let on = |v: &str| v == "true" || v == "1";
            let shown = if p.kind == "bool" {
                switch(on(&p.value)).to_string()
            } else {
                p.options.iter().find(|(v, _)| *v == p.value).map_or(p.value.clone(), |(_, l)| l.clone())
            };
            let row = Row::new(format!("pref:{}", p.id), Act::Pref { pref: Box::new(p.clone()), section: section.map(str::to_string) })
                .glyph(if p.advanced { "◦" } else { "•" }, Tone::Dim)
                .note(shown.clone())
                .kv("Setting", p.id.clone())
                .kv("Value", shown)
                .kv("Type", p.kind.clone())
                .kv("", if p.advanced { "advanced" } else { "" })
                .text(p.summary.clone().unwrap_or_default());
            let label = if p.label.is_empty() { p.id.clone() } else { p.label.clone() };
            row.item(label)
        })
        .collect()
}

pub fn people(d: &UsersDevices) -> Vec<Item> {
    let mut out = vec![heading("Accounts")];
    for a in d.accounts.iter().filter(|a| a.id.unwrap_or(0) > 0) {
        let row = Row::new(format!("acct:{}", a.id.unwrap_or(0)), Act::Info)
            .glyph("☺", Tone::Dim)
            .kv("Audio", a.default_audio_language.clone().unwrap_or("auto".into()))
            .kv("Subtitles", a.default_subtitle_language.clone().unwrap_or("auto".into()));
        out.push(row.item(a.name.clone().unwrap_or_default()));
    }
    out.push(heading("Friends & shared users"));
    if d.friends.is_empty() {
        out.push(Row::new("nofriends", Act::Info).item("This server isn't shared with anyone"));
    }
    for (i, f) in d.friends.iter().enumerate() {
        let name = f.title.clone().or(f.username.clone()).or(f.email.clone()).unwrap_or("User".into());
        let tags: Vec<&str> = [f.home.then_some("Home"), f.restricted.then_some("Restricted"), f.status.as_deref().filter(|s| *s != "accepted")]
            .into_iter()
            .flatten()
            .collect();
        let row = Row::new(format!("friend:{}", f.id.unwrap_or(i as i64)), Act::Info)
            .glyph("☺", Tone::Dim)
            .note(tags.join(" · "))
            .kv("Username", f.username.clone().filter(|u| *u != name).unwrap_or_default())
            .kv("Email", f.email.clone().unwrap_or_default());
        out.push(row.item(name));
    }
    out.push(heading("Players on the network"));
    if d.clients.is_empty() {
        out.push(Row::new("noclients", Act::Info).item("No players are advertising on the local network"));
    }
    for (i, c) in d.clients.iter().enumerate() {
        let row = Row::new(format!("client:{}", c.machine_identifier.clone().unwrap_or(i.to_string())), Act::Info)
            .glyph("▭", Tone::Dim)
            .note(c.product.clone().unwrap_or_default())
            .kv("Address", c.address.clone().unwrap_or_default())
            .kv("Version", c.version.clone().unwrap_or_default());
        out.push(row.item(c.name.clone().unwrap_or_default()));
    }
    out.push(heading("Devices that have connected"));
    for (i, dev) in d.devices.iter().rev().enumerate() {
        let row = Row::new(format!("dev:{}", dev.id.unwrap_or(i as i64)), Act::Info)
            .glyph("▭", Tone::Dim)
            .note(dev.platform.clone().unwrap_or_default())
            .kv("First seen", dev.created_at.map(format::date).unwrap_or_default());
        out.push(row.item(dev.name.clone().unwrap_or_default()));
    }
    out
}

// ---- settings ---------------------------------------------------------------------

/// The Settings screen: player settings (shared with the GUI), the terminal
/// client's own, the account and where things are saved.
pub fn settings(s: &Settings, probe: Option<&Result<String, String>>, session: Option<&SessionView>, offline: bool) -> Vec<Item> {
    let p = &s.player;
    let set = |key: SettingKey, title: &str, value: String, text: &str| {
        let tone = if value == "on" { Tone::Accent } else { Tone::Dim };
        Row::new(format!("set:{key:?}"), Act::Setting(key)).glyph("•", tone).note(value.clone()).kv("Value", value).text(text).item(title)
    };
    let flag = |key: SettingKey, title: &str, on: bool, text: &str| set(key, title, switch(on).into(), text);
    let or = |v: &str, empty: &str| if v.trim().is_empty() { empty.to_string() } else { v.to_string() };
    let probe_text = match probe {
        None => "Checking…".to_string(),
        Some(Ok(t)) => t.clone(),
        Some(Err(e)) => e.clone(),
    };
    let mut mpv = set(SettingKey::MpvPath, "mpv executable", or(&p.mpv_path, "auto-detect"), "Everything plays in mpv, with your mpv configuration, scripts and key bindings. Enter a path, or leave it empty to auto-detect.");
    if let Some(r) = mpv.row.as_mut() {
        r.detail.push(("mpv".into(), probe_text));
        if let Some(Err(_)) = probe {
            r.tone = Tone::Error;
        }
    }
    let mpv_dir = patina_core::config::mpv_config_dir();
    let conf = if mpv_dir.join("mpv.conf").exists() { " (mpv.conf found)" } else { "" };
    let quality = p.quality().label.to_string();
    let accent = ACCENTS.iter().find(|a| a.id == s.tui.accent).map_or("terminal default".to_string(), |a| a.label.to_string());
    let mut out = vec![
        heading("Player"),
        mpv,
        Row::new("set:mpvconfig", Act::Setting(SettingKey::MpvConfig)).glyph("›", Tone::Accent).kv("Folder", format!("{}{conf}", mpv_dir.display())).text("Enter opens mpv's configuration folder.").item("mpv configuration"),
        set(SettingKey::ExtraArgs, "Extra mpv arguments", or(&p.extra_args, "none"), "Appended to the command line, e.g. --fs --profile=hq. Your mpv.conf is always loaded."),
        set(SettingKey::Quality, "Streaming quality", quality, "Original plays the file directly. Lower qualities ask the server to transcode, which helps on slow connections."),
        flag(SettingKey::Continuous, "Autoplay next episode", p.continuous, "Queue the following episodes when you play one."),
        flag(SettingKey::SyncStreams, "Sync audio & subtitle tracks", p.sync_streams, "Start with the tracks selected in Plex, and remember tracks you switch to in mpv."),
        flag(SettingKey::SkipIntro, "Skip intros automatically", p.auto_skip_intro, "Uses Plex intro markers. Otherwise press the skip key when prompted."),
        flag(SettingKey::SkipCredits, "Skip credits automatically", p.auto_skip_credits, "Jumps past credits, or straight to the next episode."),
        set(SettingKey::SkipKey, "Skip key", p.skip_key.clone(), "mpv key name that skips an intro or credits. It is bound only while Patina controls mpv."),
        set(SettingKey::WatchedThreshold, "Mark as watched after", format!("{:.0}%", p.watched_threshold * 100.0), "How much of an item must play before it counts as watched."),
        flag(
            SettingKey::MediaControls,
            "System media controls",
            p.media_controls,
            "Show what's playing in the OS media controls and respond to media keys. Used by the desktop app; the terminal client doesn't show OS media controls.",
        ),
        flag(SettingKey::Discord, "Discord Rich Presence", p.discord, "Show what you're watching or listening to on your Discord profile."),
    ];
    if p.discord {
        out.push(set(
            SettingKey::DiscordAppId,
            "Discord application ID",
            or(&p.discord_app_id, "not set"),
            "Create an application at discord.com/developers and paste its ID here. Its name is what Discord shows, e.g. “Patina”.",
        ));
    }
    out.push(set(
        SettingKey::DownloadsDir,
        "Downloads folder",
        or(&p.downloads_dir, "default"),
        &format!("Where movies, episodes and music you download for offline viewing are saved. Now: {}", p.downloads_dir().display()),
    ));
    out.extend([
        heading("Terminal"),
        flag(SettingKey::Inspector, "Details panel", s.tui.show_inspector, "Show the details panel next to the columns (i toggles it for now)."),
        flag(SettingKey::WatchStatus, "Watch status marks", s.tui.show_watch_status, "● unwatched, ◐ in progress, and dimmed watched titles."),
        flag(SettingKey::LibraryCounts, "Library sizes", s.tui.show_library_counts, "Item counts next to each library."),
        flag(SettingKey::HideWatched, "Hide watched items", s.tui.hide_watched, "New columns start with watched items hidden (H toggles a column)."),
        set(SettingKey::Accent, "Accent", accent, "Used sparingly: focus, progress and watched marks."),
        heading("Account"),
    ]);
    let account = |op: AccountOp, id: &str, title: &str, text: &str| Row::new(id, Act::Account(op)).glyph("›", Tone::Accent).text(text).item(title);
    match session {
        Some(sv) => {
            let user = sv.user.as_ref();
            let who = user.map_or("Direct server connection".to_string(), |u| u.name.clone());
            let mut row = Row::new("acct:user", Act::Info).glyph("☺", Tone::Accent);
            if let Some(u) = user {
                row = row.note(if u.plex_pass { "Plex Pass" } else { "" }).kv("Email", u.email.clone().unwrap_or_default());
            } else {
                row = row.text("Not signed in to plex.tv.");
            }
            out.push(row.item(who));
            let server_note = if sv.owned { "owner" } else { "shared" };
            let servers = sv.servers.len();
            let srv = Row::new("acct:server", Act::Account(AccountOp::SwitchServer))
                .glyph("▣", Tone::Accent)
                .note(if servers > 1 { format!("{servers} servers") } else { server_note.to_string() })
                .kv("Address", sv.base_url.clone())
                .kv("Access", server_note)
                .text(if servers > 1 { "Enter switches to another server of this account." } else { "" });
            out.push(srv.item(format!("Server: {}", sv.server_name)));
            if sv.has_tv {
                out.push(account(AccountOp::SwitchUser, "acct:switch-user", "Switch Plex Home user…", "Switch to another member of your Plex Home."));
                out.push(account(AccountOp::SignOut, "acct:signout", "Sign out", "Sign out of Patina on this device (the desktop app shares the sign-in)."));
            } else {
                out.push(account(AccountOp::SignIn, "acct:signin", "Sign in with Plex", "Leave this direct connection and sign in with a plex.tv account."));
            }
        }
        None => {
            out.push(Row::new("acct:none", Act::Info).glyph("!", Tone::Error).item(if offline { "Offline" } else { "Not connected" }));
            out.push(account(AccountOp::Reconnect, "acct:reconnect", "Connect again", "Try the saved server again."));
            out.push(account(AccountOp::SignOut, "acct:signout", "Sign out", ""));
        }
    }
    out.extend([
        heading("About"),
        info("Version", format!("Patina {}", patina_core::config::VERSION)),
        info("Settings file", patina_core::config::config_dir().join("config.toml").display().to_string()),
    ]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use patina_plex::Metadata;

    fn dl(id: &str, kind: &str, show: &str, s: i64, e: i64, state: State) -> Download {
        Download {
            id: id.into(),
            server_id: "srv".into(),
            server_name: "Mock".into(),
            item: Metadata {
                kind: Some(kind.into()),
                title: Some(id.into()),
                grandparent_title: Some(show.into()).filter(|s: &String| !s.is_empty()),
                parent_index: Some(s),
                index: Some(e),
                ..Default::default()
            },
            file: None,
            poster: None,
            size: 100,
            received: 40,
            state,
            error: None,
            added_at: 0,
        }
    }

    #[test]
    fn downloads_group_like_the_gui() {
        let list = [
            dl("e2", "episode", "Show", 1, 2, State::Done),
            dl("m", "movie", "", 0, 0, State::Downloading),
            dl("e1", "episode", "Show", 1, 1, State::Failed),
        ];
        let rows = downloads(&list);
        let titles: Vec<&str> = rows.iter().map(|i| i.title()).collect();
        assert_eq!(titles, ["Movies", "m", "Show", "S01E01 · e1", "S01E02 · e2"]);
        assert_eq!(rows[1].row.as_ref().unwrap().note, "40%");
        assert_eq!(rows[3].row.as_ref().unwrap().note, "failed");
    }

    #[test]
    fn guide_shows_now_and_lists_later() {
        let p = |t: &str, b: i64, e: i64| Program { title: t.into(), subtitle: None, guid: Some(t.into()), summary: None, thumb: None, begins_at: b, ends_at: e };
        let ch = Channel { dvr: "1".into(), id: "5".into(), number: Some("5".into()), title: "Five".into(), thumb: None, programs: vec![p("old", 0, 50), p("now", 50, 150), p("next", 150, 200)] };
        let rows = guide(&[ch], 100);
        let r = rows[0].row.as_ref().unwrap();
        assert_eq!(r.note, "now");
        assert_eq!(rows[0].link.as_ref().unwrap().items.len(), 2);
    }

    #[test]
    fn settings_edit_the_shared_config() {
        let mut s = Settings::default();
        assert_eq!(SettingKey::Continuous.toggle(&mut s), Some(false));
        SettingKey::WatchedThreshold.set(&mut s, "0.85");
        assert_eq!(s.player.watched_threshold, 0.85);
        SettingKey::SkipKey.set(&mut s, " ");
        assert_eq!(s.player.skip_key, "TAB");
        assert!(SettingKey::Quality.toggle(&mut s).is_none());
        assert_eq!(SettingKey::Accent.choices()[0].1, "Terminal default");
    }
}
