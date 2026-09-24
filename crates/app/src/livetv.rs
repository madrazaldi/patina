//! Live TV & DVR: a channel guide, tuning, and recordings. The server's
//! responses are loosely structured, so they are read defensively from JSON.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use patina_plex::{Metadata, Params, PlexServer};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Program {
    pub title: String,
    /// Episode title or year.
    pub subtitle: Option<String>,
    pub guid: Option<String>,
    pub summary: Option<String>,
    pub thumb: Option<String>,
    pub begins_at: i64,
    pub ends_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub dvr: String,
    pub id: String,
    pub number: Option<String>,
    pub title: String,
    pub thumb: Option<String>,
    pub programs: Vec<Program>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub begins_at: Option<i64>,
    pub channel: Option<String>,
    pub status: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: String,
    pub title: String,
    pub kind: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Recordings {
    pub upcoming: Vec<Recording>,
    pub rules: Vec<Rule>,
}

/// `{Key: [...]}` or `{Key: {...}}` as a list.
fn list<'a>(v: &'a Value, key: &str) -> Vec<&'a Value> {
    match v.get(key) {
        Some(Value::Array(a)) => a.iter().collect(),
        Some(o @ Value::Object(_)) => vec![o],
        _ => Vec::new(),
    }
}

fn mc(v: &Value) -> &Value {
    v.get("MediaContainer").unwrap_or(v)
}

fn text(v: &Value, key: &str) -> Option<String> {
    match v.get(key)? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn int(v: &Value, key: &str) -> Option<i64> {
    match v.get(key)? {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

struct Dvr {
    id: String,
    epg: String,
    lineup: String,
    /// Channel keys enabled on the tuners; empty means all.
    enabled: Vec<String>,
}

async fn dvrs(server: &PlexServer) -> Result<Vec<Dvr>> {
    let v = server.dvrs().await?;
    Ok(list(mc(&v), "Dvr")
        .into_iter()
        .filter_map(|d| {
            let enabled = list(d, "Device")
                .into_iter()
                .flat_map(|dev| list(dev, "ChannelMapping"))
                .filter(|m| m.get("enabled").is_none_or(|e| e == "1" || e == 1 || e == true))
                .filter_map(|m| text(m, "channelKey"))
                .collect();
            Some(Dvr { id: text(d, "key")?, epg: text(d, "epgIdentifier")?, lineup: text(d, "lineup")?, enabled })
        })
        .collect())
}

pub async fn available(server: &PlexServer) -> bool {
    dvrs(server).await.is_ok_and(|d| !d.is_empty())
}

/// Channels with what is on from now for `hours`.
pub async fn guide(server: &PlexServer, hours: i64) -> Result<Vec<Channel>> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs() as i64;
    let mut out = Vec::new();
    for dvr in dvrs(server).await? {
        let epg = format!("/{}", dvr.epg.trim_start_matches('/'));
        let channels = server.lineup_channels(&epg, &dvr.lineup).await?;
        let grid = server.epg_grid(&epg, now, now + hours * 3600).await.unwrap_or(Value::Null);

        let mut programs: HashMap<String, Vec<Program>> = HashMap::new();
        for p in list(mc(&grid), "Metadata") {
            for media in list(p, "Media") {
                let Some(channel) = text(media, "channelIdentifier") else { continue };
                let (Some(begins_at), Some(ends_at)) = (int(media, "beginsAt"), int(media, "endsAt")) else { continue };
                let episode = p.get("type").and_then(Value::as_str) == Some("episode");
                programs.entry(channel).or_default().push(Program {
                    title: if episode { text(p, "grandparentTitle").or(text(p, "title")) } else { text(p, "title") }
                        .unwrap_or_default(),
                    subtitle: if episode { text(p, "title") } else { text(p, "year") },
                    guid: text(p, "guid"),
                    summary: text(p, "summary"),
                    thumb: text(p, "thumb").or(text(p, "grandparentThumb")),
                    begins_at,
                    ends_at,
                });
            }
        }
        for c in list(mc(&channels), "Channel") {
            let Some(id) = text(c, "identifier") else { continue };
            if !dvr.enabled.is_empty() && text(c, "key").is_some_and(|k| !dvr.enabled.contains(&k)) {
                continue;
            }
            let mut list = programs.remove(&id).unwrap_or_default();
            list.sort_by_key(|p| p.begins_at);
            list.dedup_by_key(|p| p.begins_at);
            list.retain(|p| p.ends_at > now);
            out.push(Channel {
                dvr: dvr.id.clone(),
                number: text(c, "channelVcn").or(text(c, "vcn")),
                title: text(c, "title").or(text(c, "callSign")).unwrap_or_else(|| id.clone()),
                thumb: text(c, "thumb"),
                id,
                programs: list,
            });
        }
    }
    out.sort_by(|a, b| {
        let n = |c: &Channel| c.number.as_deref().and_then(|n| n.parse::<f64>().ok()).unwrap_or(f64::MAX);
        n(a).total_cmp(&n(b)).then_with(|| a.title.cmp(&b.title))
    });
    Ok(out)
}

/// Find the first `Metadata` object anywhere in a response.
fn first_metadata(v: &Value) -> Option<&Value> {
    match v {
        Value::Object(o) => {
            if let Some(m) = o.get("Metadata") {
                return match m {
                    Value::Array(a) => a.first(),
                    other => Some(other),
                };
            }
            o.values().find_map(first_metadata)
        }
        Value::Array(a) => a.iter().find_map(first_metadata),
        _ => None,
    }
}

/// Tune a channel; returns the live item to hand to the player.
pub async fn tune(server: &PlexServer, dvr: &str, channel: &str) -> Result<Metadata> {
    let v = server.tune(dvr, channel).await?;
    let m = first_metadata(&v).ok_or_else(|| anyhow!("The tuner did not start a session."))?;
    let item: Metadata = serde_json::from_value(m.clone())?;
    if item.key.is_none() {
        return Err(anyhow!("The tuner did not start a session."));
    }
    Ok(item)
}

pub async fn recordings(server: &PlexServer) -> Result<Recordings> {
    let (scheduled, subscriptions) = tokio::join!(server.scheduled_recordings(), server.subscriptions());
    let scheduled = scheduled.unwrap_or(Value::Null);
    let subscriptions = subscriptions?;
    let upcoming = list(mc(&scheduled), "MediaGrabOperation")
        .into_iter()
        .filter_map(|g| {
            let m = g.get("Metadata")?;
            let media = list(m, "Media").into_iter().next();
            let episode = m.get("type").and_then(Value::as_str) == Some("episode");
            Some(Recording {
                id: text(g, "id").or(text(g, "mediaSubscriptionID"))?,
                title: if episode { text(m, "grandparentTitle") } else { text(m, "title") }.unwrap_or_default(),
                subtitle: if episode { text(m, "title") } else { None },
                begins_at: media.and_then(|x| int(x, "beginsAt")),
                channel: media.and_then(|x| text(x, "channelTitle").or(text(x, "channelCallSign"))),
                status: text(g, "status"),
            })
        })
        .collect();
    let rules = list(mc(&subscriptions), "MediaSubscription")
        .into_iter()
        .filter_map(|s| {
            let target = ["Directory", "Video", "Metadata"].iter().find_map(|k| list(s, k).into_iter().next());
            Some(Rule {
                id: text(s, "key")?,
                title: text(s, "title").or(target.and_then(|t| text(t, "title"))).unwrap_or_default(),
                kind: text(s, "type"),
            })
        })
        .collect();
    Ok(Recordings { upcoming, rules })
}

/// Record a guide item with the server's default rule for it.
pub async fn record(server: &PlexServer, guid: &str) -> Result<()> {
    let v = server.subscription_template(guid).await?;
    let template = list(mc(&v), "SubscriptionTemplate")
        .into_iter()
        .flat_map(|t| list(t, "MediaSubscription"))
        .next()
        .ok_or_else(|| anyhow!("This program cannot be recorded."))?;
    let mut p = Params::new().set("includeGrabs", 1);
    if let Some(params) = text(template, "parameters") {
        for (k, v) in url::form_urlencoded::parse(params.as_bytes()) {
            p.push(k.into_owned(), v);
        }
    }
    for key in ["type", "targetLibrarySectionID", "targetSectionLocationID"] {
        if let Some(v) = text(template, key) {
            p.push(key, v);
        }
    }
    for s in list(template, "Setting") {
        if let (Some(id), Some(value)) = (text(s, "id"), text(s, "value").or(text(s, "default"))) {
            p.push(format!("prefs[{id}]"), value);
        }
    }
    server.create_subscription(&p).await?;
    Ok(())
}
