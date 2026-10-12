use super::*;
use patina_core::ops::edit::{self, TagEdit};
use patina_core::session;
use patina_plex::HomeUser;

impl App {
    // ---- metadata editing ------------------------------------------------

    pub(super) fn editable(&mut self) -> Option<Item> {
        let item = self.selected_detail().cloned()?;
        if !self.owned() {
            self.info("Only the server owner can edit");
            return None;
        }
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

    /// Ask for a new value of `field`, starting from `value` (the current one by default).
    pub(super) fn edit_field(&mut self, m: EditModal, field: &'static str, value: Option<String>) {
        let label = m.fields.iter().find(|f| f.0 == field).map_or(field, |f| f.1);
        self.modal = Some(Modal::Input(Input {
            title: format!("{label} · {}", m.item.title()),
            hint: match field {
                "genre" | "label" => "Comma-separated".into(),
                "originallyAvailableAt" => "YYYY-MM-DD".into(),
                _ => label.into(),
            },
            value: value.unwrap_or_else(|| field_value(&m.item, field)),
            purpose: InputPurpose::EditField { edit: Box::new(m), field },
        }));
    }

    /// Save one field (only if changed), then return to the edit dialog.
    pub(super) fn submit_edit(&mut self, mut m: EditModal, field: &'static str, value: String) {
        if let Some(why) = invalid_value(field, &value) {
            self.error(why);
            return self.edit_field(m, field, Some(value));
        }
        let before = field_value(&m.item, field);
        let key = m.item.rating_key.clone().unwrap_or_default();
        let core = self.core.clone();
        if is_tag_field(field) {
            let (add, remove) = tag_diff(&split_tags(&before), &split_tags(&value));
            if !add.is_empty() || !remove.is_empty() {
                let tag = TagEdit { tag: field.into(), add, remove };
                self.spawn_op(Refresh::Item(key.clone()), async move {
                    edit::edit_metadata(&core, &key, &[], &[tag]).await.map_err(err)?;
                    Ok(Some("Saved".into()))
                });
                let tags: Vec<patina_plex::Tag> = split_tags(&value).into_iter().map(|t| patina_plex::Tag { tag: Some(t), ..Default::default() }).collect();
                if field == "genre" { m.item.genres = tags } else { m.item.labels = tags }
            }
        } else if value != before {
            self.patch(&key, &mut |i| set_field_value(i, field, &value));
            set_field_value(&mut m.item, field, &value);
            let v = value.clone();
            self.spawn_op(Refresh::Item(key.clone()), async move {
                edit::edit_metadata(&core, &key, &[(field.to_string(), v)], &[]).await.map_err(err)?;
                Ok(Some("Saved (field locked against agent updates)".into()))
            });
        }
        self.modal = Some(Modal::Edit(m));
    }

    pub(super) fn toggle_field_lock(&mut self, item: &Item, field: &'static str) {
        let key = item.rating_key.clone().unwrap_or_default();
        let lock = !item.is_field_locked(field);
        let (section, kind) = (item.library_section_id, item.type_number().unwrap_or(1));
        let done = if lock { "Field locked" } else { "Field unlocked" };
        self.server_op(Refresh::Item(key.clone()), done, move |s| async move { s.lock_fields(&key, section, kind, &[(field, lock)]).await });
    }

    pub(super) fn set_artwork(&mut self, rating_key: String, art: bool, url: String) {
        let done = if art { "Background updated" } else { "Poster updated" };
        self.server_op(Refresh::Item(rating_key.clone()), done, move |s| async move {
            if art { s.set_art(&rating_key, &url).await } else { s.set_poster(&rating_key, &url).await }
        });
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

    /// Posters, or backgrounds when `art`, to choose from (or an image URL).
    pub(super) fn open_artwork(&mut self, art: bool) {
        let Some(item) = self.editable() else { return };
        let key = item.rating_key.clone().unwrap_or_default();
        let server = self.server();
        let k = key.clone();
        let what = if art { "Background" } else { "Poster" };
        self.open_picker(format!("{what} · {}", item.title()), PickKind::Artwork { rating_key: key, art }, " enter use · v view · esc close ", async move {
            let server = server?;
            let list = if art { server.arts(&k).await } else { server.posters(&k).await }.map_err(err)?;
            let mut rows = vec![(PickEntry::ArtworkFromUrl, "+ Use an image URL…".to_string())];
            for (i, p) in list.into_iter().enumerate() {
                let selected = p.extra.get("selected").is_some_and(|v| v.as_bool().unwrap_or(v.as_i64() == Some(1)));
                let provider = p.extra.get("provider").and_then(|v| v.as_str()).unwrap_or("uploaded").to_string();
                let label = if selected { format!("● Current · {provider} #{}", i + 1) } else { format!("  {provider} #{}", i + 1) };
                rows.push((PickEntry::Artwork(Box::new(Item::from(p))), label));
            }
            Ok(rows)
        });
    }

    /// A collection's display mode (`sort` false) or sort order.
    pub(super) fn open_collection_setting(&mut self, sort: bool) {
        let Some(item) = self.editable().filter(|i| i.kind() == Kind::Collection) else { return };
        let rating_key = item.rating_key.clone().unwrap_or_default();
        let (title, current, choices, kind) = if sort {
            ("Sort", item.collection_sort().unwrap_or(0), COLLECTION_SORTS, PickKind::CollectionSort { rating_key })
        } else {
            ("Show in library", item.collection_mode.unwrap_or(-1) as i32, COLLECTION_MODES, PickKind::CollectionMode { rating_key })
        };
        let rows: Vec<(PickEntry, String)> =
            choices.iter().map(|(v, l)| (PickEntry::Value(*v), format!("{} {l}", if *v == current { "●" } else { " " }))).collect();
        self.open_picker(format!("{title} · {}", item.title()), kind, " enter choose · esc close ", async move { Ok(rows) });
    }

    /// Match candidates: the agent's own guesses, or a search by `query` (title
    /// and year). As on the GUI, the dialog also searches again or unmatches.
    pub(super) fn search_matches(&mut self, rating_key: String, title: String, year: Option<i64>, query: Option<String>) {
        let server = self.server();
        let k = rating_key.clone();
        let heading = format!("Match · {}", query.as_deref().unwrap_or(&title));
        let kind = PickKind::Matches { rating_key, title, year };
        self.open_picker(heading, kind, " enter apply · esc close ", async move {
            let q = query.as_deref().filter(|q| !q.is_empty());
            let results = server?.matches(&k, q, q.and(year), None).await.map_err(err)?;
            let mut rows = vec![(PickEntry::MatchSearch, "+ Search by title…".to_string())];
            rows.extend(results.into_iter().map(|m| {
                let mut label = m.name.clone().unwrap_or_default();
                if let Some(y) = m.year {
                    label.push_str(&format!(" ({y})"));
                }
                if let Some(sc) = m.score {
                    label.push_str(&format!(" · {sc}%"));
                }
                if m.matched {
                    label.push_str(" · current");
                }
                (PickEntry::Match(Box::new(m)), label)
            }));
            rows.push((PickEntry::Unmatch, "  Unmatch…".to_string()));
            Ok(rows)
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
            (PickKind::Sort, PickEntry::Sort(d)) => self.choose_sort(*d),
            (PickKind::Letters, PickEntry::Letter(l, at)) => self.jump_to_letter(&l, at),
            (PickKind::Artwork { rating_key, art }, PickEntry::ArtworkFromUrl) => {
                self.modal = Some(Modal::Input(Input {
                    title: format!("{} from URL", if art { "Background" } else { "Poster" }),
                    hint: "Image URL (https://…)".into(),
                    value: String::new(),
                    purpose: InputPurpose::ArtworkUrl { rating_key, art },
                }));
            }
            (PickKind::Artwork { rating_key, art }, PickEntry::Artwork(a)) => {
                let Some(url) = a.key.clone().or_else(|| a.rating_key.clone()) else { return };
                self.set_artwork(rating_key, art, url);
            }
            (PickKind::CollectionMode { rating_key }, PickEntry::Value(mode)) => {
                self.patch(&rating_key, &mut |i| i.collection_mode = Some(mode as i64));
                self.server_op(Refresh::Item(rating_key.clone()), "Collection updated", move |s| async move { s.set_collection_mode(&rating_key, mode).await });
            }
            (PickKind::CollectionSort { rating_key }, PickEntry::Value(sort)) => {
                let path = format!("/library/collections/{rating_key}/children");
                self.patch(&rating_key, &mut |i| {
                    i.meta.extra.insert("collectionSort".into(), sort.to_string().into());
                });
                self.server_op(Refresh::Paths(vec![path]), "Collection updated", move |s| async move { s.set_collection_sort(&rating_key, sort).await });
            }
            (PickKind::Matches { rating_key, .. }, PickEntry::Match(m)) => {
                let Some(guid) = m.guid.clone() else { return };
                let done = format!("Matched to {}; refreshing metadata", m.name.as_deref().unwrap_or("the result"));
                self.server_op(Refresh::Item(rating_key.clone()), done, move |s| async move { s.apply_match(&rating_key, &guid, m.name.as_deref()).await });
            }
            (PickKind::Matches { rating_key, title, year }, PickEntry::MatchSearch) => {
                self.modal = Some(Modal::Input(Input {
                    title: format!("Fix match · {title}"),
                    hint: "Search title".into(),
                    value: title.clone(),
                    purpose: InputPurpose::MatchQuery { rating_key, title, year },
                }));
            }
            (PickKind::Matches { rating_key, title, .. }, PickEntry::Unmatch) => {
                self.modal = Some(Modal::Confirm(Confirm {
                    message: format!("Unmatch {title}? Its metadata and artwork are cleared until it's matched again."),
                    action: ConfirmAction::Unmatch { rating_key },
                }));
            }
            (PickKind::Subtitles { rating_key }, PickEntry::Subtitle(st)) => {
                let Some(key) = st.key.clone() else { return };
                self.server_op(Refresh::Item(rating_key.clone()), "Subtitle downloaded", move |s| async move { s.download_subtitle(&rating_key, &key).await });
            }
            (PickKind::Setting(key), PickEntry::Text(v)) => self.save_setting(key, Some(v)),
            (PickKind::AutoShow { show }, PickEntry::Text(id)) => self.pick_auto_show(show, id),
            (PickKind::Auto { field, show }, PickEntry::Text(v)) => self.save_auto(field, show, v),
            (PickKind::Pref { id, section }, PickEntry::Text(v)) => self.save_pref(id, section, v),
            (PickKind::Servers, PickEntry::Text(id)) => self.switch_server(id),
            (PickKind::LibraryKind, PickEntry::Text(kind)) => {
                let form = patina_core::ops::server::LibraryForm {
                    name: String::new(),
                    kind,
                    agent: String::new(),
                    scanner: String::new(),
                    language: String::new(),
                    locations: Vec::new(),
                };
                self.library_form_step(form, "name");
            }
            (PickKind::LibraryAgent(mut form), PickEntry::Text(agent)) => {
                form.agent = agent;
                self.library_form_step(*form, "scanner");
            }
            (PickKind::LibraryScanner(mut form), PickEntry::Text(scanner)) => {
                form.scanner = scanner;
                self.library_form_step(*form, "language");
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
