//! A tiny HTTP server for the Seerr tests: no test talks to a real server.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// A request as the fake server saw it. Header names are lowercase.
pub struct Req {
    pub method: String,
    pub path: String,
    pub query: String,
    pub headers: HashMap<String, String>,
    pub body: String,
}

/// An answer: status, extra headers and a JSON body (empty for none).
pub struct Resp(pub u16, pub Vec<(String, String)>, pub String);

impl Resp {
    pub fn json(status: u16, body: serde_json::Value) -> Self {
        Resp(status, vec![], body.to_string())
    }
}

/// A tiny HTTP server on a free local port, answering with `handler`.
pub async fn serve(handler: impl Fn(&Req) -> Resp + Send + Sync + 'static) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handler = Arc::new(handler);
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else { return };
            let handler = handler.clone();
            tokio::spawn(async move {
                let mut raw = Vec::new();
                let mut buf = [0u8; 4096];
                let head_end = loop {
                    let n = socket.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        return;
                    }
                    raw.extend_from_slice(&buf[..n]);
                    if let Some(at) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        break at + 4;
                    }
                };
                let head = String::from_utf8_lossy(&raw[..head_end]).into_owned();
                let mut lines = head.lines();
                let mut first = lines.next().unwrap_or("").split(' ');
                let (method, target) = (first.next().unwrap_or("").to_owned(), first.next().unwrap_or(""));
                let (path, query) = target.split_once('?').unwrap_or((target, ""));
                let headers: HashMap<String, String> = lines
                    .filter_map(|l| l.split_once(':'))
                    .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
                    .collect();
                let length: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
                while raw.len() < head_end + length {
                    let n = socket.read(&mut buf).await.unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    raw.extend_from_slice(&buf[..n]);
                }
                let body = String::from_utf8_lossy(&raw[head_end..]).into_owned();
                let req = Req { method, path: path.to_owned(), query: query.to_owned(), headers, body };
                let Resp(status, extra, body) = handler(&req);
                let mut reply = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
                for (k, v) in extra {
                    reply.push_str(&format!("{k}: {v}\r\n"));
                }
                reply.push_str("\r\n");
                reply.push_str(&body);
                let _ = socket.write_all(reply.as_bytes()).await;
                let _ = socket.shutdown().await;
            });
        }
    });
    url
}
