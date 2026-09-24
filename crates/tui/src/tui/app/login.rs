use super::*;
use crossterm::event::{KeyCode, KeyEvent};
use patina_core::session;

impl App {
    // ---- sign-in ----------------------------------------------------------

    pub(super) fn on_stage(&mut self, stage: Stage) {
        match &stage {
            Stage::Login => {
                self.columns.clear();
                self.login = Login::default();
                self.request_pin();
            }
            Stage::PickServer { .. } => self.login.cursor = 0,
            Stage::Ready => {
                self.stage = stage;
                self.start();
                return;
            }
            _ => {}
        }
        self.stage = stage;
    }

    /// Ask plex.tv for a link code and wait for it to be claimed.
    pub(super) fn request_pin(&mut self) {
        self.login.requesting = true;
        self.login.error = None;
        let (core, tx) = (self.core.clone(), self.tx.clone());
        tokio::spawn(async move {
            match session::create_pin(&core, false).await {
                Ok(pin) => {
                    let _ = tx.send(Msg::Pin(Ok((pin.code.clone(), pin.url.clone()))));
                    if let Err(e) = session::complete_pin(&core, pin.id, pin.attempt).await {
                        let _ = tx.send(Msg::Pin(Err(err(e))));
                    }
                }
                Err(e) => {
                    let _ = tx.send(Msg::Pin(Err(err(e))));
                }
            }
        });
    }

    pub(super) fn sign_in_manual(&mut self, url: String, token: String) {
        session::cancel_pin(&self.core);
        self.login.code = None;
        self.login.error = None;
        self.login.requesting = true;
        let (core, tx) = (self.core.clone(), self.tx.clone());
        tokio::spawn(async move {
            let result = session::sign_in_manual(&core, &url, &token).await;
            if let Err(e) = result {
                let _ = tx.send(Msg::Pin(Err(format!("Could not connect: {}", err(e)))));
            }
        });
    }

    pub(super) fn stage_key(&mut self, key: KeyEvent) {
        let core = self.core.clone();
        match (&self.stage, key.code) {
            (_, KeyCode::Char('q')) => self.quit = true,
            (Stage::Login, KeyCode::Char('r')) => {
                session::cancel_pin(&core);
                self.request_pin();
            }
            (Stage::Login, KeyCode::Char('m')) => {
                self.modal = Some(Modal::Input(Input {
                    title: "Connect to a server".into(),
                    hint: "Server URL (e.g. 192.168.1.10:32400)".into(),
                    value: String::new(),
                    purpose: InputPurpose::ServerUrl,
                }))
            }
            (Stage::Login, KeyCode::Char('o')) => {
                if let Some((_, url)) = &self.login.code
                    && let Err(e) = open::that_detached(url)
                {
                    self.login.error = Some(format!("Couldn't open a browser: {e}"));
                }
            }
            (Stage::Failed { .. }, KeyCode::Char('r') | KeyCode::Enter) => {
                tokio::spawn(async move { session::retry(&core).await });
            }
            (Stage::Failed { .. }, KeyCode::Char('L')) => session::sign_out(&core),
            (Stage::PickServer { servers, .. }, code) => {
                let n = servers.len();
                match code {
                    KeyCode::Char('j') | KeyCode::Down => self.login.cursor = (self.login.cursor + 1).min(n.saturating_sub(1)),
                    KeyCode::Char('k') | KeyCode::Up => self.login.cursor = self.login.cursor.saturating_sub(1),
                    KeyCode::Enter => {
                        if let Some(s) = servers.get(self.login.cursor) {
                            let id = s.id.clone();
                            tokio::spawn(async move { session::run(&core, session::pick_server(&core, &id)).await });
                        }
                    }
                    KeyCode::Char('L') | KeyCode::Esc => session::sign_out(&core),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
