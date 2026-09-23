use reqwest::Method;

use super::PlexServer;
use crate::client::Params;
use crate::error::Result;
use crate::models::*;

impl PlexServer {
    pub async fn collections(&self, section_id: &str) -> Result<Vec<Metadata>> {
        Ok(self
            .get(&format!("/library/sections/{section_id}/collections"), &Params::new().set("includeCollections", 1))
            .await?
            .metadata)
    }

    pub async fn collection_items(&self, collection_id: &str) -> Result<MediaContainer> {
        self.get(&format!("/library/collections/{collection_id}/children"), &Params::new()).await
    }

    /// Create a collection. `kind` is the media type id of its items.
    pub async fn create_collection(&self, section_id: &str, title: &str, kind: u32, uri: Option<&str>, smart: bool) -> Result<Option<Metadata>> {
        let p = Params::new()
            .set("sectionId", section_id)
            .set("title", title)
            .set("type", kind)
            .set("smart", smart as u8)
            .opt("uri", uri);
        Ok(self.container(Method::POST, "/library/collections", &p).await?.metadata.into_iter().next())
    }

    pub async fn add_to_collection(&self, collection_id: &str, uri: &str) -> Result<()> {
        self.put(&format!("/library/collections/{collection_id}/items"), &Params::new().set("uri", uri)).await
    }

    pub async fn remove_from_collection(&self, collection_id: &str, rating_key: &str) -> Result<()> {
        self.delete(&format!("/library/collections/{collection_id}/items/{rating_key}"), &Params::new()).await
    }

    /// Move an item inside a custom-sorted collection.
    pub async fn move_collection_item(&self, collection_id: &str, rating_key: &str, after: Option<&str>) -> Result<()> {
        self.put(&format!("/library/collections/{collection_id}/items/{rating_key}/move"), &Params::new().opt("after", after))
            .await
    }

    pub async fn delete_collection(&self, collection_id: &str) -> Result<()> {
        self.delete(&format!("/library/collections/{collection_id}"), &Params::new()).await
    }

    /// Collection mode: -1 library default, 0 hide, 1 hide items, 2 show items.
    pub async fn set_collection_mode(&self, collection_id: &str, mode: i32) -> Result<()> {
        self.set_metadata_prefs(collection_id, &Params::new().set("collectionMode", mode)).await
    }

    /// Collection sort: 0 release date, 1 alphabetical, 2 custom.
    pub async fn set_collection_sort(&self, collection_id: &str, sort: i32) -> Result<()> {
        self.set_metadata_prefs(collection_id, &Params::new().set("collectionSort", sort)).await
    }
}
