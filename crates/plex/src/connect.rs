use std::time::{Duration, Instant};

use futures::StreamExt;
use futures::stream::FuturesUnordered;

use crate::error::{Error, Result};
use crate::models::Resource;
use crate::server::PlexServer;
use crate::tv::PlexTv;

/// A candidate URI for reaching a server, ranked by preference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionCandidate {
    pub uri: String,
    pub local: bool,
    pub relay: bool,
    /// Lower is better.
    pub rank: u8,
}

impl ConnectionCandidate {
    pub fn candidates(resource: &Resource) -> Vec<ConnectionCandidate> {
        let mut out: Vec<ConnectionCandidate> = Vec::new();
        let mut push = |uri: String, local: bool, relay: bool, rank: u8| {
            if !out.iter().any(|c| c.uri == uri) {
                out.push(ConnectionCandidate { uri, local, relay, rank });
            }
        };
        for c in &resource.connections {
            let Some(uri) = c.uri.clone() else { continue };
            let https = uri.starts_with("https");
            let rank = match (c.local, c.relay, https) {
                (true, false, true) => 0,
                (true, false, false) => 1,
                (false, false, true) => 2,
                (false, false, false) => 3,
                (_, true, _) => 5,
            };
            push(uri, c.local, c.relay, rank);
            // plex.direct names need working DNS rebinding; offer the raw IP too.
            if c.local && !c.relay && !resource.https_required
                && let (Some(addr), Some(port)) = (&c.address, c.port) {
                    let host = if c.ipv6 { format!("[{addr}]") } else { addr.clone() };
                    push(format!("http://{host}:{port}"), true, false, 4);
                }
        }
        out.sort_by_key(|c| c.rank);
        out
    }
}

/// Race every connection of a server resource and return a client bound to
/// the best one that answers. Local direct connections win over remote, and
/// relays are the last resort.
pub async fn connect_resource(tv: &PlexTv, resource: &Resource) -> Result<PlexServer> {
    let token = resource
        .access_token
        .clone()
        .or_else(|| tv.token().map(str::to_string))
        .ok_or(Error::Unauthorized)?;
    let candidates = ConnectionCandidate::candidates(resource);
    if candidates.is_empty() {
        return Err(Error::NoConnection(resource.name().to_string()));
    }

    let mut pending: FuturesUnordered<_> = candidates
        .iter()
        .map(|c| {
            let server = PlexServer::with_http(tv.http().clone(), &c.uri, &token);
            let rank = c.rank;
            async move {
                let server = server?;
                let ok = tokio::time::timeout(Duration::from_secs(6), server.identity()).await;
                match ok {
                    Ok(Ok(id)) => Ok((rank, server.with_machine_identifier(id.machine_identifier))),
                    Ok(Err(e)) => Err(e),
                    Err(_) => Err(Error::Other("timed out".into())),
                }
            }
        })
        .collect();

    let best_rank = candidates[0].rank;
    let mut best: Option<(u8, PlexServer)> = None;
    let mut first_success: Option<Instant> = None;
    let mut last_err = None;
    loop {
        // Once something works, give better-ranked candidates a short grace period.
        let grace = first_success.map(|t| Duration::from_millis(1200).saturating_sub(t.elapsed()));
        let next = match grace {
            Some(g) => match tokio::time::timeout(g, pending.next()).await {
                Ok(n) => n,
                Err(_) => break,
            },
            None => pending.next().await,
        };
        let Some(result) = next else { break };
        match result {
            Ok((rank, server)) => {
                first_success.get_or_insert_with(Instant::now);
                if best.as_ref().is_none_or(|(r, _)| rank < *r) {
                    best = Some((rank, server));
                }
                if rank == best_rank {
                    break;
                }
            }
            Err(e) => {
                tracing::debug!("connection attempt failed: {e}");
                last_err = Some(e);
            }
        }
    }
    match best {
        Some((_, server)) => Ok(server),
        None => Err(match last_err {
            Some(Error::Unauthorized) => Error::Unauthorized,
            _ => Error::NoConnection(resource.name().to_string()),
        }),
    }
}
