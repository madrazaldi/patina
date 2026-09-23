//! Sign-in, server discovery and connection management.

use anyhow::{Context, Result, anyhow};
use dioxus::core::spawn_forever;
use dioxus::prelude::*;
use patina_plex::{PlexServer, PlexTv, Resource, User, connect_resource};

use crate::images;
use crate::state::{AppState, Session};

/// Where the app is in the sign-in / connect flow.
#[derive(Clone, PartialEq)]
pub enum Stage {
    Booting,
    Login,
    PickServer { servers: Vec<Resource>, user: Box<User> },
    Connecting(String),
    Ready,
    Failed(String),
}

fn tv(app: &AppState, token: Option<String>) -> Result<PlexTv> {
    Ok(PlexTv::new(app.client_info(), token)?)
}

/// Restore the previous session on startup (or after changing accounts).
pub async fn restore(app: AppState) {
    let mut stage = app.stage;
    let settings = app.settings.peek().clone();
    if let (Some(url), Some(token)) = (settings.server.manual_url.clone(), settings.server.manual_token.clone()) {
        stage.set(Stage::Connecting(url.clone()));
        match connect_manual(app, &url, &token).await {
            Ok(()) => stage.set(Stage::Ready),
            Err(e) => stage.set(Stage::Failed(format!("{e:#}"))),
        }
        return;
    }
    let Some(token) = settings.auth.token.clone() else {
        stage.set(Stage::Login);
        return;
    };
    if let Err(e) = sign_in(app, token).await {
        stage.set(Stage::Failed(format!("{e:#}")));
    }
}

/// Validate a plex.tv token and connect to the preferred server.
pub async fn sign_in(app: AppState, token: String) -> Result<()> {
    let mut stage = app.stage;
    stage.set(Stage::Connecting("plex.tv".into()));
    let tv = tv(&app, Some(token.clone()))?;
    let user = match tv.user().await {
        Ok(u) => u,
        Err(patina_plex::Error::Unauthorized) => {
            app.update_settings(|s| s.auth.token = None);
            stage.set(Stage::Login);
            return Ok(());
        }
        Err(e) => return Err(e).context("reaching plex.tv"),
    };
    app.update_settings(|s| s.auth.token = Some(token));
    let servers = tv.servers().await.context("listing servers")?;
    if servers.is_empty() {
        return Err(anyhow!("No Plex Media Servers are available to {}.", user.display_name()));
    }
    let preferred = app.settings.peek().server.id.clone();
    let choice = preferred
        .and_then(|id| servers.iter().find(|s| s.id() == id).cloned())
        .or_else(|| (servers.len() == 1).then(|| servers[0].clone()));
    match choice {
        Some(resource) => connect(app, tv, user, resource, servers).await,
        None => {
            stage.set(Stage::PickServer { servers, user: Box::new(user) });
            Ok(())
        }
    }
}

/// Connect to one server resource and make it the active session.
pub async fn connect(app: AppState, tv: PlexTv, user: User, resource: Resource, servers: Vec<Resource>) -> Result<()> {
    let mut stage = app.stage;
    stage.set(Stage::Connecting(resource.name().to_string()));
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

async fn connect_manual(app: AppState, url: &str, token: &str) -> Result<()> {
    let server = PlexServer::new(app.client_info(), url, token)?;
    let id = server.identity().await.context("contacting server")?;
    let server = server.with_machine_identifier(id.machine_identifier.clone());
    let caps = server.capabilities().await.context("server rejected the token")?;
    let name = caps.attr("friendlyName").unwrap_or_else(|| url.to_string());
    // A server token usually also works against plex.tv (watchlist, users).
    let tv = tv(&app, Some(token.to_string())).ok();
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

/// Save a manual server and connect to it.
pub async fn sign_in_manual(app: AppState, url: String, token: String) -> Result<()> {
    let url = if url.starts_with("http") { url } else { format!("http://{url}") };
    connect_manual(app, &url, &token).await?;
    app.update_settings(|s| {
        s.server.manual_url = Some(url);
        s.server.manual_token = Some(token);
    });
    let mut stage = app.stage;
    stage.set(Stage::Ready);
    Ok(())
}

async fn activate(app: AppState, session: Session) -> Result<()> {
    let sections = session.server.sections().await.context("loading libraries")?;
    images::set_server(Some(session.server.clone()));
    let (mut s, mut secs, mut stage) = (app.session, app.sections, app.stage);
    secs.set(sections.into_iter().filter(|d| !d.hidden).collect());
    s.set(Some(session));
    stage.set(Stage::Ready);
    app.bump();
    Ok(())
}

pub async fn reload_sections(app: AppState) {
    let Some(server) = app.session.peek().as_ref().map(|s| s.server.clone()) else { return };
    if let Ok(sections) = server.sections().await {
        let mut secs = app.sections;
        secs.set(sections.into_iter().filter(|d| !d.hidden).collect());
    }
}

/// Switch to another server of the same account.
pub async fn switch_server(app: AppState, resource: Resource) {
    let Some(session) = app.session.peek().clone() else { return };
    let (Some(tv), Some(user)) = (session.tv.clone(), session.user.clone()) else { return };
    if let Err(e) = connect(app, tv, user, resource, session.servers.clone()).await {
        app.error(format!("{e:#}"));
        let mut stage = app.stage;
        stage.set(Stage::Ready);
    }
}

/// Switch Plex Home user; the new user's token replaces the stored one.
pub async fn switch_user(app: AppState, uuid: String, pin: Option<String>) -> Result<()> {
    let tv = app.tv().ok_or_else(|| anyhow!("not signed in to plex.tv"))?;
    let user = tv.switch_home_user(&uuid, pin.as_deref()).await.context("switching user")?;
    let token = user.auth_token.clone().ok_or_else(|| anyhow!("plex.tv returned no token"))?;
    let mut session = app.session;
    session.set(None);
    sign_in(app, token).await
}

pub fn sign_out(app: AppState) {
    crate::player::control(crate::player::Control::Stop);
    app.update_settings(|s| {
        s.auth.token = None;
        s.server = Default::default();
    });
    images::set_server(None);
    let (mut session, mut stage, mut sections) = (app.session, app.stage, app.sections);
    session.set(None);
    sections.set(Vec::new());
    stage.set(Stage::Login);
}

/// Poll a link PIN until it is claimed (or 15 minutes pass).
pub async fn wait_for_pin(app: AppState, pin_id: i64) -> Result<String> {
    let tv = tv(&app, None)?;
    for _ in 0..450 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        if let Ok(pin) = tv.check_pin(pin_id).await
            && let Some(token) = pin.auth_token.filter(|t| !t.is_empty()) {
                return Ok(token);
            }
    }
    Err(anyhow!("The sign-in code expired. Please try again."))
}

pub async fn create_pin(app: AppState, strong: bool) -> Result<(patina_plex::Pin, String)> {
    let tv = tv(&app, None)?;
    let pin = tv.create_pin(strong).await.context("contacting plex.tv")?;
    let url = tv.auth_url(pin.code.as_deref().unwrap_or(""));
    Ok((pin, url))
}

pub async fn password_sign_in(app: AppState, login: String, password: String, code: Option<String>) -> Result<String> {
    let tv = tv(&app, None)?;
    let user = tv.sign_in(&login, &password, code.as_deref()).await.map_err(|e| match e.status() {
        Some(401) => anyhow!("Incorrect email/username or password."),
        _ => anyhow!(e),
    })?;
    user.auth_token.ok_or_else(|| anyhow!("plex.tv returned no token"))
}

/// Spawn a session task at the app root so it outlives the calling component.
pub fn run(app: AppState, fut: impl std::future::Future<Output = Result<()>> + 'static) {
    spawn_forever(async move {
        if let Err(e) = fut.await {
            let mut stage = app.stage;
            stage.set(Stage::Failed(format!("{e:#}")));
        }
    });
}
