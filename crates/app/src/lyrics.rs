//! Track lyrics from Plex lyric streams (stream type 4): LRC files become
//! timed lines, plain text stays untimed.

use anyhow::Result;
use patina_plex::{MetadataOptions, Params, PlexServer, stream_type};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub time_ms: Option<i64>,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    pub synced: bool,
    pub lines: Vec<Line>,
}

pub async fn load(server: &PlexServer, rating_key: &str) -> Result<Option<Lyrics>> {
    let Some(item) = server.metadata(rating_key, MetadataOptions::default()).await? else { return Ok(None) };
    let key = item
        .media
        .iter()
        .flat_map(|m| &m.parts)
        .flat_map(|p| p.streams_of(stream_type::LYRICS))
        .find_map(|s| s.key.clone());
    let Some(key) = key else { return Ok(None) };
    let (bytes, _) = server.bytes(&key, &Params::new()).await?;
    let lyrics = parse(&String::from_utf8_lossy(&bytes));
    Ok((!lyrics.lines.is_empty()).then_some(lyrics))
}

/// Parse LRC (`[mm:ss.xx] text`, several stamps per line allowed) or plain text.
pub fn parse(text: &str) -> Lyrics {
    let mut timed = Vec::new();
    let mut plain = Vec::new();
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut stamps = Vec::new();
        while let Some(body) = rest.strip_prefix('[') {
            let Some(end) = body.find(']') else { break };
            let tag = &body[..end];
            rest = body[end + 1..].trim_start();
            if let Some(ms) = stamp(tag) {
                stamps.push(ms);
            }
            // Other tags ([ar:], [ti:], [offset:]) are metadata; drop them.
        }
        if stamps.is_empty() {
            if !raw.trim_start().starts_with('[') {
                plain.push(Line { time_ms: None, text: raw.trim().to_string() });
            }
        } else {
            for ms in stamps {
                timed.push(Line { time_ms: Some(ms), text: rest.to_string() });
            }
        }
    }
    if timed.is_empty() {
        while plain.last().is_some_and(|l| l.text.is_empty()) {
            plain.pop();
        }
        return Lyrics { synced: false, lines: plain };
    }
    timed.sort_by_key(|l| l.time_ms);
    Lyrics { synced: true, lines: timed }
}

fn stamp(tag: &str) -> Option<i64> {
    let (m, s) = tag.split_once(':')?;
    let m: i64 = m.trim().parse().ok()?;
    let s: f64 = s.trim().replace(',', ".").parse().ok()?;
    Some(m * 60_000 + (s * 1000.0).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc() {
        let l = parse("[ti:Song]\n[00:01.50]One\n[00:03.00][00:10.00]Two\n");
        assert!(l.synced);
        assert_eq!(l.lines.iter().map(|l| l.time_ms.unwrap()).collect::<Vec<_>>(), vec![1500, 3000, 10000]);
        assert_eq!(l.lines[2].text, "Two");
    }

    #[test]
    fn keeps_plain_text() {
        let l = parse("First line\n\nSecond\n\n");
        assert!(!l.synced);
        assert_eq!(l.lines.len(), 3);
    }
}
