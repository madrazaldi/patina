//! Small formatting helpers for the UI.

use patina_plex::Metadata;

/// "2h 14m", "48m", "35s".
pub fn duration(ms: i64) -> String {
    let secs = ms / 1000;
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    match (h, m) {
        (0, 0) => format!("{secs}s"),
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

/// "1:02:03" / "4:05" clock format.
pub fn clock(ms: i64) -> String {
    let secs = ms.max(0) / 1000;
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

pub fn remaining(item: &Metadata) -> Option<String> {
    let left = item.duration? - item.view_offset?;
    (left > 0).then(|| format!("{} left", duration(left)))
}

pub fn bytes(n: i64) -> String {
    let n = n as f64;
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{n} B") } else { format!("{v:.1} {}", UNITS[i]) }
}

pub fn bitrate(kbps: i64) -> String {
    if kbps >= 1000 { format!("{:.1} Mbps", kbps as f64 / 1000.0) } else { format!("{kbps} kbps") }
}

/// "Mar 4, 2024" from a unix timestamp.
pub fn date(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0).map(|d| d.format("%b %-d, %Y").to_string()).unwrap_or_default()
}

/// "3 hours ago", "yesterday", "Mar 4".
pub fn ago(ts: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let d = now - ts;
    match d {
        d if d < 60 => "just now".into(),
        d if d < 3600 => format!("{} min ago", d / 60),
        d if d < 86400 => format!("{} h ago", d / 3600),
        d if d < 172800 => "yesterday".into(),
        d if d < 86400 * 7 => format!("{} days ago", d / 86400),
        _ => date(ts),
    }
}

/// "2024-03-04" → "Mar 4, 2024".
pub fn air_date(s: &str) -> String {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").map(|d| d.format("%b %-d, %Y").to_string()).unwrap_or_else(|_| s.to_string())
}

/// Secondary line under a card title.
pub fn subtitle(item: &Metadata) -> String {
    match item.kind() {
        "episode" => {
            let show = item.grandparent_title.as_deref().unwrap_or("");
            let code = item.episode_code();
            if show.is_empty() { code } else { format!("{show} · {code}") }
        }
        "season" => item
            .leaf_count
            .map(|n| format!("{n} episodes"))
            .or_else(|| item.parent_title.clone())
            .unwrap_or_default(),
        "show" => item
            .child_count
            .map(|n| format!("{n} season{}", if n == 1 { "" } else { "s" }))
            .or(item.year.map(|y| y.to_string()))
            .unwrap_or_default(),
        "album" => item.parent_title.clone().or(item.year.map(|y| y.to_string())).unwrap_or_default(),
        "track" => item.grandparent_title.clone().unwrap_or_default(),
        "playlist" => {
            let n = item.leaf_count.unwrap_or(0);
            format!("{n} item{}", if n == 1 { "" } else { "s" })
        }
        "collection" => item.child_count.map(|n| format!("{n} items")).unwrap_or_default(),
        _ => item.year.map(|y| y.to_string()).unwrap_or_default(),
    }
}

/// Card title: episodes show their own title, everything else its name.
pub fn card_title(item: &Metadata) -> String {
    item.title().to_string()
}

pub fn initials(name: &str) -> String {
    name.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase()
}

pub fn hex_encode(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

pub fn hex_decode(s: &str) -> String {
    let bytes: Vec<u8> = (0..s.len() / 2).filter_map(|i| u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok()).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(duration(8_040_000), "2h 14m");
        assert_eq!(clock(3_723_000), "1:02:03");
        assert_eq!(clock(245_000), "4:05");
        assert_eq!(hex_decode(&hex_encode("/library/sections/1/all?genre=5&x=ü")), "/library/sections/1/all?genre=5&x=ü");
    }
}
