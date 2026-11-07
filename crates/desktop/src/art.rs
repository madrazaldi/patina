//! Artwork. Images come through the core's artwork cache (which adds the
//! server token and keeps resized copies on disk), are decoded off the UI
//! thread, and are cached in memory by GPUI's asset system, which redraws the
//! view that asked once an image is ready.

use std::sync::Arc;

use gpui_kit::*;
use patina_core::{AppState, images, servers};
use smallvec::smallvec;

use crate::core::{Core, bg};

/// One image at one size.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ArtKey {
    /// A server-relative path (`/library/metadata/1/thumb/…`) or an absolute URL.
    pub source: String,
    pub w: u32,
    pub h: u32,
    /// Machine id of another server of the account the image lives on.
    pub server: Option<String>,
    /// The saved artwork of a download (works offline) instead of `source`.
    pub download: Option<String>,
}

impl ArtKey {
    pub fn new(source: impl Into<String>, w: u32, h: u32) -> Self {
        Self { source: source.into(), w, h, server: None, download: None }
    }

    pub fn on(mut self, server: Option<&str>) -> Self {
        self.server = server.map(str::to_string);
        self
    }

    pub fn download(id: impl Into<String>) -> Self {
        Self { source: String::new(), w: 0, h: 0, server: None, download: Some(id.into()) }
    }
}

enum Artwork {}

impl Asset for Artwork {
    type Source = ArtKey;
    type Output = Option<Arc<RenderImage>>;

    fn load(key: ArtKey, cx: &mut App) -> impl Future<Output = Self::Output> + Send + 'static {
        let app = cx.global::<Core>().0.clone();
        bg(async move {
            let bytes = fetch(&app, &key).await?;
            decode(&bytes)
        })
    }
}

async fn fetch(app: &AppState, key: &ArtKey) -> Option<Vec<u8>> {
    if let Some(id) = &key.download {
        return images::download_poster(id).map(|(bytes, _)| bytes);
    }
    let found = match key.server.as_deref() {
        Some(on) if !servers::is_current(app, Some(on)) => servers::image(app, on, &key.source, key.w, key.h).await,
        _ => images::fetch(&key.source, key.w, key.h).await,
    };
    found.map(|(bytes, _)| bytes)
}

fn decode(bytes: &[u8]) -> Option<Arc<RenderImage>> {
    let image = image::load_from_memory(bytes).ok()?;
    let mut buffer = image.into_rgba8();
    // GPUI wants BGRA.
    for px in buffer.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new(smallvec![image::Frame::new(buffer)])))
}

/// An image element for artwork; empty (showing whatever is behind it) until it loads.
pub fn art(key: ArtKey) -> Img {
    img(ImageSource::Custom(Arc::new(move |window: &mut Window, cx: &mut App| {
        window.use_asset::<Artwork>(&key, cx).map(|found| found.ok_or_else(|| ImageCacheError::Asset("no artwork".into())))
    })))
}

/// Artwork for a server image path at a given size (device pixels are doubled for sharpness).
pub fn art_of(source: Option<&str>, w: f32, h: f32, server: Option<&str>) -> Option<Img> {
    let source = source.filter(|s| !s.is_empty())?;
    Some(art(ArtKey::new(source, (w * 2.) as u32, (h * 2.) as u32).on(server)))
}
