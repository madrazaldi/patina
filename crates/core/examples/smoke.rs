//! Drive the backend against a server without a front end, printing events.
//!
//! ```sh
//! python3 tools/mock_pms.py --media DIR --port 32499 &
//! XDG_CONFIG_HOME=$(mktemp -d) cargo run -p patina-core --example smoke -- http://127.0.0.1:32499 TOKEN [SECONDS]
//! ```
//! Signs in manually, lists the libraries, plays the first movie in mpv
//! (without video or audio output) and then follows live events.

use std::sync::Arc;
use std::time::Duration;

use patina_core::player::{self, PlayRequest};
use patina_core::state::Stage;
use patina_core::{AppState, CoreEvent, EventSink, ops, session};
use patina_plex::LibraryQuery;
use tokio::sync::mpsc;

struct Printer(mpsc::UnboundedSender<CoreEvent>);

impl EventSink for Printer {
    fn emit(&self, event: CoreEvent) {
        let text = match &event {
            CoreEvent::NowPlaying(Some(np)) => format!("{} at {}ms", np.item.title(), np.position_ms),
            CoreEvent::Session { session, sections } => {
                format!("{} with {} libraries", session.as_ref().map_or("none", |s| s.server_name.as_str()), sections.len())
            }
            _ => event.payload().to_string(),
        };
        println!("event {:<16} {}", event.name(), text.chars().take(160).collect::<String>());
        let _ = self.0.send(event);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let url = args.next().unwrap_or_else(|| "http://127.0.0.1:32499".into());
    let token = args.next().unwrap_or_else(|| "token".into());
    let follow: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(40);

    let (tx, mut events) = mpsc::unbounded_channel();
    let app = AppState::new(Arc::new(Printer(tx)), tokio::runtime::Handle::current());
    app.update_settings(|s| s.player.extra_args = "--vo=null --ao=null".into());
    session::sign_in_manual(&app, &format!("  {url} "), &token).await?;
    while !matches!(app.snapshot().stage, Stage::Ready) {
        events.recv().await;
    }

    // Another process changes the file; our next save keeps its change.
    let path = patina_core::config::config_dir().join("config.toml");
    let text = std::fs::read_to_string(&path)?;
    std::fs::write(&path, text.replace("hide_watched = false", "hide_watched = true"))?;
    app.update_settings(|s| s.player.continuous = false);
    println!("settings merged: hide_watched={} continuous={}", app.settings().tui.hide_watched, app.settings().player.continuous);

    for s in app.sections() {
        println!("library {} {}", s.key.as_deref().unwrap_or("?"), s.title.as_deref().unwrap_or("?"));
    }
    let server = app.server()?;
    let movies = server.section_items("1", &LibraryQuery { size: 5, ..Default::default() }).await?.metadata;
    println!("movies: {:?}", movies.iter().map(|m| m.title()).collect::<Vec<_>>());
    let filters = ops::library::section_filters(&app, "1", Some(1)).await?;
    println!("filters: {:?}", filters.iter().map(|f| f.filter.as_str()).collect::<Vec<_>>());
    let tasks = ops::server::tasks(&app).await?;
    println!("tasks: {} activities, {} transcodes", tasks.activities.len(), tasks.transcodes.len());
    let history = ops::lists::history(&app, 0).await?;
    println!("history: {} items, more: {}", history.items.len(), history.has_more);

    if let Some(movie) = movies.into_iter().next() {
        player::play(app.clone(), PlayRequest { item: movie, resume: false, ..Default::default() });
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(follow);
    while let Ok(Some(_)) = tokio::time::timeout_at(deadline, events.recv()).await {}
    player::control(player::Control::Stop);
    Ok(())
}
