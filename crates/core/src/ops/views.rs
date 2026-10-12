//! Saved library views: named queries (type, sort, filters) of one library,
//! kept per account and server in the shared `config.toml` under
//! `ui.saved_views`. The desktop app reads and writes the same entries, so a
//! view saved in either front end is there in both.
//!
//! The entries are stored as the JSON the desktop app writes
//! (`web/src/lib/types.ts`, `SavedView`). Fields this code doesn't know are
//! kept as they are when an entry is rewritten, and entries that don't parse
//! are left alone.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::state::{AppState, SessionView};

/// A value filter ("Genre: Drama").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Picked {
    pub filter: String,
    pub title: String,
    pub value: String,
    pub label: String,
}

/// What only the terminal client adds to a view; the desktop app ignores it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TuiExtras {
    pub hide_watched: bool,
    /// Text typed into the column's `/` filter.
    pub filter: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SavedView {
    pub id: String,
    pub name: String,
    /// `account:server`, see [`scope`].
    pub scope: String,
    /// The library's section id.
    pub library: String,
    /// Plex media type listed; 0 for the library's default.
    pub media_type: i64,
    /// Plex sort key, e.g. `titleSort`; `desc` reverses it.
    pub sort: String,
    pub desc: bool,
    pub unwatched: bool,
    /// Boolean filters that are on, by key.
    pub toggles: Vec<String>,
    pub picked: Vec<Picked>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tui: Option<TuiExtras>,
    /// Fields written by other versions or front ends.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl SavedView {
    /// A new view of `library` for `scope`, with a fresh id.
    pub fn new(scope: &str, library: &str, name: &str) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.trim().to_string(),
            scope: scope.into(),
            library: library.into(),
            sort: "titleSort".into(),
            ..Default::default()
        }
    }
}

/// The scope views are saved under: the same `accountId:serverId` the desktop app uses.
pub fn scope_of(session: &SessionView) -> String {
    format!("{}:{}", session.account_id, session.server_id)
}

/// The current account and server's scope; `None` when not connected.
pub fn scope(app: &AppState) -> Option<String> {
    app.session().map(|s| scope_of(&SessionView::from(&s)))
}

/// The views of `library` on the connected server, in the order saved.
pub fn list(app: &AppState, library: &str) -> Vec<SavedView> {
    let Some(scope) = scope(app) else { return Vec::new() };
    matching(&app.settings().ui.saved_views, &scope, library)
}

/// Save `view`. A view of the same library and name (in the same scope) is replaced.
pub fn save(app: &AppState, view: SavedView) -> Result<()> {
    anyhow::ensure!(!view.name.trim().is_empty(), "A saved view needs a name.");
    let value = serde_json::to_value(&view)?;
    app.try_update_settings(|s| {
        upsert(&mut s.ui.saved_views, view, value);
        Ok(())
    })
}

/// Delete the view with this id.
pub fn remove(app: &AppState, id: &str) -> Result<()> {
    app.try_update_settings(|s| {
        forget(&mut s.ui.saved_views, id);
        Ok(())
    })
}

fn parse(v: &Value) -> Option<SavedView> {
    serde_json::from_value(v.clone()).ok().filter(|v: &SavedView| !v.id.is_empty())
}

pub(crate) fn matching(stored: &[Value], scope: &str, library: &str) -> Vec<SavedView> {
    stored.iter().filter_map(parse).filter(|v| v.scope == scope && v.library == library).collect()
}

pub(crate) fn upsert(stored: &mut Vec<Value>, view: SavedView, value: Value) {
    let same = |v: &Value| parse(v).is_some_and(|old| old.scope == view.scope && old.library == view.library && old.name.eq_ignore_ascii_case(&view.name));
    match stored.iter().position(same) {
        Some(at) => {
            // Keep the id so anything pointing at the view still does.
            let mut value = value;
            if let (Some(old), Some(obj)) = (stored[at].get("id").cloned(), value.as_object_mut()) {
                obj.insert("id".into(), old);
            }
            stored[at] = value;
        }
        None => stored.push(value),
    }
}

pub(crate) fn forget(stored: &mut Vec<Value>, id: &str) {
    stored.retain(|v| v.get("id").and_then(Value::as_str) != Some(id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// An entry as the desktop app writes it.
    fn gui_entry(id: &str, name: &str, library: &str) -> Value {
        json!({
            "id": id, "name": name, "scope": "alice:home", "library": library,
            "mediaType": 0, "sort": "addedAt", "desc": true, "unwatched": true,
            "toggles": ["hdr"], "picked": [{"filter": "genre", "title": "Genre", "value": "10", "label": "Drama"}],
            "somethingNew": {"a": 1}
        })
    }

    #[test]
    fn reads_what_the_desktop_app_wrote() {
        let stored = vec![gui_entry("v1", "Recent drama", "1"), gui_entry("v2", "Elsewhere", "2"), json!("garbage"), json!({"name": "no id"})];
        let views = matching(&stored, "alice:home", "1");
        assert_eq!(views.len(), 1);
        let v = &views[0];
        assert_eq!((v.id.as_str(), v.name.as_str(), v.sort.as_str()), ("v1", "Recent drama", "addedAt"));
        assert!(v.desc && v.unwatched && v.tui.is_none());
        assert_eq!(v.toggles, ["hdr"]);
        assert_eq!(v.picked, [Picked { filter: "genre".into(), title: "Genre".into(), value: "10".into(), label: "Drama".into() }]);
        // Another account or server doesn't see it.
        assert!(matching(&stored, "bob:home", "1").is_empty());
        // Unknown fields survive a rewrite.
        let back = serde_json::to_value(v).unwrap();
        assert_eq!(back["somethingNew"], json!({"a": 1}));
        assert_eq!(back["mediaType"], 0);
        assert!(back.get("tui").is_none(), "the desktop app's own entries gain no terminal fields");
    }

    #[test]
    fn terminal_views_keep_the_desktop_shape() {
        let mut view = SavedView::new("alice:home", "1", "  Unwatched HDR ");
        view.unwatched = true;
        view.toggles = vec!["hdr".into()];
        view.tui = Some(TuiExtras { hide_watched: true, filter: "dune".into() });
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["name"], "Unwatched HDR");
        for key in ["id", "name", "scope", "library", "mediaType", "sort", "desc", "unwatched", "toggles", "picked"] {
            assert!(json.get(key).is_some(), "the desktop app reads {key}");
        }
        assert_eq!(json["tui"], json!({"hideWatched": true, "filter": "dune"}));
        assert_eq!(parse(&json).unwrap(), view);
        // And it round-trips through the config file.
        let mut settings = crate::config::Settings::default();
        settings.ui.saved_views.push(json);
        let text = toml::to_string_pretty(&settings).unwrap();
        let again: crate::config::Settings = toml::from_str(&text).unwrap();
        assert_eq!(matching(&again.ui.saved_views, "alice:home", "1"), vec![view]);
    }

    #[test]
    fn saving_the_same_name_replaces_and_removing_deletes() {
        let mut stored = vec![gui_entry("v1", "Recent drama", "1")];
        let mut view = SavedView::new("alice:home", "1", "recent DRAMA");
        view.sort = "titleSort".into();
        let value = serde_json::to_value(&view).unwrap();
        upsert(&mut stored, view, value);
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0]["id"], "v1", "the id of the replaced view is kept");
        assert_eq!(stored[0]["sort"], "titleSort");
        let other = SavedView::new("alice:home", "1", "Another");
        let value = serde_json::to_value(&other).unwrap();
        upsert(&mut stored, other.clone(), value);
        assert_eq!(stored.len(), 2);
        // A view of the same name in another library is a different view.
        let elsewhere = SavedView::new("alice:home", "2", "Another");
        let value = serde_json::to_value(&elsewhere).unwrap();
        upsert(&mut stored, elsewhere, value);
        assert_eq!(stored.len(), 3);
        forget(&mut stored, &other.id);
        assert_eq!(stored.len(), 2);
        forget(&mut stored, "missing");
        assert_eq!(stored.len(), 2);
    }

    #[test]
    fn scope_matches_the_desktop_apps() {
        let view = SessionView {
            account_id: "acct".into(),
            server_name: "S".into(),
            server_id: "srv".into(),
            machine_identifier: None,
            base_url: String::new(),
            owned: true,
            has_tv: false,
            user: None,
            servers: Vec::new(),
        };
        assert_eq!(scope_of(&view), "acct:srv");
    }
}
