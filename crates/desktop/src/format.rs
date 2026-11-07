//! Formatting for the UI, and helpers over Plex metadata that the models
//! themselves do not have (the webview's `format.ts` and `plex.ts`).

use std::time::{SystemTime, UNIX_EPOCH};

use patina_plex::Metadata;

/// "2h 14m", "48m", "35s".
pub fn duration(ms: i64) -> String {
    let secs = ms.max(0) / 1000;
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    match (h, m) {
        (0, 0) => format!("{secs}s"),
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

/// "1:02:03" / "4:05".
pub fn clock(ms: i64) -> String {
    let secs = ms.max(0) / 1000;
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// "48m left" for something partly watched.
pub fn remaining(m: &Metadata) -> Option<String> {
    let left = m.duration? - m.view_offset?;
    (left > 0).then(|| format!("{} left", duration(left)))
}

pub fn bytes(n: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024. && i < UNITS.len() - 1 {
        v /= 1024.;
        i += 1;
    }
    if i == 0 { format!("{n} B") } else { format!("{v:.1} {}", UNITS[i]) }
}

pub fn bitrate(kbps: i64) -> String {
    if kbps >= 1000 { format!("{:.1} Mbps", kbps as f64 / 1000.) } else { format!("{kbps} kbps") }
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// Days since 1970-01-01 → (year, month 1..=12, day).
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The local UTC offset is not known without a timezone database, so dates are UTC.
pub fn date(ts: i64) -> String {
    let (y, m, d) = civil(ts.div_euclid(86_400));
    format!("{} {d}, {y}", MONTHS[(m - 1) as usize])
}

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// "3 h ago", "yesterday", "Mar 4, 2024".
pub fn ago(ts: i64) -> String {
    let d = now() - ts;
    match d {
        ..60 => "just now".into(),
        60..3600 => format!("{} min ago", d / 60),
        3600..86_400 => format!("{} h ago", d / 3600),
        86_400..172_800 => "yesterday".into(),
        172_800..604_800 => format!("{} days ago", d / 86_400),
        _ => date(ts),
    }
}

/// "2024-03-04" → "Mar 4, 2024".
pub fn air_date(s: &str) -> String {
    let parts: Vec<&str> = s.split('-').collect();
    if let [y, m, d] = parts[..]
        && let (Ok(y), Ok(m), Ok(d)) = (y.parse::<i64>(), m.parse::<usize>(), d.parse::<u32>())
        && (1..=12).contains(&m)
    {
        return format!("{} {d}, {y}", MONTHS[m - 1]);
    }
    s.to_string()
}

pub fn initials(name: &str) -> String {
    name.split_whitespace().take(2).filter_map(|w| w.chars().next()).flat_map(char::to_uppercase).collect()
}

/// "1,234".
pub fn count(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

/// "1 episode", "3 episodes".
pub fn plural(n: i64, word: &str) -> String {
    format!("{} {}{}", count(n), word, if n == 1 { "" } else { "s" })
}

// Metadata ------------------------------------------------------------------

/// Something a Play button makes sense for.
pub fn can_play(m: &Metadata) -> bool {
    m.is_playable() || m.is_container()
}

/// Secondary line under a card title.
pub fn subtitle(m: &Metadata) -> String {
    let year = || m.year.map(|y| y.to_string()).unwrap_or_default();
    match m.kind() {
        "episode" => {
            let show = m.grandparent_title.clone().unwrap_or_default();
            if show.is_empty() { m.episode_code() } else { format!("{show} · {}", m.episode_code()) }
        }
        "season" => m.leaf_count.map(|n| plural(n, "episode")).unwrap_or_else(|| m.parent_title.clone().unwrap_or_default()),
        "show" => m.child_count.map(|n| plural(n, "season")).unwrap_or_else(year),
        "album" => m.parent_title.clone().unwrap_or_else(year),
        "track" => m.grandparent_title.clone().unwrap_or_default(),
        "playlist" => plural(m.leaf_count.unwrap_or(0), "item"),
        "collection" => m.child_count.map(|n| plural(n, "item")).unwrap_or_default(),
        _ => year(),
    }
}

/// Title and subtitle for "now playing"-style rows.
pub fn playing_lines(m: &Metadata) -> (String, String) {
    match m.kind() {
        "episode" => (m.grandparent_title.clone().unwrap_or_default(), format!("{} · {}", m.episode_code(), m.title())),
        "track" => (m.title().to_string(), m.grandparent_title.clone().unwrap_or_default()),
        _ => (m.title().to_string(), m.year.map(|y| y.to_string()).unwrap_or_default()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Poster,
    Landscape,
    Square,
}

impl Shape {
}

pub fn shape_for(m: &Metadata) -> Shape {
    match m.kind() {
        "episode" | "clip" => Shape::Landscape,
        "album" | "artist" | "track" | "playlist" | "photo" | "photoalbum" => Shape::Square,
        _ => Shape::Poster,
    }
}

/// One shape for a row of mixed items, so the cards line up: theirs if they
/// agree, else posters (episodes show their show's poster).
pub fn common_shape(items: &[Metadata]) -> Shape {
    let mut shapes: Vec<Shape> = items.iter().map(shape_for).collect();
    shapes.dedup();
    shapes.sort_by_key(|s| *s as u8);
    shapes.dedup();
    match shapes[..] {
        [one] => one,
        _ if shapes.is_empty() || shapes.contains(&Shape::Poster) => Shape::Poster,
        _ => Shape::Square,
    }
}

/// Playlist type an item can be added to.
pub fn playlist_kind(m: &Metadata) -> &'static str {
    match m.kind() {
        "track" | "album" | "artist" => "audio",
        "photo" | "photoalbum" => "photo",
        _ => "video",
    }
}

/// Whether the bulk audio/subtitle dialog applies.
pub fn supports_tracks(m: &Metadata) -> bool {
    matches!(m.kind(), "show" | "season" | "episode")
}

/// Things that have a watched state worth toggling.
pub fn has_watch_state(m: &Metadata) -> bool {
    !matches!(m.kind(), "playlist" | "photo" | "photoalbum" | "artist" | "album" | "track" | "collection")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_and_clocks() {
        assert_eq!(duration(35_000), "35s");
        assert_eq!(duration(48 * 60_000), "48m");
        assert_eq!(duration(2 * 3_600_000), "2h");
        assert_eq!(duration(2 * 3_600_000 + 14 * 60_000), "2h 14m");
        assert_eq!(clock(3_723_000), "1:02:03");
        assert_eq!(clock(245_000), "4:05");
    }

    #[test]
    fn dates() {
        assert_eq!(date(1_709_510_400), "Mar 4, 2024");
        assert_eq!(air_date("2024-03-04"), "Mar 4, 2024");
        assert_eq!(air_date("soon"), "soon");
    }

    #[test]
    fn counts() {
        assert_eq!(count(1234567), "1,234,567");
        assert_eq!(count(12), "12");
        assert_eq!(plural(1, "item"), "1 item");
        assert_eq!(plural(1200, "item"), "1,200 items");
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(1536), "1.5 KB");
        assert_eq!(initials("server owner"), "SO");
    }
}
