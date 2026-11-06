//! The bridge to `patina-core`: the shared [`AppState`], its event sink, and
//! running core futures (which need tokio) from GPUI tasks.

use crate::prelude::*;
use std::future::Future;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use gpui_kit::{App, Global};
use patina_core::{AppState, CoreEvent, EventSink};

/// The core's state, shared by every view.
pub struct Core(pub AppState);

impl Global for Core {}

/// The core's state.
pub fn core(cx: &App) -> AppState {
    cx.global::<Core>().0.clone()
}

/// Forwards core events (from any thread) to the UI thread, where [`crate::store`] applies them.
pub struct GuiSink(UnboundedSender<CoreEvent>);

impl GuiSink {
    pub fn new() -> (Self, UnboundedReceiver<CoreEvent>) {
        let (tx, rx) = unbounded();
        (Self(tx), rx)
    }
}

impl EventSink for GuiSink {
    fn emit(&self, event: CoreEvent) {
        let _ = self.0.unbounded_send(event);
    }
}

/// Run `fut` on the core's tokio runtime; the returned future can be awaited from a GPUI task.
pub fn bg<T: Send + 'static>(fut: impl Future<Output = T> + Send + 'static) -> impl Future<Output = T> {
    let handle = patina_core::runtime().spawn(fut);
    async move {
        match handle.await {
            Ok(v) => v,
            Err(e) if e.is_panic() => std::panic::resume_unwind(e.into_panic()),
            Err(e) => panic!("core task cancelled: {e}"),
        }
    }
}

/// Run blocking work (file IO such as saving settings) off the UI thread.
pub fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> impl Future<Output = T> {
    bg(async move { tokio::task::spawn_blocking(f).await.expect("blocking task") })
}

/// Something a view loads: still loading, loaded, or failed with a message.
#[derive(Clone, Debug, Default)]
pub enum Load<T> {
    #[default]
    Loading,
    Ready(T),
    Failed(String),
}

impl<T> Load<T> {
    pub fn from_result(r: anyhow::Result<T>) -> Self {
        match r {
            Ok(v) => Self::Ready(v),
            Err(e) => Self::Failed(message(&e)),
        }
    }

    pub fn ready(&self) -> Option<&T> {
        match self {
            Self::Ready(v) => Some(v),
            _ => None,
        }
    }

    pub fn ready_mut(&mut self) -> Option<&mut T> {
        match self {
            Self::Ready(v) => Some(v),
            _ => None,
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }
}

/// An error as one line for a toast or an error state.
pub fn message(e: &anyhow::Error) -> String {
    format!("{e:#}")
}
