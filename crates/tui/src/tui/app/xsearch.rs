//! Search across every server of the account, from the search overlay (`ctrl-a`).
//!
//! Results arrive as servers answer: one entry per title, badged with the servers that
//! have it. Enter on a title opens the copy chooser, where a copy is played (on its own
//! server) or opened for browsing.

use crossterm::event::{KeyCode, KeyEvent};
use patina_core::ops::xsearch::{Group, Results, ServerState, ServerStatus};
use patina_core::servers;

use super::*;

/// Whether the search overlay searches every server, and what it found.
#[derive(Default)]
pub struct XSearch {
    pub on: bool,
    /// One per displayed result, in order.
    pub groups: Vec<Group>,
    pub servers: Vec<ServerStatus>,
    pub done: bool,
    /// Ignores answers of a search that a newer one replaced.
    generation: u64,
}

/// "A · B 1080p" — the servers that have a title, with the best quality of each copy.
pub fn badges(group: &Group) -> String {
    group
        .copies
        .iter()
        .map(|c| {
            let quality = c.versions.iter().filter_map(|v| v.resolution.clone()).max_by_key(|r| rank(r));
            match quality {
                Some(q) => format!("{} {q}", c.server_name),
                None => c.server_name.clone(),
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

fn rank(resolution: &str) -> u32 {
    match resolution {
        "4K" => 2160,
        "SD" => 480,
        r => r.trim_end_matches('p').parse().unwrap_or(0),
    }
}

/// "Alpha ✓ · Beta … · Gamma ✗" for the overlay's footer.
pub fn status_line(servers: &[ServerStatus]) -> String {
    servers
        .iter()
        .map(|s| {
            let mark = match s.state {
                ServerState::Pending => "…",
                ServerState::Done => "✓",
                ServerState::Failed => "✗",
                ServerState::TimedOut => "⧖",
                ServerState::Offline => "off",
            };
            format!("{} {mark}", s.name)
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

impl App {
    /// `ctrl-a` in the search overlay.
    pub(super) fn toggle_all_servers(&mut self) {
        if self.search.is_none() {
            return;
        }
        if self.session.as_ref().is_some_and(|s| s.servers.is_empty()) {
            self.info("Searching every server needs a plex.tv sign-in");
            return;
        }
        self.xs.on = !self.xs.on;
        self.xs.groups.clear();
        self.xs.servers.clear();
        self.xs.generation += 1;
        if let Some(s) = &mut self.search {
            s.set_groups(Vec::new());
        }
        self.info(if self.xs.on { "Searching all servers" } else { "Searching this server" });
        self.search_changed();
    }

    /// Search every server for `query`; the results grow as servers answer.
    pub(super) fn search_all_servers(&mut self, query: String) {
        self.xs.generation += 1;
        let (core, tx, generation) = (self.core.clone(), self.tx.clone(), self.xs.generation);
        tokio::spawn(async move {
            let updates = tx.clone();
            let q = query.clone();
            let done = patina_core::ops::xsearch::search_all(&core, &query, move |r| {
                let _ = updates.send(Msg::XSearch { generation, query: q.clone(), results: Box::new(r.clone()) });
            })
            .await;
            let _ = tx.send(Msg::XSearch { generation, query, results: Box::new(done) });
        });
    }

    pub(super) fn on_xsearch(&mut self, generation: u64, query: String, results: Results) {
        if generation != self.xs.generation || !self.xs.on {
            return;
        }
        let Some(s) = &mut self.search else { return };
        if s.input.trim() != query {
            return;
        }
        let cursor_key = s.results.get(s.cursor).map(|i| (i.server_id().map(str::to_string), i.rating_key.clone()));
        let items: Vec<Item> = results.groups.iter().filter_map(|g| g.copies.first()).map(|c| Item::from(c.item.clone())).collect();
        let mut group = Item::link("Across servers", "hub", Source::Static, items);
        group.child_count = Some(results.groups.len() as i64);
        s.set_groups(if results.groups.is_empty() { Vec::new() } else { vec![group] });
        // Results move as servers answer; keep the cursor on what it was on.
        s.cursor = cursor_key
            .and_then(|k| s.results.iter().position(|i| (i.server_id().map(str::to_string), i.rating_key.clone()) == k))
            .unwrap_or(s.cursor)
            .min(s.results.len().saturating_sub(1));
        s.pending = !results.done;
        s.error = None;
        self.xs.groups = results.groups;
        self.xs.servers = results.servers;
        self.xs.done = results.done;
        self.after_selection_change();
    }

    /// The titles' copies under the cursor when searching all servers.
    fn copies_under_cursor(&self) -> Option<Group> {
        let s = self.search.as_ref().filter(|_| self.xs.on)?;
        self.xs.groups.get(s.cursor).cloned()
    }

    /// Enter in an all-servers search: choose which copy to use.
    pub(super) fn search_all_accept(&mut self) -> bool {
        let Some(group) = self.copies_under_cursor() else { return false };
        // A title on one server and one version needs no choice.
        if group.copies.len() == 1 && group.copies[0].versions.len() <= 1 && group.copies[0].current {
            return false;
        }
        self.modal = Some(Modal::Copies(CopyPicker { group, cursor: 0 }));
        true
    }

    pub(super) fn copies_key(&mut self, mut p: CopyPicker, key: KeyEvent) -> Option<Modal> {
        let n = p.group.copies.len();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                p.cursor = (p.cursor + 1).min(n.saturating_sub(1));
                Some(Modal::Copies(p))
            }
            KeyCode::Char('k') | KeyCode::Up => {
                p.cursor = p.cursor.saturating_sub(1);
                Some(Modal::Copies(p))
            }
            KeyCode::Enter | KeyCode::Char('p') => {
                if let Some(copy) = p.group.copies.get(p.cursor).cloned() {
                    self.search = None;
                    let from_start = key.code == KeyCode::Char('p');
                    let mut req = patina_core::player::PlayRequest { item: copy.item, resume: !from_start, ..Default::default() };
                    req.shuffle = false;
                    self.start_play(req, Vec::new());
                }
                None
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Char('o') => {
                if let Some(copy) = p.group.copies.get(p.cursor).cloned() {
                    self.open_copy(copy.item, &copy.server_name);
                }
                None
            }
            KeyCode::Esc | KeyCode::Char('q') => None,
            _ => Some(Modal::Copies(p)),
        }
    }

    /// Browse a copy: a column holding it, opened into if it is a container.
    fn open_copy(&mut self, item: Metadata, server_name: &str) {
        let title = format!("{} · {server_name}", item.title());
        self.search = None;
        self.columns.truncate(1);
        let item = Item::from(item);
        let identity = Column::identity(&item);
        let container = item.is_container();
        self.open_source(title, Source::Static, vec![item]);
        if let Some(col) = self.columns.last_mut() {
            col.select_identity(&identity);
        }
        if container {
            self.drill();
        }
        self.after_selection_change();
    }

    /// Keys that would change the wrong server's item are refused for items found on another
    /// server; playing, queueing, browsing and marking watched work on their own server.
    pub(super) fn foreign_blocked(&mut self, key: &KeyEvent) -> bool {
        let Some(item) = self.selected().filter(|i| i.row.is_none()) else { return false };
        let Some(id) = item.server_id().map(str::to_string) else { return false };
        let blocked = matches!(
            key.code,
            KeyCode::Char('e' | '*' | ' ' | 'x' | '+' | 'C' | 'T' | 'c' | 'm' | '.' | 'O' | 'y' | 'D' | 'J' | 'K' | 'b' | 'B' | 'S')
        ) || key.code == KeyCode::Delete;
        if blocked {
            let name = servers::name_of(&self.core, Some(&id));
            self.info(format!("That item is on {name}; from here you can play it, browse it and mark it watched"));
        }
        blocked
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use patina_core::ops::xsearch::{ServerCopy, Version};

    fn copy(name: &str, resolution: Option<&str>) -> ServerCopy {
        ServerCopy {
            server_id: name.to_lowercase(),
            server_name: name.into(),
            current: false,
            library: None,
            item: Metadata::default(),
            versions: resolution.map(|r| Version { label: r.into(), resolution: Some(r.into()), video_codec: None, container: None, bitrate: None, size: None, hdr: None }).into_iter().collect(),
        }
    }

    #[test]
    fn badges_name_each_server_with_its_best_quality() {
        let mut two = copy("Beta", Some("720p"));
        two.versions.push(Version { label: "4K".into(), resolution: Some("4K".into()), video_codec: None, container: None, bitrate: None, size: None, hdr: None });
        let group = Group { key: "k".into(), title: "T".into(), year: None, kind: "movie".into(), subtitle: None, copies: vec![copy("Alpha", None), two, copy("Gamma", Some("1080p"))] };
        assert_eq!(badges(&group), "Alpha · Beta 4K · Gamma 1080p");
    }

    #[test]
    fn the_status_line_shows_each_server_state() {
        let status = |name: &str, state| ServerStatus { id: name.into(), name: name.into(), current: false, state, count: 0, error: None };
        let line = status_line(&[status("A", ServerState::Done), status("B", ServerState::Pending), status("C", ServerState::TimedOut), status("D", ServerState::Failed)]);
        assert_eq!(line, "A ✓ · B … · C ⧖ · D ✗");
    }
}
