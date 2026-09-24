//! Editing items: fields, tags and collections.

use anyhow::{Result, anyhow};
use patina_plex::{MediaType, MetadataOptions};
use serde::Deserialize;

use crate::state::AppState;

#[derive(Clone, Debug, Deserialize)]
pub struct TagEdit {
    /// `genre`, `label`, `collection`, `mood`, ...
    pub tag: String,
    pub add: Vec<String>,
    pub remove: Vec<String>,
}

/// Edit fields (locking them so refreshes keep the change) and tags.
pub async fn edit_metadata(app: &AppState, rating_key: &str, fields: &[(String, String)], tags: &[TagEdit]) -> Result<()> {
    let server = app.server()?;
    let item = server.metadata(rating_key, MetadataOptions::default()).await?.ok_or_else(|| anyhow!("Item not found."))?;
    let kind = MediaType::from_name(item.kind()).map(MediaType::id).ok_or_else(|| anyhow!("This item cannot be edited."))?;
    if !fields.is_empty() {
        let pairs: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        server.edit_metadata(rating_key, item.library_section_id, kind, &pairs, true).await?;
    }
    for t in tags.iter().filter(|t| !t.add.is_empty() || !t.remove.is_empty()) {
        let add: Vec<&str> = t.add.iter().map(String::as_str).collect();
        let remove: Vec<&str> = t.remove.iter().map(String::as_str).collect();
        server.edit_tags(rating_key, kind, &t.tag, &add, &remove).await?;
    }
    Ok(())
}

/// Add an item to a collection, or to a new one called `title`.
pub async fn add_to_collection(app: &AppState, rating_key: &str, collection: Option<&str>, title: Option<&str>) -> Result<()> {
    let server = app.server()?;
    let uri = server.item_uri(&[rating_key]);
    match (collection, title) {
        (Some(id), _) => server.add_to_collection(id, &uri).await?,
        (None, Some(title)) => {
            let item = server.metadata(rating_key, MetadataOptions::default()).await?.ok_or_else(|| anyhow!("Item not found."))?;
            let section = item.library_section_id.ok_or_else(|| anyhow!("The item is not in a library."))?;
            let kind = MediaType::from_name(item.kind()).map(MediaType::id).unwrap_or(1);
            server.create_collection(&section.to_string(), title, kind, Some(&uri), false).await?;
        }
        (None, None) => return Err(anyhow!("No collection given.")),
    }
    Ok(())
}
