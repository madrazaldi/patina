//! Non-interactive subcommands.

use std::io::Write;
use std::sync::Arc;

use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use patina_core::ops::lists;
use patina_core::player::{self, PlayRequest};
use patina_core::state::{ServerView, Stage};
use patina_core::{AppState, CoreEvent, EventSink, config, session};
use patina_plex::{MetadataOptions, Params};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::fetch;
use crate::format;
use crate::item::{Item, Kind, resolve_key};

#[derive(Parser)]
#[command(name = "patina-tui", version, about = "A fast terminal client for Plex", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Sign in with plex.tv and choose a server (or pass --url and --token).
    Login {
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        token: Option<String>,
    },
    /// Forget the saved sign-in.
    Logout,
    /// List servers on your account; `--switch` picks a different one.
    Servers {
        #[arg(long)]
        switch: bool,
    },
    /// Show server details and activity.
    Info,
    /// List library sections.
    Libraries(Output),
    /// List any Plex path (`/library/...`) or an item's children by rating key.
    Ls {
        target: String,
        #[command(flatten)]
        out: Output,
    },
    /// Search the server.
    Search {
        query: String,
        #[command(flatten)]
        out: Output,
    },
    /// Continue watching (in-progress and next-up items).
    Continue(Output),
    /// Recently added items.
    Recent {
        #[arg(short = 'n', default_value_t = 30)]
        limit: usize,
        #[command(flatten)]
        out: Output,
    },
    /// Who is streaming from the server right now.
    Sessions(Output),
    /// Watch history.
    History {
        #[arg(short = 'n', default_value_t = 30)]
        limit: usize,
        #[command(flatten)]
        out: Output,
    },
    /// List playlists, or a playlist's items.
    Playlists {
        id: Option<String>,
        #[command(flatten)]
        out: Output,
    },
    /// Play an item (or a whole show/season/album/playlist) by rating key.
    Play {
        rating_key: String,
        /// Ignore the resume point.
        #[arg(long)]
        from_start: bool,
        /// Shuffle a container.
        #[arg(long)]
        shuffle: bool,
        /// `original`, or a transcode preset such as 1080p, 720p or 480p.
        /// Defaults to the configured quality.
        #[arg(long)]
        quality: Option<String>,
    },
    /// Show your plex.tv watchlist, or add/remove an item by rating key.
    Watchlist {
        #[command(subcommand)]
        action: Option<WatchlistAction>,
        #[command(flatten)]
        out: Output,
    },
    /// List Plex Home users, or switch to one.
    Users {
        #[command(subcommand)]
        action: Option<UsersAction>,
    },
    /// Mark an item (or a whole show/season) watched or unwatched.
    Mark { state: MarkState, rating_key: String },
    /// Rate an item from 0 to 10 (omit the rating to clear it).
    Rate { rating_key: String, rating: Option<f64> },
    /// Scan libraries for new files: a section id or name, or all.
    Scan {
        section: Option<String>,
        /// Force a full metadata refresh.
        #[arg(long)]
        force: bool,
    },
    /// Print a direct stream URL for an item (contains your token).
    Url { rating_key: String },
    /// Print the config file path.
    ConfigPath,
}

#[derive(clap::Args, Clone, Copy)]
pub struct Output {
    /// Print Plex's JSON instead of a table.
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand)]
pub enum WatchlistAction {
    Add { rating_key: String },
    Remove { rating_key: String },
}

#[derive(Subcommand)]
pub enum UsersAction {
    /// Switch to a Home user by name (asks for their PIN if protected).
    Switch {
        user: String,
        #[arg(long)]
        pin: Option<String>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum MarkState {
    Watched,
    Unwatched,
}

/// Prints toasts to stderr and hands every event to the command.
struct CliSink(UnboundedSender<CoreEvent>);

impl EventSink for CliSink {
    fn emit(&self, event: CoreEvent) {
        if let CoreEvent::Toast(t) = &event {
            eprintln!("{}{}", if t.error { "error: " } else { "" }, t.text);
        }
        let _ = self.0.send(event);
    }
}

struct Cx {
    core: AppState,
    rx: UnboundedReceiver<CoreEvent>,
}

impl Cx {
    fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let core = AppState::new(Arc::new(CliSink(tx)), tokio::runtime::Handle::current());
        Self { core, rx }
    }

    /// Wait for the session to settle on a stage other than booting/connecting.
    async fn settle(&mut self) -> Stage {
        loop {
            match self.rx.recv().await {
                Some(CoreEvent::Stage(Stage::Booting | Stage::Connecting { .. })) => {}
                Some(CoreEvent::Stage(stage)) => return stage,
                Some(_) => {}
                None => return Stage::Failed { message: "the session ended".into() },
            }
        }
    }

    /// Restore the saved session and wait until it is connected.
    async fn ready(&mut self) -> Result<()> {
        tokio::spawn(session::restore(self.core.clone()));
        self.expect_ready().await
    }

    async fn expect_ready(&mut self) -> Result<()> {
        match self.settle().await {
            Stage::Ready => Ok(()),
            Stage::Login => bail!("not signed in; run `patina-tui login`"),
            Stage::PickServer { .. } => bail!("no server chosen; run `patina-tui servers --switch`"),
            Stage::Failed { message } => bail!("{message}"),
            Stage::Booting | Stage::Connecting { .. } => unreachable!(),
        }
    }
}

fn print_items(items: &[Item], out: Output) -> Result<()> {
    if out.json {
        let values: Vec<_> = items.iter().map(Item::json).collect();
        println!("{}", serde_json::to_string_pretty(&values)?);
        return Ok(());
    }
    for item in items {
        println!("{}", row(item));
    }
    Ok(())
}

fn row(item: &Item) -> String {
    let id = item.rating_key.clone().unwrap_or_default();
    let kind = match item.kind() {
        Kind::Section => format!("{} library", item.meta.kind.as_deref().unwrap_or("")),
        _ => item.meta.kind.clone().unwrap_or_else(|| "dir".into()),
    };
    let mut title = if matches!(item.kind(), Kind::Episode | Kind::Track | Kind::Season | Kind::Album) {
        item.full_title()
    } else {
        item.title().to_string()
    };
    if let Some(y) = item.year.filter(|_| matches!(item.kind(), Kind::Movie | Kind::Show)) {
        title.push_str(&format!(" ({y})"));
    }
    let mut extra = Vec::new();
    if let Some(u) = &item.user {
        extra.push(u.title.clone().unwrap_or_default());
    }
    if let Some(p) = &item.player {
        extra.push(format!("{} [{}]", p.title.as_deref().unwrap_or(""), p.state.as_deref().unwrap_or("")));
    }
    if let Some(ts) = item.viewed_at {
        extra.push(format::relative(ts));
    }
    if item.is_watchable() {
        if let Some(p) = item.progress().filter(|_| item.in_progress()) {
            extra.push(format!("{:.0}% watched", p * 100.0));
        } else if let Some(n) = item.unwatched_leaves().filter(|n| *n > 0) {
            extra.push(format!("{n} unwatched"));
        } else if item.is_watched() {
            extra.push("watched".into());
        }
    }
    let extra = if extra.is_empty() { String::new() } else { format!("  — {}", extra.join(", ")) };
    format!("{id:>8}  {kind:<10} {title}{extra}")
}

fn prompt_line(prompt: &str) -> Result<String> {
    eprint!("{prompt}");
    std::io::stderr().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

fn print_servers(servers: &[ServerView], current: Option<&str>) {
    for (i, s) in servers.iter().enumerate() {
        let mark = if Some(s.id.as_str()) == current { "*" } else { " " };
        let mut notes = Vec::new();
        if !s.owned {
            notes.push(format!("shared by {}", s.source_title.as_deref().unwrap_or("someone")));
        }
        if !s.presence {
            notes.push("offline".into());
        }
        let notes = if notes.is_empty() { String::new() } else { format!("  ({})", notes.join(", ")) };
        println!("{mark} {:>2}. {}{notes}", i + 1, s.name);
    }
}

fn choose(servers: &[ServerView]) -> Result<String> {
    if servers.is_empty() {
        bail!("no servers on this account");
    }
    print_servers(servers, None);
    let n: usize = prompt_line("Server number: ")?.parse().map_err(|_| anyhow!("not a number"))?;
    let s = servers.get(n.wrapping_sub(1)).ok_or_else(|| anyhow!("no server {n}"))?;
    Ok(s.id.clone())
}

fn connected(core: &AppState) {
    let snap = core.snapshot();
    if let Some(s) = snap.session {
        let user = s.user.map(|u| format!(" as {}", u.name)).unwrap_or_default();
        println!("✓ Connected to {}{user}", s.server_name);
    }
}

/// Run one subcommand.
pub async fn run(command: Command) -> Result<()> {
    let mut cx = Cx::new();
    let core = cx.core.clone();
    match command {
        Command::ConfigPath => {
            println!("{}", config::config_dir().join("config.toml").display());
            return Ok(());
        }
        Command::Logout => {
            session::sign_out(&core);
            println!("Signed out.");
            return Ok(());
        }
        Command::Login { url: Some(url), token } => {
            let token = match token {
                Some(t) => t,
                None => prompt_line("X-Plex-Token: ")?,
            };
            session::sign_in_manual(&core, &url, &token).await?;
            connected(&core);
            return Ok(());
        }
        Command::Login { url: None, token } => {
            match token {
                Some(token) => {
                    tokio::spawn({
                        let core = core.clone();
                        async move { session::run(&core, session::sign_in(&core, token)).await }
                    });
                }
                None => {
                    let pin = session::create_pin(&core, false).await?;
                    println!("Go to {} and enter the code {}", pin.url, pin.code);
                    let _ = open::that_detached(&pin.url);
                    println!("Waiting for you to sign in…");
                    session::complete_pin(&core, pin.id, pin.attempt).await?;
                }
            }
            loop {
                match cx.settle().await {
                    Stage::Ready => break,
                    Stage::PickServer { servers, .. } => {
                        let id = choose(&servers)?;
                        tokio::spawn({
                            let core = core.clone();
                            async move { session::run(&core, session::pick_server(&core, &id)).await }
                        });
                    }
                    Stage::Failed { message } => bail!("{message}"),
                    Stage::Login => bail!("sign-in was not completed"),
                    _ => {}
                }
            }
            connected(&core);
            return Ok(());
        }
        _ => {}
    }

    cx.ready().await?;
    let server = core.server()?;
    match command {
        Command::Servers { switch } => {
            let snap = core.snapshot();
            let session = snap.session.ok_or_else(|| anyhow!("not signed in"))?;
            if session.servers.is_empty() {
                bail!("{} was added by URL; there is no account server list", session.server_name);
            }
            if switch {
                let id = choose(&session.servers)?;
                session::switch_server(&core, &id).await?;
                cx.expect_ready().await?;
                connected(&core);
            } else {
                print_servers(&session.servers, Some(&session.server_id));
            }
        }
        Command::Users { action } => {
            let tv = core.plex_tv()?;
            let users = tv.home_users().await?;
            let current = core.snapshot().session.and_then(|s| s.user).map(|u| u.name).unwrap_or_default();
            let Some(UsersAction::Switch { user, pin }) = action else {
                for u in &users {
                    let mut notes = Vec::new();
                    if u.admin {
                        notes.push("admin");
                    } else if u.restricted {
                        notes.push("managed");
                    }
                    if u.protected {
                        notes.push("PIN");
                    }
                    let mark = if u.name() == current { "*" } else { " " };
                    println!("{mark} {}  {}", u.name(), notes.join(", "));
                }
                return Ok(());
            };
            let target = users
                .into_iter()
                .find(|u| u.name().eq_ignore_ascii_case(&user) || u.uuid.as_deref() == Some(user.as_str()))
                .ok_or_else(|| anyhow!("no Home user named {user:?}"))?;
            let pin = match pin {
                Some(p) => Some(p),
                None if target.protected => Some(prompt_line(&format!("PIN for {}: ", target.name()))?),
                None => None,
            };
            let uuid = target.uuid.clone().ok_or_else(|| anyhow!("{} has no id", target.name()))?;
            session::switch_user(&core, &uuid, pin.as_deref()).await?;
            cx.expect_ready().await?;
            println!("✓ Switched to {}", target.name());
        }
        Command::Info => {
            let caps = server.capabilities().await?;
            let attr = |k: &str| caps.attr(k).unwrap_or_else(|| "?".into());
            println!("{}", caps.attr("friendlyName").unwrap_or_else(|| "Plex Media Server".into()));
            println!("  URL        {}", server.base_url());
            println!("  Version    {}", attr("version"));
            println!("  Platform   {} {}", attr("platform"), caps.attr("platformVersion").unwrap_or_default());
            println!("  Machine ID {}", caps.machine_identifier.as_deref().unwrap_or("?"));
            if let Some(u) = caps.attr("myPlexUsername") {
                println!("  Owner      {u}");
            }
            let pass = matches!(caps.attr("myPlexSubscription").as_deref(), Some("1" | "true"));
            println!("  Plex Pass  {}", if pass { "yes" } else { "no" });
            let sessions = server.sessions().await.map(|s| s.len()).unwrap_or(0);
            println!("  Streaming  {sessions} session(s)");
            for a in server.activities().await.unwrap_or_default() {
                println!(
                    "  Activity   {} {}{}",
                    a.title.unwrap_or_default(),
                    a.subtitle.unwrap_or_default(),
                    a.progress.map(|p| format!(" ({p}%)")).unwrap_or_default()
                );
            }
        }
        Command::Libraries(out) => {
            let sections: Vec<Item> = core.sections().into_iter().map(Item::section).collect();
            if out.json {
                return print_items(&sections, out);
            }
            for s in sections {
                println!("{:>4}  {:<8} {}", s.key.as_deref().unwrap_or(""), s.meta.kind.as_deref().unwrap_or(""), s.title());
            }
        }
        Command::Ls { target, out } => {
            let path = if target.starts_with('/') { target } else { format!("/library/metadata/{target}/children") };
            let items = Item::listing(server.fetch(&path, &Params::new()).await?);
            print_items(&items, out)?;
            if !out.json {
                for i in items.iter().filter(|i| i.rating_key.is_none()) {
                    if let Some(k) = i.children_key() {
                        eprintln!("  {} → {}", i.title(), resolve_key(&path, &k));
                    }
                }
            }
        }
        Command::Search { query, out } => print_items(&fetch::search(&server, &query, 20).await?, out)?,
        Command::Continue(out) => print_items(&fetch::continue_watching(&server).await?, out)?,
        Command::Recent { limit, out } => {
            let mut items = fetch::recently_added(&server).await?;
            items.truncate(limit);
            print_items(&items, out)?
        }
        Command::Sessions(out) => print_items(&server.sessions().await?.into_iter().map(Item::from).collect::<Vec<_>>(), out)?,
        Command::History { limit, out } => {
            let mut items = Vec::new();
            for page in 0.. {
                let h = lists::history(&core, page).await?;
                items.extend(h.items.into_iter().map(Item::from));
                if !h.has_more || items.len() >= limit {
                    break;
                }
            }
            items.truncate(limit);
            print_items(&items, out)?
        }
        Command::Playlists { id, out } => {
            let items = match id {
                None => server.playlists(None, None).await?,
                Some(id) => lists::playlist_entries(&server, &id).await?.0,
            };
            print_items(&items.into_iter().map(Item::from).collect::<Vec<_>>(), out)?
        }
        Command::Watchlist { action: None, out } => print_items(&fetch::watchlist_items(&core, &server).await?, out)?,
        Command::Watchlist { action: Some(action), .. } => {
            let (rating_key, on) = match action {
                WatchlistAction::Add { rating_key } => (rating_key, true),
                WatchlistAction::Remove { rating_key } => (rating_key, false),
            };
            let item = server.metadata(&rating_key, MetadataOptions::default()).await?.ok_or_else(|| anyhow!("no item {rating_key}"))?;
            let id = item.discover_id().ok_or_else(|| anyhow!("{} isn't matched with the Plex agent", item.title()))?;
            let tv = core.plex_tv()?;
            if on {
                tv.add_to_watchlist(id).await?;
            } else {
                tv.remove_from_watchlist(id).await?;
            }
            println!("✓ {} {} your watchlist", item.title(), if on { "added to" } else { "removed from" });
        }
        Command::Play { rating_key, from_start, shuffle, quality } => {
            let quality = match quality {
                Some(q) => Some(config::quality(&q).ok_or_else(|| anyhow!("unknown quality {q:?}"))?.id.to_string()),
                None => None,
            };
            let opts = MetadataOptions { on_deck: true, ..Default::default() };
            let item = server.metadata(&rating_key, opts).await?.ok_or_else(|| anyhow!("no item {rating_key}"))?;
            player::play(core.clone(), PlayRequest { item, resume: !from_start, shuffle, quality, ..Default::default() });
            play_until_stopped(&mut cx).await?;
        }
        Command::Mark { state, rating_key } => {
            match state {
                MarkState::Watched => server.scrobble(&rating_key).await?,
                MarkState::Unwatched => server.unscrobble(&rating_key).await?,
            }
            println!("✓ done");
        }
        Command::Rate { rating_key, rating } => {
            server.rate(&rating_key, rating).await?;
            println!("✓ done");
        }
        Command::Scan { section, force } => {
            let sections = core.sections();
            let chosen: Vec<_> = match &section {
                None => sections.iter().collect(),
                Some(s) => sections.iter().filter(|d| d.key.as_deref() == Some(s) || d.title().eq_ignore_ascii_case(s)).collect(),
            };
            if chosen.is_empty() {
                bail!("no library matches {:?}", section.unwrap_or_default());
            }
            for s in chosen {
                server.refresh_section(s.key.as_deref().unwrap_or(""), force, None).await?;
                println!("✓ scanning {}", s.title());
            }
        }
        Command::Url { rating_key } => {
            let item = server.metadata(&rating_key, MetadataOptions::default()).await?.ok_or_else(|| anyhow!("no item {rating_key}"))?;
            let part = Item::from(item).primary_part().and_then(|p| p.key.clone()).ok_or_else(|| anyhow!("no media"))?;
            println!("{}", server.part_url(&part, false)?);
        }
        Command::Login { .. } | Command::Logout | Command::ConfigPath => unreachable!(),
    }
    Ok(())
}

/// Report playback until the player closes.
async fn play_until_stopped(cx: &mut Cx) -> Result<()> {
    let mut current: Option<String> = None;
    let mut last_pos = 0;
    loop {
        match cx.rx.recv().await {
            Some(CoreEvent::NowPlaying(Some(np))) => {
                last_pos = np.position_ms;
                let title = Item::from(np.item.clone()).full_title();
                if current.as_deref() != Some(title.as_str()) {
                    println!("▶ {title}");
                    current = Some(title);
                }
            }
            Some(CoreEvent::NowPlaying(None)) => {
                if current.is_none() {
                    bail!("playback did not start");
                }
                println!("  stopped at {}", format::clock_ms(last_pos));
                return Ok(());
            }
            Some(_) => {}
            None => return Ok(()),
        }
    }
}
