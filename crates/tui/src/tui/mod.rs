//! Terminal UI: Miller columns, a details panel, search and dialogs.

mod app;
mod column;
mod marks;
mod modal;
mod pager;
pub mod rows;
mod saved;
pub mod source;
mod task;
mod ui;

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste, EventStream};
use crossterm::execute;
use futures::StreamExt;
use patina_core::{AppState, CoreEvent, EventSink, session};
use tokio::sync::mpsc::UnboundedSender;

use app::{App, Msg};

/// Forwards core events into the UI loop.
struct TuiSink(UnboundedSender<Msg>);

impl EventSink for TuiSink {
    fn emit(&self, event: CoreEvent) {
        let _ = self.0.send(Msg::Core(event));
    }
}

/// Run the interactive UI until the user quits.
pub async fn run() -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let core = AppState::new(Arc::new(TuiSink(tx.clone())), tokio::runtime::Handle::current());
    let mut accent = core.settings().tui.accent;
    let mut theme = ui::Theme::new(&accent);
    let mut app = App::new(core.clone(), tx);
    tokio::spawn(session::restore(core));

    let mut terminal = ratatui::init();
    let _ = execute!(std::io::stdout(), EnableBracketedPaste);
    let mut events = EventStream::new();
    let mut tick = tokio::time::interval(Duration::from_millis(100));
    let result: Result<()> = loop {
        if app.tui.accent != accent {
            accent = app.tui.accent.clone();
            theme = ui::Theme::new(&accent);
        }
        if let Err(e) = terminal.draw(|f| ui::draw(f, &mut app, &theme)) {
            break Err(e.into());
        }
        tokio::select! {
            ev = events.next() => match ev {
                Some(Ok(ev)) => app.on_event(ev),
                Some(Err(e)) => break Err(e.into()),
                None => break Ok(()),
            },
            Some(msg) = rx.recv() => app.on_msg(msg),
            _ = tick.tick() => app.on_tick(),
        }
        // Apply everything else that is already queued before redrawing.
        while let Ok(msg) = rx.try_recv() {
            app.on_msg(msg);
        }
        if app.quit {
            break Ok(());
        }
    };
    let _ = execute!(std::io::stdout(), DisableBracketedPaste);
    ratatui::restore();
    result
}
