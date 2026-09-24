//! Sign-in, server discovery and connection management.

use std::sync::atomic::Ordering;

use anyhow::{Context, Result, anyhow};
use patina_plex::{PlexServer, PlexTv, Resource, User, connect_resource};

use crate::images;
use crate::state::{AppState, ServerView, Session, Stage, UserView};

fn tv(app: &AppState, token: Option<String>) -> Result<PlexTv> {
    Ok(PlexTv::new(app.client_info(), token)?)
}

/// Run a sign-in step, showing its error on the failure screen.
pub async fn run(app: &AppState, fut: impl Future<Output = Result<()>>) {
    if let Err(e) = fut.await {
        app.set_stage(Stage::Failed { message: format!("{e:#}") });
    }
}

/// Restore the previous session on startup (or after changing accounts).
pub async fn restore(app: AppState) {
    let settings = app.settings();
    if let (Some(url), Some(token)) = (settings.server.manual_url.clone(), settings.server.manual_token.clone()) {
        app.set_stage(Stage::Connecting { name: url.clone() });
        run(&app, connect_manual(&app, &url, &token)).await;
        return;
    }
    let Some(token) = settings.auth.token.clone() else {
        app.set_stage(Stage::Login);
        return;
    };
    run(&app, sign_in(&app, token)).await;
}

/// Validate a plex.tv token and connect to the preferred server.
pub async fn sign_in(app: &AppState, token: String) -> Result<()> {
    app.set_stage(Stage::Connecting { name: "plex.tv".into() });
    let tv = tv(app, Some(token.clone()))?;
    let user = match tv.user().await {
        Ok(u) => u,
        Err(patina_plex::Error::Unauthorized) => {
            app.update_settings(|s| s.auth.token = None);
            app.set_stage(Stage::Login);
            return Ok(());
        }
        Err(e) => return Err(e).context("reaching plex.tv"),
    };
    app.update_settings(|s| s.auth.token = Some(token));
    let servers = tv.servers().await.context("listing servers")?;
    if servers.is_empty() {
        return Err(anyhow!("No Plex Media Servers are available to {}.", user.display_name()));
    }
    let preferred = app.settings().server.id;
    let choice = preferred
        .and_then(|id| servers.iter().find(|s| s.id() == id).cloned())
        .or_else(|| (servers.len() == 1).then(|| servers[0].clone()));
    match choice {
        Some(resource) => connect(app, tv, user, resource, servers).await,
        None => {
            app.set_stage(Stage::PickServer {
                servers: servers.iter().map(ServerView::from).collect(),
                user: UserView::from(&user),
            });
            *app.pending.lock().unwrap() = Some((tv, user, servers));
            Ok(())
        }
    }
}

/// Connect to a server offered on the "choose a server" screen.
pub async fn pick_server(app: &AppState, id: &str) -> Result<()> {
    let (tv, user, servers) = app.pending.lock().unwrap().clone().ok_or_else(|| anyhow!("No server list to pick from."))?;
    let resource = servers.iter().find(|s| s.id() == id).cloned().ok_or_else(|| anyhow!("That server is no longer available."))?;
    connect(app, tv, user, resource, servers).await
}

/// Connect to one server resource and make it the active session.
pub async fn connect(app: &AppState, tv: PlexTv, user: User, resource: Resource, servers: Vec<Resource>) -> Result<()> {
    app.set_stage(Stage::Connecting { name: resource.name().to_string() });
    let server = connect_resource(&tv, &resource).await.with_context(|| format!("connecting to {}", resource.name()))?;
    app.update_settings(|s| {
        s.server.id = Some(resource.id().to_string());
        s.server.last_uri = Some(server.base_url().to_string());
    });
    activate(
        app,
        Session {
            tv: Some(tv),
            server_name: resource.name().to_string(),
            server_id: resource.id().to_string(),
            owned: resource.owned,
            user: Some(user),
            server,
            servers,
        },
    )
    .await
}

async fn connect_manual(app: &AppState, url: &str, token: &str) -> Result<()> {
    let server = PlexServer::new(app.client_info(), url, token)?;
    let id = server.identity().await.context("contacting server")?;
    let server = server.with_machine_identifier(id.machine_identifier.clone());
    let caps = server.capabilities().await.context("server rejected the token")?;
    let name = caps.attr("friendlyName").unwrap_or_else(|| url.to_string());
    // A server token usually also works against plex.tv (watchlist, users).
    let tv = tv(app, Some(token.to_string())).ok();
    let user = match &tv {
        Some(tv) => tv.user().await.ok(),
        None => None,
    };
    activate(
        app,
        Session {
            tv: user.as_ref().and(tv),
            server_name: name,
            server_id: id.machine_identifier.unwrap_or_default(),
            owned: true,
            user,
            server,
            servers: Vec::new(),
        },
    )
    .await
}

/// Save a manual server and connect to it. Errors are returned (and shown on
/// the form) rather than sending the app to the failure screen.
pub async fn sign_in_manual(app: &AppState, url: String, token: String) -> Result<()> {
    let url = if url.starts_with("http") { url } else { format!("http://{url}") };
    connect_manual(app, &url, &token).await?;
    app.update_settings(|s| {
        s.server.manual_url = Some(url);
        s.server.manual_token = Some(token);
    });
    Ok(())
}

async fn activate(app: &AppState, session: Session) -> Result<()> {
    let sections = session.server.sections().await.context("loading libraries")?;
    images::set_server(Some(session.server.clone()));
    *app.pending.lock().unwrap() = None;
    app.set_session(Some(session), sections.into_iter().filter(|d| !d.hidden).collect());
    app.set_stage(Stage::Ready);
    app.bump();
    crate::live::watch(app.clone());
    crate::downloads::resume(app);
    Ok(())
}

pub async fn reload_sections(app: &AppState) -> Result<()> {
    let sections = app.server()?.sections().await?;
    app.set_sections(sections.into_iter().filter(|d| !d.hidden).collect());
    Ok(())
}

/// Switch to another server of the same account.
pub async fn switch_server(app: &AppState, id: &str) -> Result<()> {
    let session = app.session().ok_or_else(|| anyhow!("Not connected."))?;
    let (Some(tv), Some(user)) = (session.tv.clone(), session.user.clone()) else {
        return Err(anyhow!("Switching servers needs a plex.tv sign-in."));
    };
    let resource = session.servers.iter().find(|s| s.id() == id).cloned().ok_or_else(|| anyhow!("Unknown server."))?;
    if let Err(e) = connect(app, tv, user, resource, session.servers.clone()).await {
        // Stay on the current server.
        app.set_stage(Stage::Ready);
        return Err(e);
    }
    Ok(())
}

/// Switch Plex Home user; the new user's token replaces the stored one.
pub async fn switch_user(app: &AppState, uuid: &str, pin: Option<&str>) -> Result<()> {
    let tv = app.tv().ok_or_else(|| anyhow!("Not signed in to plex.tv."))?;
    let user = tv.switch_home_user(uuid, pin).await.context("switching user")?;
    let token = user.auth_token.clone().ok_or_else(|| anyhow!("plex.tv returned no token"))?;
    app.set_session(None, Vec::new());
    run(app, sign_in(app, token)).await;
    Ok(())
}

pub fn sign_out(app: &AppState) {
    crate::player::control(crate::player::Control::Stop);
    crate::live::stop(app);
    app.update_settings(|s| {
        s.auth.token = None;
        s.server = Default::default();
    });
    images::set_server(None);
    *app.pending.lock().unwrap() = None;
    app.set_session(None, Vec::new());
    app.set_stage(Stage::Login);
}

pub struct NewPin {
    pub id: i64,
    pub code: String,
    pub url: String,
    pub attempt: u64,
}

/// Start a PIN sign-in. `strong` PINs are approved in the browser; short
/// ones are typed in at plex.tv/link.
pub async fn create_pin(app: &AppState, strong: bool) -> Result<NewPin> {
    let tv = tv(app, None)?;
    let pin = tv.create_pin(strong).await.context("contacting plex.tv")?;
    let code = pin.code.clone().unwrap_or_default();
    let url = tv.auth_url(&code);
    let attempt = app.login_attempt.fetch_add(1, Ordering::SeqCst) + 1;
    Ok(NewPin { id: pin.id.ok_or_else(|| anyhow!("plex.tv returned no PIN"))?, code, url, attempt })
}

/// Poll a PIN until it is claimed (or 15 minutes pass), then sign in.
/// Returns early without signing in when the attempt is cancelled.
pub async fn complete_pin(app: &AppState, pin_id: i64, attempt: u64) -> Result<()> {
    let tv = tv(app, None)?;
    let current = || app.login_attempt.load(Ordering::SeqCst) == attempt;
    for _ in 0..450 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        if !current() {
            return Ok(());
        }
        if let Ok(pin) = tv.check_pin(pin_id).await
            && let Some(token) = pin.auth_token.filter(|t| !t.is_empty())
        {
            if current() {
                run(app, sign_in(app, token)).await;
            }
            return Ok(());
        }
    }
    Err(anyhow!("The sign-in code expired. Please try again."))
}

pub fn cancel_pin(app: &AppState) {
    app.login_attempt.fetch_add(1, Ordering::SeqCst);
}

pub async fn password_sign_in(app: &AppState, login: &str, password: &str, code: Option<&str>) -> Result<()> {
    let tv = tv(app, None)?;
    let user = tv.sign_in(login, password, code).await.map_err(|e| match e.status() {
        Some(401) => anyhow!("Incorrect email/username or password."),
        _ => anyhow!(e),
    })?;
    let token = user.auth_token.ok_or_else(|| anyhow!("plex.tv returned no token"))?;
    run(app, sign_in(app, token)).await;
    Ok(())
}
