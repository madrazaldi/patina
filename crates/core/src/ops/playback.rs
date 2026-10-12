//! Starting playback from things that aren't library items.

use anyhow::{Result, anyhow};

use crate::player::{self, PlayRequest};
use crate::recovery::{self, SavedQueueView, Scope};
use crate::state::AppState;
use crate::{downloads, livetv};

/// Play downloads offline, in the order given.
pub fn play_downloads(app: &AppState, ids: &[String]) -> Result<()> {
    let files = downloads::files(ids);
    let account = app.session().map(|s| crate::offline::account(&s)).or_else(|| app.settings().auth.account_id);
    if files.iter().any(|d| !d.account.is_empty() && Some(d.account.as_str()) != account.as_deref()) {
        return Err(anyhow!("These downloads belong to another Plex profile. Switch to that profile to play them."));
    }
    if files.is_empty() {
        return Err(anyhow!("The downloaded file is missing."));
    }
    player::play_files(app.clone(), files);
    Ok(())
}

/// Tune a Live TV channel and play it; `title` names it if the tuner doesn't.
pub async fn livetv_watch(app: &AppState, dvr: &str, channel: &str, title: &str) -> Result<()> {
    let mut item = livetv::tune(&app.server()?, dvr, channel).await?;
    if item.title.is_none() {
        item.title = Some(title.to_string());
    }
    player::play(app.clone(), PlayRequest { item, live: true, resume: false, ..Default::default() });
    Ok(())
}

/// The queue that was playing when Patina last stopped (for this account and server), to offer a Resume.
pub fn saved_queue(app: &AppState) -> Option<SavedQueueView> {
    let scope = Scope::of(&app.session()?);
    recovery::load_queue(&scope)?.view()
}

/// Play the saved queue again from where it stopped.
pub fn resume_saved(app: &AppState) -> Result<()> {
    let session = app.session().ok_or_else(|| anyhow!("Not connected to a server."))?;
    let saved = recovery::load_queue(&Scope::of(&session)).ok_or_else(|| anyhow!("There is no saved queue to resume."))?;
    let item = saved.items[saved.current].clone();
    let selection = if saved.items.len() > 1 { saved.items.clone() } else { Vec::new() };
    player::play_from(app.clone(), PlayRequest { item, selection, resume: true, quality: saved.quality.clone(), ..Default::default() }, saved.position_ms);
    Ok(())
}

/// Forget the saved queue.
pub fn forget_saved(app: &AppState) {
    if let Some(session) = app.session()
        && let Err(e) = recovery::clear_queue(&Scope::of(&session))
    {
        tracing::debug!("could not forget the saved queue: {e:#}");
    }
}
