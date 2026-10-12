//! Automatic playback preferences (per profile and per show) and smart
//! downloads: the dialogs and row actions around `patina_core::prefs` and
//! `patina_core::smart`. The rules themselves live in the core.

use super::*;
use patina_core::smart::{self, Patch, PolicyView};
use patina_core::prefs::{self, Rules};

use crate::tui::auto::{AutoField, AutoPrefs, FIELDS};
use crate::tui::rows::Act;

/// Bytes in a GB, as sizes are shown.
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

type Show = (String, String);

/// The show an item belongs to (itself for a show), as the track dialog finds it.
fn show_of(item: &Item) -> Option<Show> {
    match item.kind() {
        Kind::Show => Some((item.rating_key.clone()?, item.title().to_string())),
        Kind::Season => Some((item.parent_rating_key.clone()?, item.parent_title.clone().unwrap_or_default())),
        Kind::Episode => Some((item.grandparent_rating_key.clone()?, item.grandparent_title.clone().unwrap_or_default())),
        _ => None,
    }
}

impl App {
    /// The profile's rules and the shows with their own, for the settings rows.
    pub(super) fn auto_prefs(&self) -> Option<AutoPrefs> {
        self.session.as_ref()?;
        Some(AutoPrefs { profile: prefs::view(&self.core, None).ok()?.profile, shows: prefs::shows(&self.core).ok()? })
    }

    /// The rules being edited: a show's own, or the profile's.
    fn auto_rules(&self, show: &Option<Show>) -> Rules {
        match prefs::view(&self.core, show.as_ref().map(|s| s.0.as_str())) {
            Ok(v) => match show {
                Some(_) => v.show.unwrap_or_default(),
                None => v.profile,
            },
            Err(_) => Rules::default(),
        }
    }

    /// Enter on a rule: type its value, or pick it from a list.
    pub(super) fn change_auto(&mut self, field: AutoField, show: Option<Show>) {
        let rules = self.auto_rules(&show);
        let scope = show.as_ref().map_or("profile".to_string(), |s| s.1.clone());
        let title = format!("{} · {scope}", field.title());
        if field.is_text() {
            let hint = if show.is_some() { "e.g. eng, ja · empty: same as the profile" } else { "e.g. eng, ja · empty: not set" };
            self.modal = Some(Modal::Input(Input { title, hint: hint.into(), value: field.value(&rules), purpose: InputPurpose::Auto { field, show } }));
            return;
        }
        let unset = if show.is_some() { "Same as profile" } else { "Not set" };
        let current = field.value(&rules);
        let rows = field
            .choices(&rules, unset)
            .into_iter()
            .map(|(v, l)| (PickEntry::Text(v.clone()), format!("{} {l}", if v == current { "●" } else { " " })))
            .collect();
        self.open_picker(title, PickKind::Auto { field, show }, " enter choose · esc cancel ", async move { Ok(rows) });
    }

    /// Store a rule of the profile or a show.
    pub(super) fn save_auto(&mut self, field: AutoField, show: Option<Show>, value: String) {
        let mut rules = self.auto_rules(&show);
        field.set(&mut rules, &value);
        let saved = prefs::save(&self.core, show.as_ref().map(|s| s.0.as_str()), show.as_ref().map(|s| s.1.as_str()), rules);
        if let Err(e) = saved {
            return self.error(err(e));
        }
        match show {
            Some(show) => self.open_show_prefs_for(show),
            None => self.refresh_local(&Source::Settings),
        }
    }

    pub(super) fn clear_auto(&mut self, show: Option<Show>) {
        let saved = prefs::save(&self.core, show.as_ref().map(|s| s.0.as_str()), show.as_ref().map(|s| s.1.as_str()), Rules::default());
        match saved {
            Ok(()) => {
                self.info(match &show {
                    Some((_, title)) => format!("Cleared the playback preferences of {title}"),
                    None => "Cleared the playback preferences".into(),
                });
                self.refresh_local(&Source::Settings);
            }
            Err(e) => self.error(err(e)),
        }
    }

    /// Menu: the automatic playback rules of the show the selection belongs to.
    pub(super) fn open_show_prefs(&mut self) {
        let Some(show) = self.selected().and_then(show_of) else {
            return self.info("Playback preferences work on a show, season or episode");
        };
        self.open_show_prefs_for(show);
    }

    /// The show's rules as a list of fields with their values, plus a way to clear them.
    pub(super) fn open_show_prefs_for(&mut self, show: Show) {
        let rules = self.auto_rules(&Some(show.clone()));
        let mut rows: Vec<(PickEntry, String)> = FIELDS
            .into_iter()
            .map(|f| (PickEntry::Text(f.id().into()), format!("{:<22}{}", f.title(), f.label(&rules, "same as profile"))))
            .collect();
        rows.push((PickEntry::Text("clear".into()), "  Clear this show's preferences…".into()));
        let title = format!("Playback preferences · {}", show.1);
        self.open_picker(title, PickKind::AutoShow { show }, " enter change · esc close ", async move { Ok(rows) });
    }

    pub(super) fn pick_auto_show(&mut self, show: Show, id: String) {
        match AutoField::from_id(&id) {
            Some(field) => self.change_auto(field, Some(show)),
            None => {
                self.modal = Some(Modal::Confirm(Confirm {
                    message: format!("Clear the playback preferences of {}? It follows your profile's again.", show.1),
                    action: ConfirmAction::ClearAuto { show: Some(show) },
                }))
            }
        }
    }

    // ---- smart downloads ------------------------------------------------------

    /// The policy of this connection for an item, if it has one.
    fn policy_of(&self, rating_key: &str) -> Option<PolicyView> {
        smart::overview(&self.core).policies.into_iter().find(|p| p.active && p.policy.rating_key == rating_key)
    }

    /// Menu: keep the next N unwatched items of a show, season or playlist downloaded.
    pub(super) fn open_keep(&mut self) {
        let Some(item) = self.selected().cloned() else { return };
        let Some(rating_key) = item.rating_key.clone().filter(|_| matches!(item.kind(), Kind::Show | Kind::Season | Kind::Playlist)) else {
            return self.info("Keeping downloads works on a show, season or playlist");
        };
        let existing = self.policy_of(&rating_key);
        let title = item.full_title();
        self.modal = Some(Modal::Input(Input {
            title: format!("Keep downloaded · {title}"),
            hint: format!("How many unwatched to keep (1–{})", smart::MAX_KEEP),
            value: existing.as_ref().map_or(3, |p| p.policy.keep).to_string(),
            purpose: InputPurpose::KeepNext { rating_key, title, delete_watched: existing.is_none_or(|p| p.policy.delete_watched) },
        }));
    }

    pub(super) fn save_keep(&mut self, rating_key: String, title: String, delete_watched: bool, value: &str) {
        let Ok(keep) = value.parse::<u32>() else { return self.error(format!("Enter a number from 1 to {}", smart::MAX_KEEP)) };
        let core = self.core.clone();
        self.spawn_op(Refresh::Nothing, async move {
            smart::set_policy(&core, &rating_key, keep, delete_watched).await.map_err(err)?;
            Ok(Some(format!("Keeping the next {keep} unwatched of {title} downloaded")))
        });
    }

    /// A row action on a policy of the Downloads screen.
    pub(super) fn policy_op(&mut self, op: RowOp, p: &PolicyView) {
        let id = p.policy.id.clone();
        let title = p.policy.title.clone();
        let patch = |app: &mut App, patch: Patch| {
            if let Err(e) = smart::update_policy(&app.core, &id, patch) {
                app.error(err(e));
            }
        };
        match op {
            RowOp::Change => {
                self.modal = Some(Modal::Input(Input {
                    title: format!("Keep downloaded · {title}"),
                    hint: format!("How many unwatched to keep (1–{})", smart::MAX_KEEP),
                    value: p.policy.keep.to_string(),
                    purpose: InputPurpose::KeepPolicy { id: p.policy.id.clone() },
                }));
            }
            RowOp::Toggle => patch(self, Patch { enabled: Some(!p.policy.enabled), ..Default::default() }),
            RowOp::KeepWatched => patch(self, Patch { delete_watched: Some(!p.policy.delete_watched), ..Default::default() }),
            RowOp::Remove | RowOp::RemoveFiles => {
                let delete_files = op == RowOp::RemoveFiles;
                let message = if delete_files {
                    format!("Stop keeping {title} downloaded and delete the {} download(s) it fetched?", p.downloads)
                } else {
                    format!("Stop keeping {title} downloaded? What it fetched stays until you delete it.")
                };
                self.modal = Some(Modal::Confirm(Confirm { message, action: ConfirmAction::StopKeeping { id: p.policy.id.clone(), delete_files } }));
            }
            _ => self.run_smart(),
        }
    }

    pub(super) fn save_keep_policy(&mut self, id: &str, value: &str) {
        let Ok(keep) = value.parse::<u32>() else { return self.error(format!("Enter a number from 1 to {}", smart::MAX_KEEP)) };
        if let Err(e) = smart::update_policy(&self.core, id, Patch { keep: Some(keep), ..Default::default() }) {
            self.error(err(e));
        }
    }

    pub(super) fn stop_keeping(&mut self, id: &str, delete_files: bool) {
        if let Err(e) = smart::remove_policy(&self.core, id, delete_files) {
            self.error(err(e));
        }
    }

    /// Fetch and remove downloads now to match the policies.
    pub(super) fn run_smart(&mut self) {
        let core = self.core.clone();
        self.spawn_op(Refresh::Nothing, async move {
            let r = smart::reconcile(&core).await.map_err(err)?;
            let bits: Vec<String> = [
                (r.added > 0).then(|| format!("{} queued", r.added)),
                (r.removed > 0).then(|| format!("{} removed", r.removed)),
                (r.over_budget > 0).then(|| format!("{} left out (storage budget)", r.over_budget)),
            ]
            .into_iter()
            .flatten()
            .chain(r.errors)
            .collect();
            Ok(Some(if bits.is_empty() { "Downloads already match the policies".to_string() } else { bits.join(" · ") }))
        });
    }

    pub(super) fn change_budget(&mut self) {
        let budget = smart::overview(&self.core).budget;
        self.modal = Some(Modal::Input(Input {
            title: "Storage budget".into(),
            hint: "GB all downloads together may use; empty for no limit".into(),
            value: budget.map(|b| format!("{:.1}", b as f64 / GIB)).unwrap_or_default(),
            purpose: InputPurpose::Budget,
        }));
    }

    pub(super) fn save_budget(&mut self, value: &str) {
        let text = value.trim().to_lowercase();
        let gb = text.trim_end_matches("gb").trim();
        let bytes = match gb {
            "" | "off" | "none" | "0" => None,
            gb => match gb.parse::<f64>() {
                Ok(n) if n > 0.0 => Some((n * GIB) as u64),
                _ => return self.error("Enter a size in GB, or leave it empty for no limit"),
            },
        };
        match smart::set_budget(&self.core, bytes) {
            Ok(()) => self.info(bytes.map_or("No storage budget".to_string(), |b| format!("Storage budget: {}", crate::format::bytes(b as i64)))),
            Err(e) => self.error(err(e)),
        }
    }

    /// Enter, `x` and the menu on a row of automatic playback or smart downloads; `true` when handled.
    pub(super) fn auto_row_op(&mut self, op: RowOp, act: &Act) -> bool {
        match (op, act) {
            (RowOp::Change, Act::Auto(f)) => self.change_auto(*f, None),
            (RowOp::Change, Act::AutoShow { rating_key, title }) => self.open_show_prefs_for((rating_key.clone(), title.clone())),
            (RowOp::Remove, Act::AutoShow { rating_key, title }) => {
                self.modal = Some(Modal::Confirm(Confirm {
                    message: format!("Clear the playback preferences of {title}? It follows your profile's again."),
                    action: ConfirmAction::ClearAuto { show: Some((rating_key.clone(), title.clone())) },
                }))
            }
            (RowOp::Run, Act::AutoClear) => {
                self.modal = Some(Modal::Confirm(Confirm {
                    message: "Clear every automatic playback rule of this profile?".into(),
                    action: ConfirmAction::ClearAuto { show: None },
                }))
            }
            (RowOp::Change, Act::Budget) => self.change_budget(),
            (RowOp::Change | RowOp::Toggle | RowOp::KeepWatched | RowOp::Remove | RowOp::RemoveFiles | RowOp::Run, Act::Policy(p)) => self.policy_op(op, p),
            (RowOp::Run, Act::SmartRun) => self.run_smart(),
            _ => return false,
        }
        true
    }
}
