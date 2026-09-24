use super::*;
use patina_core::ops::edit::{self, TagEdit};
use patina_core::session;
use patina_plex::HomeUser;

impl App {
    // ---- metadata editing ------------------------------------------------

    pub(super) fn editable(&mut self) -> Option<Item> {
        let item = self.selected_detail().cloned()?;
        if item.rating_key.is_none() || item.type_number().is_none() || !detail_kind(&item) {
            self.info("Select a library item");
            return None;
        }
        Some(item)
    }

    pub(super) fn open_edit(&mut self) {
        let Some(item) = self.editable() else { return };
        let key = item.rating_key.clone().unwrap_or_default();
        // Listings lack labels and lock flags; edit from full metadata.
        let (server, tx) = (self.server(), self.tx.clone());
        tokio::spawn(async move {
            let r = async {
                let m = server?.metadata(&key, MetadataOptions::default()).await.map_err(err)?;
                m.map(|m| Box::new(Item::from(m))).ok_or_else(|| "Item not found".to_string())
            }
            .await;
            let _ = tx.send(Msg::EditItem(r));
        });
    }

    pub(super) fn submit_edit(&mut self, item: Item, field: &'static str, value: String) {
        let key = item.rating_key.clone().unwrap_or_default();
        if field != "labels" {
            self.patch(&key, &mut |i| set_field_value(i, field, &value));
        }
        let core = self.core.clone();
        self.spawn_op(Refresh::Item(key.clone()), async move {
            if field == "labels" {
                let labels: Vec<String> = value.split(',').map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
                let current: Vec<String> = item.labels.iter().map(|t| t.name().to_string()).filter(|l| !l.is_empty()).collect();
                let tag = TagEdit {
                    tag: "label".into(),
                    add: labels.iter().filter(|l| !current.contains(l)).cloned().collect(),
                    remove: current.iter().filter(|l| !labels.contains(l)).cloned().collect(),
                };
                edit::edit_metadata(&core, &key, &[], &[tag]).await
            } else {
                edit::edit_metadata(&core, &key, &[(field.to_string(), value)], &[]).await
            }
            .map_err(err)?;
            Ok(Some("Saved (field locked against agent updates)".into()))
        });
    }

    pub(super) fn toggle_field_lock(&mut self, item: &Item, field: &'static str) {
        let key = item.rating_key.clone().unwrap_or_default();
        let lock = !item.is_field_locked(field);
        let (section, kind) = (item.library_section_id, item.type_number().unwrap_or(1));
        let done = if lock { "Field locked" } else { "Field unlocked" };
        self.server_op(Refresh::Item(key.clone()), done, move |s| async move { s.lock_fields(&key, section, kind, &[(field, lock)]).await });
    }

    /// Open a picker whose rows load in the background.
    pub(super) fn open_picker<F>(&mut self, title: String, kind: PickKind, hint: &'static str, load: F)
    where
        F: std::future::Future<Output = Result<Vec<(PickEntry, String)>, String>> + Send + 'static,
    {
        self.modal = Some(Modal::Pick(Picker { title, kind, rows: Vec::new(), cursor: 0, loading: true, error: None, hint }));
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(Msg::PickRows(load.await));
        });
    }

    pub(super) fn open_posters(&mut self) {
        let Some(item) = self.editable() else { return };
        let key = item.rating_key.clone().unwrap_or_default();
        let server = self.server();
        let k = key.clone();
        self.open_picker(format!("Poster · {}", item.title()), PickKind::Posters { rating_key: key }, " enter use · v view · esc close ", async move {
            let posters = server?.posters(&k).await.map_err(err)?;
            let mut rows = vec![(PickEntry::PosterFromUrl, "+ Use an image URL…".to_string())];
            for (i, p) in posters.into_iter().enumerate() {
                let selected = p.extra.get("selected").is_some_and(|v| v.as_bool().unwrap_or(v.as_i64() == Some(1)));
                let provider = p.extra.get("provider").and_then(|v| v.as_str()).unwrap_or("uploaded").to_string();
                let label = format!("{} {provider} #{}", if selected { "●" } else { " " }, i + 1);
                rows.push((PickEntry::Poster(Box::new(Item::from(p))), label));
            }
            Ok(rows)
        });
    }

    pub(super) fn search_matches(&mut self, rating_key: String, query: String, year: Option<i64>) {
        let server = self.server();
        let k = rating_key.clone();
        self.open_picker(format!("Match · {query}"), PickKind::Matches { rating_key }, " enter apply · esc close ", async move {
            let title = Some(query.as_str()).filter(|q| !q.is_empty());
            let results = server?.matches(&k, title, year, None).await.map_err(err)?;
            Ok(results
                .into_iter()
                .map(|m| {
                    let mut label = m.name.clone().unwrap_or_default();
                    if let Some(y) = m.year {
                        label.push_str(&format!(" ({y})"));
                    }
                    if let Some(sc) = m.score {
                        label.push_str(&format!(" · {sc}%"));
                    }
                    (PickEntry::Match(Box::new(m)), label)
                })
                .collect())
        });
    }

    pub(super) fn search_subtitles(&mut self, rating_key: String, language: String) {
        let server = self.server();
        let k = rating_key.clone();
        self.open_picker(format!("Subtitles · {language}"), PickKind::Subtitles { rating_key }, " enter download · esc close ", async move {
            let results = server?.search_subtitles(&k, &language, false, false).await.map_err(err)?;
            Ok(results
                .into_iter()
                .map(|st| {
                    let mut label = st.title.clone().or_else(|| st.display_title.clone()).unwrap_or_else(|| st.label());
                    if st.hearing_impaired {
                        label.push_str(" · SDH");
                    }
                    if let Some(p) = &st.provider_title {
                        label.push_str(&format!(" · {p}"));
                    }
                    if let Some(sc) = st.score {
                        label.push_str(&format!(" · score {sc}"));
                    }
                    (PickEntry::Subtitle(Box::new(st)), label)
                })
                .collect())
        });
    }

    pub(super) fn open_users(&mut self) {
        let tv = match self.core.plex_tv() {
            Ok(tv) => tv,
            Err(e) => return self.error(err(e)),
        };
        let current = self.username().unwrap_or_default().to_string();
        self.open_picker("Switch user".into(), PickKind::Users, " enter switch · esc close ", async move {
            let users = tv.home_users().await.map_err(err)?;
            Ok(users
                .into_iter()
                .map(|u| {
                    let mut label = format!("{} {}", if u.name() == current { "●" } else { " " }, u.name());
                    if u.admin {
                        label.push_str(" · admin");
                    } else if u.restricted {
                        label.push_str(" · managed");
                    }
                    if u.protected {
                        label.push_str(" · PIN");
                    }
                    (PickEntry::User(Box::new(u)), label)
                })
                .collect())
        });
    }

    /// Switch Plex Home user; the session reconnects and the browser is rebuilt.
    pub(super) fn switch_user(&mut self, user: HomeUser, pin: Option<String>) {
        let Some(uuid) = user.uuid.clone() else { return };
        self.info(format!("Switching to {}…", user.name()));
        let core = self.core.clone();
        self.spawn_op(Refresh::Nothing, async move {
            session::switch_user(&core, &uuid, pin.as_deref()).await.map_err(err)?;
            Ok(None)
        });
    }

    pub(super) fn pick_selected(&mut self, p: Picker) {
        let Some((entry, _)) = p.rows.into_iter().nth(p.cursor) else { return };
        match (p.kind, entry) {
            (PickKind::Posters { rating_key }, PickEntry::PosterFromUrl) => {
                self.modal = Some(Modal::Input(Input {
                    title: "Poster from URL".into(),
                    hint: "Image URL (https://…)".into(),
                    value: String::new(),
                    purpose: InputPurpose::PosterUrl { rating_key },
                }));
            }
            (PickKind::Posters { rating_key }, PickEntry::Poster(poster)) => {
                let Some(url) = poster.rating_key.clone().or_else(|| poster.key.clone()) else { return };
                self.server_op(Refresh::Item(rating_key.clone()), "Poster changed", move |s| async move { s.set_poster(&rating_key, &url).await });
            }
            (PickKind::Matches { rating_key }, PickEntry::Match(m)) => {
                let Some(guid) = m.guid.clone() else { return };
                let done = format!("Matched to {}; refreshing metadata", m.name.as_deref().unwrap_or("the result"));
                self.server_op(Refresh::Item(rating_key.clone()), done, move |s| async move { s.apply_match(&rating_key, &guid, m.name.as_deref()).await });
            }
            (PickKind::Subtitles { rating_key }, PickEntry::Subtitle(st)) => {
                let Some(key) = st.key.clone() else { return };
                self.server_op(Refresh::Item(rating_key.clone()), "Subtitle downloaded", move |s| async move { s.download_subtitle(&rating_key, &key).await });
            }
            (PickKind::Users, PickEntry::User(u)) => {
                if Some(u.name()) == self.username() {
                    self.info(format!("Already signed in as {}", u.name()));
                } else if u.protected {
                    self.modal = Some(Modal::Input(Input {
                        title: format!("PIN for {}", u.name()),
                        hint: "4-digit PIN".into(),
                        value: String::new(),
                        purpose: InputPurpose::UserPin { user: u },
                    }));
                } else {
                    self.switch_user(*u, None);
                }
            }
            _ => {}
        }
    }
}
