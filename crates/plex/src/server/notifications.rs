use futures::{Stream, StreamExt};
use reqwest::Method;
use serde_json::Value;

use super::PlexServer;
use crate::client::Params;
use crate::error::{Error, Result};

/// A server-sent event from `/:/eventsource/notifications`.
#[derive(Debug, Clone, PartialEq)]
pub struct Notification {
    /// `playing`, `activity`, `timeline`, `transcodeSession.update`, `status`, ...
    pub event: String,
    pub data: Value,
}

impl PlexServer {
    /// Live server notifications (playback state, scans, activities, library changes).
    /// `filters` limits the event types; empty means all.
    pub async fn notifications(&self, filters: &[&str]) -> Result<impl Stream<Item = Result<Notification>> + use<>> {
        let p = if filters.is_empty() { Params::new() } else { Params::new().set("filters", filters.join(",")) };
        let rb = self
            .http
            .client
            .request(Method::GET, self.url("/:/eventsource/notifications")?)
            .query(&p.0)
            .header("X-Plex-Token", &self.token)
            .header("Accept", "text/event-stream")
            .timeout(std::time::Duration::from_secs(60 * 60 * 24 * 365));
        let resp = self.http.send(rb).await?;
        let bytes = resp.bytes_stream();

        struct State<S> {
            inner: S,
            buf: String,
            event: String,
            data: String,
        }
        let state = State { inner: Box::pin(bytes), buf: String::new(), event: String::new(), data: String::new() };

        Ok(futures::stream::unfold(state, |mut st| async move {
            loop {
                // Drain complete lines already buffered.
                while let Some(pos) = st.buf.find('\n') {
                    let line: String = st.buf.drain(..=pos).collect();
                    let line = line.trim_end_matches(['\r', '\n']);
                    if line.is_empty() {
                        if st.data.is_empty() {
                            st.event.clear();
                            continue;
                        }
                        let data = std::mem::take(&mut st.data);
                        let event = std::mem::take(&mut st.event);
                        let data = serde_json::from_str(&data).unwrap_or(Value::String(data));
                        let event = if event.is_empty() { "message".to_string() } else { event };
                        return Some((Ok(Notification { event, data }), st));
                    }
                    if let Some(v) = line.strip_prefix("event:") {
                        st.event = v.trim().to_string();
                    } else if let Some(v) = line.strip_prefix("data:") {
                        if !st.data.is_empty() {
                            st.data.push('\n');
                        }
                        st.data.push_str(v.trim_start());
                    }
                }
                match st.inner.next().await {
                    Some(Ok(chunk)) => st.buf.push_str(&String::from_utf8_lossy(&chunk)),
                    Some(Err(e)) => return Some((Err(Error::Http(e)), st)),
                    None => return None,
                }
            }
        }))
    }
}
