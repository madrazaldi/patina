//! The `plximg://` scheme. The webview loads `plximg://localhost/i?u=<path>&w=&h=`
//! URLs, served through core's artwork cache so the token never reaches the
//! DOM. `plximg://localhost/d?id=<download>` serves the saved artwork of a
//! download, which also works offline.

use patina_core::images;
use tauri::UriSchemeResponder;
use tauri::http::{Request, Response};

pub const SCHEME: &str = "plximg";

pub fn handle(request: Request<Vec<u8>>, responder: UriSchemeResponder) {
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
        let (mut w, mut h) = (300u32, 450u32);
        for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
            match k.as_ref() {
                "u" => source = v.into_owned(),
                "w" => w = v.parse().unwrap_or(w),
                "h" => h = v.parse().unwrap_or(h),
                _ => {}
            }
        }
        let response = match images::fetch(&source, w, h).await {
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
