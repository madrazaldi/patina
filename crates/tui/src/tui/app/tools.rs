//! The tool screens: downloads, Live TV, activity, server tools, settings
//! and the account, whose rows act through [`Act`].

use super::*;
use patina_core::downloads::State;
use patina_core::ops::server::{self as admin, LibraryForm};
use patina_core::ops::{playback, setup};
use patina_core::{livetv, session};

use crate::tui::rows::{self, AccountOp, Act, ServerOp, SettingKey};
use crate::tui::source::Group;

const LIBRARY_KINDS: [(&str, &str); 4] = [("movie", "Movies"), ("show", "TV Shows"), ("artist", "Music"), ("photo", "Photos")];

impl App {
    pub(super) fn root_items(&self) -> Vec<Item> {
        if self.offline {
            return vec![Item::nav("Downloads", Source::Downloads), Item::nav("Settings", Source::Settings)];
        }
        let mut items = vec![Item::nav("Home", Source::Home)];
        items.extend(self.sections.iter().cloned());
        items.extend([
            Item::nav("Playlists", Source::Path(PLAYLISTS.into())),
            Item::nav("Watchlist", Source::Watchlist),
            Item::nav("Pick something to watch", Source::Pick),
            Item::nav("Search", Source::Search),
        ]);
        // Like the GUI's sidebar, Downloads only shows once there is one (or a smart download policy).
        if !self.downloads.is_empty() || patina_core::smart::overview(&self.core).policies.iter().any(|p| p.active) {
            items.push(Item::nav("Downloads", Source::Downloads));
        }
        if self.live_tv {
            items.push(Item::group(
                "Live TV",
                Source::Group(Group::LiveTv),
                vec![Item::nav("Guide", Source::Guide), Item::nav("Upcoming recordings", Source::Upcoming), Item::nav("Recording rules", Source::Rules)],
            ));
        }
        items.push(Item::group(
            "Activity",
            Source::Group(Group::Activity),
            vec![Item::nav("Now Playing", Source::Sessions), Item::nav("History", Source::History(1)), Item::nav("Tasks", Source::Tasks)],
        ));
        if self.owned() {
            items.push(Item::group(
                "Server",
                Source::Group(Group::Server),
                vec![
                    Item::nav("Overview & updates", Source::Overview),
                    Item::nav("Streaming · last 24 hours", Source::Stats(false)),
                    Item::nav("Streaming · last 30 days", Source::Stats(true)),
                    Item::nav("Libraries", Source::ServerLibraries),
                    Item::nav("Maintenance", Source::Maintenance),
                    Item::nav("Settings", Source::Prefs),
                    Item::nav("Users & devices", Source::People),
                ],
            ));
        }
        items.push(Item::nav("Settings", Source::Settings));
        items
    }

    pub(super) fn rebuild_root(&mut self) {
        let items = self.root_items();
        if let Some(root) = self.columns.first_mut() {
            root.set_items(items, &mut self.fuzzy);
        }
    }

    /// Rows of a listing built from local state.
    pub(super) fn local_items(&self, source: &Source) -> Vec<Item> {
        match source {
            Source::Downloads => {
                let mut items = rows::smart_downloads(&patina_core::smart::overview(&self.core));
                items.extend(rows::downloads(&self.downloads));
                items
            }
            Source::Settings => {
                rows::settings(&self.core.settings(), self.mpv_probe.as_ref(), self.session.as_ref(), self.offline, self.auto_prefs().as_ref())
            }
            Source::ServerLibraries => rows::libraries(&self.sections, &self.activities),
            Source::Pick => pick::screen(&self.pick),
            _ => Vec::new(),
        }
    }

    /// Rebuild the open local listings of `source`.
    pub(super) fn refresh_local(&mut self, source: &Source) {
        for idx in 0..self.columns.len() {
            if self.columns[idx].source == *source {
                self.load(idx);
            }
        }
    }

    /// Show Live TV in the root when the server has a DVR, as the GUI does.
    pub(super) fn check_live_tv(&mut self) {
        self.live_tv = false;
        let Ok(server) = self.core.server() else { return };
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(Msg::LiveTv(livetv::available(&server).await));
        });
    }

    pub(super) fn probe_mpv(&mut self) {
        self.mpv_probe = None;
        let (core, tx) = (self.core.clone(), self.tx.clone());
        tokio::spawn(async move {
            let _ = tx.send(Msg::Probe(setup::probe_mpv(&core).await.map_err(err)));
        });
    }

    /// Browse downloads (and settings) while the server can't be reached.
    pub(super) fn start_offline(&mut self) {
        self.offline = true;
        self.columns.clear();
        self.details.clear();
        self.modal = None;
        let mut root = self.new_column("Offline", Source::Root);
        root.items = self.root_items();
        root.loaded = true;
        root.rebuild(&mut self.fuzzy);
        self.columns.push(root);
        self.drill();
    }

    // ---- enter, x and the menu on a row --------------------------------------

    pub(super) fn row_enter(&mut self, item: Item) {
        let Some(r) = item.row.as_deref() else { return };
        use RowOp::*;
        let op = match &r.act {
            Act::Download(d) if d.state == State::Done => Play,
            Act::Download(d) if matches!(d.state, State::Failed | State::Paused) => Retry,
            Act::Download(d) if matches!(d.state, State::Queued | State::Downloading) => PauseDownload,
            Act::Channel(_) => Watch,
            Act::Program { .. } => Record,
            Act::MoreHistory => Later,
            Act::Butler { .. } => Start,
            Act::Server(_) | Act::Account(_) | Act::OpenUrl(_) | Act::AutoClear | Act::SmartRun => Run,
            Act::Pref { .. } | Act::Setting(_) | Act::Auto(_) | Act::AutoShow { .. } | Act::Budget | Act::Policy(_) => Change,
            Act::Pick(field) => return self.pick_row(*field),
            Act::Library(_) => return self.row_menu(item),
            _ => return,
        };
        self.run_row(op);
    }

    /// `x` on a row; `false` when it has no destructive action.
    pub(super) fn row_delete(&mut self, item: &Item) -> bool {
        let Some(r) = item.row.as_deref() else { return false };
        let op = match &r.act {
            Act::Download(_) => RowOp::Remove,
            Act::Rule(_) => RowOp::DeleteRule,
            Act::Activity { cancellable: true, .. } => RowOp::Cancel,
            Act::Transcode(_) => RowOp::Kill,
            Act::Butler { .. } => RowOp::Stop,
            Act::Library(_) => RowOp::DeleteLibrary,
            Act::Policy(_) | Act::AutoShow { .. } => RowOp::Remove,
            _ => return false,
        };
        self.run_row(op);
        true
    }

    pub(super) fn row_menu(&mut self, item: Item) {
        let Some(r) = item.row.as_deref() else { return };
        use RowOp::*;
        let mut rows: Vec<(RowOp, &'static str, &'static str)> = Vec::new();
        match &r.act {
            Act::Download(d) => {
                if d.state == State::Done {
                    rows.push((Play, "Play", "enter"));
                    rows.push((PlayGroup, "Play the group", "a"));
                }
                if matches!(d.state, State::Failed | State::Paused) {
                    rows.push((Retry, "Resume / retry", "enter"));
                }
                if matches!(d.state, State::Queued | State::Downloading) {
                    rows.push((PauseDownload, "Pause download", "enter"));
                }
                if self.download_on_server(d).is_some() {
                    rows.push((GoToItem, "Go to the item", ""));
                }
                rows.push((Remove, if d.state == State::Done { "Delete download…" } else { "Remove from downloads…" }, "x"));
            }
            Act::Channel(ch) => {
                rows.push((Watch, "Watch", "enter"));
                if ch.programs.first().is_some_and(|p| p.guid.is_some()) {
                    rows.push((Record, "Record what's on", ""));
                }
                rows.push((Later, "Later programs", "l"));
            }
            Act::Program { program } if program.guid.is_some() => rows.push((Record, "Record", "enter")),
            Act::Rule(_) => rows.push((DeleteRule, "Delete rule…", "x")),
            Act::Activity { cancellable: true, .. } => rows.push((Cancel, "Cancel", "x")),
            Act::Transcode(_) => rows.push((Kill, "Stop transcode", "x")),
            Act::MoreHistory => rows.push((Later, "Load older history", "enter")),
            Act::Butler { pref, .. } => {
                rows.extend([(Start, "Run now", "enter"), (Stop, "Stop", "x")]);
                if let Some(p) = pref {
                    let on = p.value == "true" || p.value == "1";
                    rows.push((Change, if on { "Turn off scheduled runs" } else { "Turn on scheduled runs" }, ""));
                }
                rows.extend([(StartAll, "Start all tasks", ""), (StopAll, "Stop all tasks", "")]);
            }
            Act::Server(_) | Act::Account(_) | Act::OpenUrl(_) => rows.push((Run, "Run", "enter")),
            Act::Pref { .. } | Act::Setting(_) | Act::Auto(_) => rows.push((Change, "Change…", "enter")),
            Act::AutoShow { .. } => rows.extend([(Change, "Edit…", "enter"), (Remove, "Clear this show's preferences…", "x")]),
            Act::AutoClear => rows.push((Run, "Clear all rules…", "enter")),
            Act::Budget => rows.push((Change, "Set the storage budget…", "enter")),
            Act::SmartRun => rows.push((Run, "Check now", "enter")),
            Act::Policy(p) => {
                rows.push((Change, "Change how many to keep…", "enter"));
                rows.push((KeepWatched, if p.policy.delete_watched { "Keep watched downloads" } else { "Remove downloads once watched" }, ""));
                rows.push((Toggle, if p.policy.enabled { "Pause" } else { "Resume" }, ""));
                rows.push((Run, "Check now", ""));
                rows.push((Remove, "Stop keeping downloaded…", "x"));
                if p.downloads > 0 {
                    rows.push((RemoveFiles, "Stop keeping and delete its downloads…", ""));
                }
            }
            Act::Library(d) => {
                let id = d.key.clone().unwrap_or_default();
                let scanning = d.refreshing || self.activities.iter().any(|a| a.section_id.as_deref() == Some(id.as_str()));
                rows.push(if scanning { (CancelScan, "Cancel scan", "") } else { (Scan, "Scan library files", "") });
                rows.extend([
                    (RefreshLibrary, "Refresh all metadata", ""),
                    (AnalyzeLibrary, "Analyze", ""),
                    (EmptyTrash, "Empty trash…", ""),
                    (EditLibrary, "Edit name & folders…", ""),
                    (LibrarySettings, "Library settings", "l"),
                    (BrowseLibrary, "Browse the library", ""),
                    (DeleteLibrary, "Delete library…", "x"),
                ]);
            }
            _ => {}
        }
        if self.columns.last().is_some_and(|c| c.source == Source::Downloads) {
            rows.push((OpenFolder, "Open downloads folder", ""));
        }
        if rows.is_empty() {
            return;
        }
        let rows = rows.into_iter().map(|(op, label, key)| (MenuAction::Row(op), label, key)).collect();
        self.modal = Some(Modal::Menu(Menu { title: item.title().to_string(), rows, cursor: 0 }));
    }

    /// The library item a download came from, when this server has it.
    fn download_on_server(&self, d: &Download) -> Option<String> {
        let sid = self.session.as_ref()?.server_id.clone();
        (d.server_id == sid && !self.offline).then(|| d.item.rating_key.clone()).flatten()
    }

    pub(super) fn run_row(&mut self, op: RowOp) {
        let Some(item) = self.selected().cloned() else { return };
        let Some(r) = item.row.clone() else {
            if op == RowOp::OpenFolder {
                self.open_downloads_folder();
            }
            return;
        };
        let title = item.title().to_string();
        use RowOp::*;
        if self.auto_row_op(op, &r.act) {
            return;
        }
        match (op, r.act) {
            (OpenFolder, _) => self.open_downloads_folder(),
            (Play, Act::Download(d)) => self.play_downloads(vec![d.id.clone()]),
            (PlayGroup, Act::Download(d)) => {
                let group = rows::download_group(&d);
                let ids = self
                    .columns
                    .last()
                    .map_or(&[][..], |c| &c.items[..])
                    .iter()
                    .filter_map(|i| match i.row.as_ref().map(|r| &r.act) {
                        Some(Act::Download(x)) if x.state == State::Done && rows::download_group(x) == group => Some(x.id.clone()),
                        _ => None,
                    })
                    .collect();
                self.play_downloads(ids);
            }
            (Retry, Act::Download(d)) => downloads::retry(&self.core, &d.id),
            (PauseDownload, Act::Download(d)) => downloads::pause(&self.core, &d.id),
            (Remove, Act::Download(d)) => {
                let message = if d.state == State::Done {
                    format!("Delete the download of {title}? The file is removed from this computer.")
                } else {
                    format!("Stop downloading {title}?")
                };
                self.modal = Some(Modal::Confirm(Confirm { message, action: ConfirmAction::RemoveDownload { id: d.id.clone() } }));
            }
            (GoToItem, Act::Download(d)) => {
                if let Some(rk) = self.download_on_server(&d) {
                    let parent = match d.item.kind() {
                        "episode" | "track" => d.item.parent_rating_key.clone(),
                        _ => None,
                    };
                    let (path, name) = match parent {
                        Some(p) => (format!("/library/metadata/{p}/children"), Item::from(d.item.clone()).full_title().rsplit_once(" · ").map_or(String::new(), |(a, _)| a.to_string())),
                        None => (format!("/library/metadata/{rk}"), title.clone()),
                    };
                    self.open_selecting(name, path, rk);
                }
            }
            (Watch, Act::Channel(ch)) => {
                self.info(format!("Tuning {}…", ch.title));
                let core = self.core.clone();
                self.spawn_op(Refresh::Nothing, async move {
                    playback::livetv_watch(&core, &ch.dvr, &ch.id, &ch.title).await.map_err(err)?;
                    Ok(None)
                });
            }
            (Record, Act::Channel(ch)) => {
                if let Some(p) = ch.programs.first() {
                    self.record(p.guid.clone(), p.title.clone());
                }
            }
            (Record, Act::Program { program }) => self.record(program.guid.clone(), program.title.clone()),
            (Later, Act::Channel(_)) => self.drill(),
            (Later, Act::MoreHistory) => {
                let Some(idx) = self.columns.len().checked_sub(1) else { return };
                if let Source::History(n) = self.columns[idx].source {
                    self.columns[idx].source = Source::History(n + 1);
                    self.load(idx);
                }
            }
            (DeleteRule, Act::Rule(id)) => {
                self.modal = Some(Modal::Confirm(Confirm {
                    message: format!("Delete the recording rule for {title}? Existing recordings are kept."),
                    action: ConfirmAction::DeleteRule { id },
                }));
            }
            (Cancel, Act::Activity { uuid, .. }) => {
                self.server_op(Refresh::Source(Source::Tasks), "Cancelled", move |s| async move { s.cancel_activity(&uuid).await })
            }
            (Kill, Act::Transcode(key)) => {
                let core = self.core.clone();
                self.spawn_op(Refresh::Source(Source::Tasks), async move {
                    admin::kill_transcode(&core, &key).await.map_err(err)?;
                    Ok(Some("Transcode stopped".into()))
                });
            }
            (Start | Stop, Act::Butler { name, .. }) => {
                let start = op == Start;
                let done = if start { format!("Started {title}") } else { format!("Stopped {title}") };
                self.server_op(Refresh::Nothing, done, move |s| async move {
                    if start { s.start_butler_task(&name).await } else { s.stop_butler_task(&name).await }
                });
            }
            (StartAll, _) => self.server_run(ServerOp::ButlerStartAll),
            (StopAll, _) => self.server_run(ServerOp::ButlerStopAll),
            (Run, Act::Server(s)) => self.server_run(s),
            (Run, Act::Account(a)) => self.account_run(a),
            (Run, Act::OpenUrl(url)) => {
                if let Err(e) = open::that_detached(&url) {
                    self.error(format!("Couldn't open a browser: {e}"));
                }
            }
            (Change, Act::Setting(key)) => self.change_setting(key, title),
            (Change, Act::Pref { pref, section }) => self.change_pref(*pref, section),
            (Change, Act::Butler { pref: Some(pref), .. }) => self.change_pref(*pref, None),
            (op, Act::Library(d)) => self.library_op(op, *d),
            _ => {}
        }
    }

    fn play_downloads(&mut self, ids: Vec<String>) {
        if let Err(e) = playback::play_downloads(&self.core, &ids) {
            self.error(err(e));
        }
    }

    fn open_downloads_folder(&mut self) {
        let dir = self.core.settings().player.downloads_dir();
        let _ = std::fs::create_dir_all(&dir);
        match open::that_detached(&dir) {
            Ok(()) => self.info(format!("Opened {}", dir.display())),
            Err(e) => self.error(format!("Couldn't open {}: {e}", dir.display())),
        }
    }

    fn record(&mut self, guid: Option<String>, title: String) {
        let Some(guid) = guid else { return self.info("This program can't be recorded") };
        let server = self.server();
        self.spawn_op(Refresh::Source(Source::Upcoming), async move {
            livetv::record(&server?, &guid).await.map_err(err)?;
            Ok(Some(format!("Recording scheduled: {title}")))
        });
    }

    fn server_run(&mut self, op: ServerOp) {
        let core = self.core.clone();
        let (refresh, done): (Refresh, &str) = match op {
            ServerOp::CheckUpdates => (Refresh::Source(Source::Overview), "Checked for updates"),
            ServerOp::InstallUpdate => (Refresh::Nothing, "Installing update…"),
            ServerOp::UpdateTonight => (Refresh::Nothing, "Update scheduled for tonight"),
            ServerOp::ScanAll => (Refresh::Nothing, "Scanning all libraries…"),
            ServerOp::ReloadSections => (Refresh::Nothing, "Libraries reloaded"),
            ServerOp::CleanBundles => (Refresh::Nothing, "Cleaning bundles…"),
            ServerOp::OptimizeDatabase => (Refresh::Nothing, "Optimizing database…"),
            ServerOp::ButlerStartAll => (Refresh::Nothing, "Started all tasks"),
            ServerOp::ButlerStopAll => (Refresh::Nothing, "Stopped all tasks"),
            ServerOp::AddLibrary => {
                let rows = LIBRARY_KINDS.iter().map(|(k, l)| (PickEntry::Text(k.to_string()), l.to_string())).collect();
                return self.open_picker("Add library · type".into(), PickKind::LibraryKind, " enter choose · esc cancel ", async move { Ok(rows) });
            }
            ServerOp::SaveLogs | ServerOp::SaveDatabase => {
                let what = if op == ServerOp::SaveLogs { "logs" } else { "database" };
                self.info(format!("Downloading the server's {what}…"));
                return self.spawn_op(Refresh::Nothing, async move {
                    let path = admin::save_diagnostics(&core, what).await.map_err(err)?;
                    Ok(Some(format!("Saved to {}", path.display())))
                });
            }
        };
        self.spawn_op(refresh, async move {
            let server = core.server().map_err(err)?;
            let r = match op {
                ServerOp::CheckUpdates => server.check_for_updates(true).await,
                ServerOp::InstallUpdate => server.apply_update(false, false).await,
                ServerOp::UpdateTonight => server.apply_update(true, false).await,
                ServerOp::ScanAll => server.refresh_all_sections(false).await,
                ServerOp::CleanBundles => server.clean_bundles().await,
                ServerOp::OptimizeDatabase => server.optimize_database().await,
                ServerOp::ButlerStartAll => server.start_all_butler_tasks().await,
                ServerOp::ButlerStopAll => server.stop_all_butler_tasks().await,
                _ => return session::reload_sections(&core).await.map(|_| Some(done.to_string())).map_err(err),
            };
            r.map_err(err)?;
            Ok(Some(done.to_string()))
        });
    }

    fn account_run(&mut self, op: AccountOp) {
        let core = self.core.clone();
        match op {
            AccountOp::SwitchServer => {
                let Some(sv) = self.session.clone() else { return };
                if !sv.has_tv {
                    return self.info("Switching servers needs a plex.tv sign-in");
                }
                let rows = sv
                    .servers
                    .iter()
                    .map(|s| {
                        let current = if s.id == sv.server_id { "●" } else { " " };
                        let note = match (s.owned, s.presence) {
                            (_, false) => " · offline".to_string(),
                            (true, _) => String::new(),
                            (false, _) => format!(" · shared by {}", s.source_title.as_deref().unwrap_or("someone")),
                        };
                        (PickEntry::Text(s.id.clone()), format!("{current} {}{note}", s.name))
                    })
                    .collect();
                self.open_picker("Switch server".into(), PickKind::Servers, " enter connect · esc close ", async move { Ok(rows) });
            }
            AccountOp::SwitchUser => self.open_users(),
            AccountOp::SignOut => {
                self.modal = Some(Modal::Confirm(Confirm { message: "Sign out of Patina on this device?".into(), action: ConfirmAction::Logout }))
            }
            AccountOp::SignIn => session::sign_out(&core),
            AccountOp::Reconnect => {
                self.info("Connecting…");
                tokio::spawn(async move { session::retry(&core).await });
            }
        }
    }

    pub(super) fn switch_server(&mut self, id: String) {
        if self.session.as_ref().is_some_and(|s| s.server_id == id) {
            return self.info("Already connected to this server");
        }
        let core = self.core.clone();
        self.spawn_op(Refresh::Nothing, async move {
            session::switch_server(&core, &id).await.map_err(err)?;
            Ok(None)
        });
    }

    // ---- settings ------------------------------------------------------------

    fn change_setting(&mut self, key: SettingKey, title: String) {
        if key == SettingKey::MpvConfig {
            let dir = patina_core::config::mpv_config_dir();
            let _ = std::fs::create_dir_all(&dir);
            if let Err(e) = open::that_detached(&dir) {
                self.error(format!("Couldn't open {}: {e}", dir.display()));
            }
            return;
        }
        let settings = self.core.settings();
        if key.is_text() {
            let hint = match key {
                SettingKey::MpvPath => "Path to mpv; empty auto-detects",
                SettingKey::ExtraArgs => "e.g. --fs --profile=hq",
                SettingKey::SkipKey => "mpv key name, e.g. TAB",
                SettingKey::DiscordAppId => "Discord application ID",
                _ => "Folder; empty uses the default",
            };
            self.modal = Some(Modal::Input(Input { title, hint: hint.into(), value: key.value(&settings), purpose: InputPurpose::Setting(key) }));
            return;
        }
        let choices = key.choices();
        if choices.is_empty() {
            return self.save_setting(key, None);
        }
        let current = key.value(&settings);
        let rows = choices.into_iter().map(|(v, l)| (PickEntry::Text(v.clone()), format!("{} {l}", if v == current { "●" } else { " " }))).collect();
 self.open_picker(title, PickKind::Setting(key), " enter choose · esc cancel ", async move { Ok(rows) });
    }

    /// Store a setting (`None` flips a switch) in the shared config file.
    pub(super) fn save_setting(&mut self, key: SettingKey, value: Option<String>) {
        self.core.update_settings(|s| match &value {
            Some(v) => key.set(s, v),
            None => {
                key.toggle(s);
            }
        });
        let s = self.core.settings();
        self.tui = s.tui.clone();
        match key {
            SettingKey::Inspector => self.show_inspector = s.tui.show_inspector,
            SettingKey::LibraryCounts => {
                self.load_counts();
            }
            SettingKey::MpvPath => self.probe_mpv(),
            _ => {}
        }
        if matches!(key, SettingKey::Discord | SettingKey::DiscordAppId) {
            patina_core::presence::update(self.now_playing.as_ref(), &s.player);
        }
        self.refresh_local(&Source::Settings);
    }

    fn change_pref(&mut self, pref: admin::Pref, section: Option<String>) {
        let on = |v: &str| v == "true" || v == "1";
        if pref.kind == "bool" {
            let value = if on(&pref.value) { "false" } else { "true" };
            return self.save_pref(pref.id, section, value.into());
        }
        if !pref.options.is_empty() {
            let rows = pref.options.iter().map(|(v, l)| (PickEntry::Text(v.clone()), format!("{} {l}", if *v == pref.value { "●" } else { " " }))).collect();
            let kind = PickKind::Pref { id: pref.id.clone(), section };
            return self.open_picker(pref.label, kind, " enter choose · esc cancel ", async move { Ok(rows) });
        }
        self.modal = Some(Modal::Input(Input {
            title: pref.label.clone(),
            hint: pref.id.clone(),
            value: pref.value.clone(),
            purpose: InputPurpose::Pref { id: pref.id, section },
        }));
    }

    pub(super) fn save_pref(&mut self, id: String, section: Option<String>, value: String) {
        // Plex prefixes every scheduled-task setting with `Butler`; those live under Maintenance.
        let refresh = Refresh::Source(match &section {
            Some(s) => Source::LibraryPrefs(s.clone()),
            None if id.starts_with("Butler") => Source::Maintenance,
            None => Source::Prefs,
        });
        self.server_op(refresh, "Setting saved", move |s| async move {
            match section {
                Some(sid) => s.set_section_prefs(&sid, &Params::new().set(id, value)).await,
                None => s.set_pref(&id, &value).await,
            }
        });
    }

    // ---- libraries -------------------------------------------------------------

    fn library_op(&mut self, op: RowOp, d: patina_plex::Directory) {
        let id = d.key.clone().unwrap_or_default();
        let title = d.title().to_string();
        use RowOp::*;
        match op {
            Scan | RefreshLibrary | AnalyzeLibrary => {
                let done = match op {
                    Scan => format!("Scanning {title}…"),
                    RefreshLibrary => format!("Refreshing {title}…"),
                    _ => format!("Analyzing {title}…"),
                };
                self.server_op(Refresh::Nothing, done, move |s| async move {
                    match op {
                        Scan => s.refresh_section(&id, false, None).await,
                        RefreshLibrary => s.refresh_section(&id, true, None).await,
                        _ => s.analyze_section(&id).await,
                    }
                });
            }
            CancelScan => self.server_op(Refresh::Nothing, "Scan cancelled", move |s| async move { s.cancel_refresh_section(&id).await }),
            EmptyTrash => {
                self.modal = Some(Modal::Confirm(Confirm {
                    message: format!("Permanently remove {title}'s missing items?"),
                    action: ConfirmAction::EmptyTrash { section_id: id },
                }))
            }
            EditLibrary => {
                let folders: Vec<String> = d.location.iter().filter_map(|l| l.path.clone()).collect();
                self.modal = Some(Modal::Input(Input {
                    title: format!("Edit {title} · name"),
                    hint: "Library name".into(),
                    value: title,
                    purpose: InputPurpose::EditLibraryName { id, folders: folders.join("; ") },
                }));
            }
            LibrarySettings => self.drill(),
            BrowseLibrary => {
                if let Some(v) = self.sections.iter().find(|s| s.key.as_deref() == Some(id.as_str())).and_then(nav::library_view) {
                    self.open_source(title, Source::Library(v), Vec::new());
                }
            }
            DeleteLibrary => {
                self.modal = Some(Modal::Confirm(Confirm {
                    message: format!("Delete the library {title}? It and its metadata are removed from the server; media files on disk are not touched."),
                    action: ConfirmAction::DeleteLibrary { id, title },
                }))
            }
            _ => {}
        }
    }

    /// The next step of adding a library, after `form` got another answer.
    pub(super) fn library_form_step(&mut self, mut form: LibraryForm, step: &str) {
        let label = LIBRARY_KINDS.iter().find(|k| k.0 == form.kind).map_or("Library", |k| k.1);
        match step {
            "name" => {
                self.modal = Some(Modal::Input(Input {
                    title: format!("Add library · {label}"),
                    hint: "Library name".into(),
                    value: label.into(),
                    purpose: InputPurpose::LibraryName(Box::new(form)),
                }))
            }
            "folders" => {
                self.modal = Some(Modal::Input(Input {
                    title: format!("Add library · {}", form.name),
                    hint: "Folders on the server, separated by ;".into(),
                    value: String::new(),
                    purpose: InputPurpose::LibraryFolders(Box::new(form)),
                }))
            }
            "agent" | "scanner" => {
                let core = self.core.clone();
                let kind = form.kind.clone();
                let agent = step == "agent";
                let pick = if agent { PickKind::LibraryAgent(Box::new(form.clone())) } else { PickKind::LibraryScanner(Box::new(form.clone())) };
                let title = format!("Add library · {}", if agent { "agent" } else { "scanner" });
                self.open_picker(title, pick, " enter choose · esc cancel ", async move {
                    let c = admin::library_choices(&core, &kind).await.map_err(err)?;
                    Ok(if agent {
                        c.agents.into_iter().map(|(id, t)| (PickEntry::Text(id.clone()), if t.is_empty() { id } else { t })).collect()
                    } else {
                        c.scanners.into_iter().map(|s| (PickEntry::Text(s.clone()), s)).collect()
                    })
                });
            }
            "language" => {
                self.modal = Some(Modal::Input(Input {
                    title: format!("Add library · {}", form.name),
                    hint: "Language".into(),
                    value: "en-US".into(),
                    purpose: InputPurpose::LibraryLanguage(Box::new(form)),
                }))
            }
            _ => {
                if form.language.trim().is_empty() {
                    form.language = "en-US".into();
                }
                let core = self.core.clone();
                let name = form.name.clone();
                self.spawn_op(Refresh::Nothing, async move {
                    admin::create_library(&core, &form).await.map_err(err)?;
                    Ok(Some(format!("Added {name}")))
                });
            }
        }
    }

    pub(super) fn edit_library(&mut self, id: String, name: String, folders: &str) {
        let locations: Vec<String> = folders.split(';').map(|f| f.trim().to_string()).filter(|f| !f.is_empty()).collect();
        if locations.is_empty() {
            return self.error("A library needs at least one folder");
        }
        let core = self.core.clone();
        self.spawn_op(Refresh::Nothing, async move {
            admin::edit_library(&core, &id, &name, &locations).await.map_err(err)?;
            Ok(Some("Library updated".into()))
        });
    }

    // ---- go to the parent -----------------------------------------------------

    /// Open the column listing an item, selecting it once loaded.
    pub(super) fn open_selecting(&mut self, title: String, path: String, select: String) {
        self.search = None;
        self.open_column(title, path);
        if let Some(col) = self.columns.last_mut() {
            col.select_on_load = Some(select);
        }
    }

    /// `b`/`B`: the season or album (`B`: the show or artist) of the selection, as the GUI's "Go to …".
    pub(super) fn go_to_parent(&mut self, grand: bool) {
        let Some(item) = self.selected().cloned() else { return };
        let kind = item.kind();
        let target = match (kind, grand) {
            (Kind::Episode | Kind::Track, false) => item.parent_rating_key.clone().map(|k| (k, item.parent_title.clone(), item.rating_key.clone())),
            (Kind::Episode | Kind::Track, true) => item.grandparent_rating_key.clone().map(|k| (k, item.grandparent_title.clone(), item.parent_rating_key.clone())),
            (Kind::Season | Kind::Album, _) => item.parent_rating_key.clone().map(|k| (k, item.parent_title.clone(), item.rating_key.clone())),
            _ => None,
        };
        let Some((key, title, select)) = target else {
            return self.info("Go to show, season, artist or album works on episodes, seasons, tracks and albums");
        };
        let title = title.unwrap_or_default();
        self.open_selecting(title, format!("/library/metadata/{key}/children"), select.unwrap_or_default());
    }
}
