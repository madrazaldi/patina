//! The `plximg://` scheme. The webview loads `plximg://localhost/i?u=<path>&w=&h=`
//! URLs, served through core's artwork cache so the token never reaches the
//! DOM. `plximg://localhost/d?id=<download>` serves the saved artwork of a
//! download, which also works offline.

use patina_core::{AppState, images, servers};
use tauri::{AppHandle, Manager, UriSchemeResponder};
use tauri::http::{Request, Response};

pub const SCHEME: &str = "plximg";

pub fn handle(handle: AppHandle, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let query = request.uri().query().unwrap_or("").to_string();
    if request.uri().path() == "/d" {
        let id = url::form_urlencoded::parse(query.as_bytes()).find(|(k, _)| k == "id").map(|(_, v)| v.into_owned());
        let response = match id.and_then(|id| images::download_poster(&id)) {
            Some((bytes, ctype)) => Response::builder().status(200).header("Content-Type", ctype).body(bytes),
            None => Response::builder().status(404).body(Vec::new()),
        };
        responder.respond(response.unwrap());
        return;
    }
    tauri::async_runtime::spawn(async move {
        let mut source = String::new();
        // `s` names another server of the account that the artwork comes from.
        let mut server = None;
        let (mut w, mut h) = (300u32, 450u32);
        for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
            match k.as_ref() {
                "u" => source = v.into_owned(),
                "s" => server = Some(v.into_owned()).filter(|s| !s.is_empty()),
                "w" => w = v.parse().unwrap_or(w),
                "h" => h = v.parse().unwrap_or(h),
                _ => {}
            }
        }
        let found = match (server, handle.try_state::<AppState>()) {
            (Some(id), Some(app)) if !servers::is_current(&app, Some(&id)) => servers::image(&app, &id, &source, w, h).await,
            _ => images::fetch(&source, w, h).await,
        };
        let response = match found {
            Some((bytes, ctype)) => Response::builder()
                .status(200)
                .header("Content-Type", ctype)
                .header("Cache-Control", "public, max-age=604800")
                .header("Access-Control-Allow-Origin", "*")
                .body(bytes),
            None => Response::builder().status(404).body(Vec::new()),
        };
        responder.respond(response.unwrap());
    });
}
