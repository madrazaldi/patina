//! Library browsing beyond plain listings: filters, folders, related hubs.

use anyhow::Result;
use patina_plex::{Directory, Hub, LibraryMetaType, MediaType, Metadata, PlexServer};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterView {
    pub filter: String,
    pub title: String,
    /// `string`, `integer`, `tag` or `boolean`.
    pub kind: String,
}

/// Filters for a section and media type (genre, year, resolution, unwatched, ...).
pub async fn section_filters(app: &AppState, id: &str, kind: Option<u32>) -> Result<Vec<FilterView>> {
    let server = app.server()?;
    let view = |d: &Directory| {
        Some(FilterView {
            filter: d.filter.clone()?,
            title: d.title.clone().unwrap_or_default(),
            kind: d.filter_type.clone().unwrap_or_else(|| "string".into()),
        })
    };
    if let Some(meta) = server.section_meta(id).await? {
        let of_kind = |t: &&LibraryMetaType| t.kind.as_deref().and_then(MediaType::from_name).map(MediaType::id) == kind;
        if let Some(t) = meta.types.iter().find(of_kind).or(meta.types.first()) {
            return Ok(t.filters.iter().filter_map(view).collect());
        }
    }
    Ok(server.section_filters(id).await?.iter().filter_map(view).collect())
}

#[derive(Serialize)]
pub struct FolderView {
    pub title: Option<String>,
    pub directories: Vec<Directory>,
    pub items: Vec<Metadata>,
}

/// A section's folder tree: the root, or the folder at `key`.
pub async fn section_folders(app: &AppState, id: &str, key: Option<&str>) -> Result<FolderView> {
    let mc = app.server()?.section_folders(id, key).await?;
    Ok(FolderView { title: mc.title2.or(mc.title1), directories: mc.directory, items: mc.metadata })
}

/// Extra hubs for a detail page, plus sonically similar music.
pub async fn item_hubs(app: &AppState, rating_key: &str, kind: &str) -> Result<Vec<Hub>> {
    item_hubs_on(&app.server()?, rating_key, kind).await
}

/// [`item_hubs`] on any server (an item found on another server of the account).
pub async fn item_hubs_on(server: &PlexServer, rating_key: &str, kind: &str) -> Result<Vec<Hub>> {
    let mut hubs = server.metadata_hubs(rating_key, 12).await.unwrap_or_default();
    let similar = match kind {
        "track" | "album" => server.nearest(rating_key).await.ok().map(|m| ("Sonically similar", m)),
        "artist" => server.similar(rating_key).await.ok().map(|m| ("Similar artists", m)),
        _ => None,
    };
    if let Some((title, metadata)) = similar.filter(|(_, m)| !m.is_empty()) {
        hubs.push(Hub {
            title: Some(title.into()),
            hub_identifier: Some(format!("patina.{kind}.similar")),
            metadata: metadata.into_iter().filter(|m| m.rating_key.as_deref() != Some(rating_key)).collect(),
            ..Default::default()
        });
    }
    Ok(hubs)
}
