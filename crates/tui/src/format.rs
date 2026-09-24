//! Human-readable formatting helpers.

use chrono::{Local, TimeZone};

pub fn duration_ms(ms: i64) -> String {
    let total = ms.max(0) / 1000;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{s}s")
    }
}

pub fn clock_ms(ms: i64) -> String {
    let total = ms.max(0) / 1000;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

pub fn bytes(n: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n.max(0) as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{n} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

pub fn bitrate_kbps(kbps: i64) -> String {
    if kbps >= 1000 {
        format!("{:.1} Mbps", kbps as f64 / 1000.0)
    } else {
        format!("{kbps} kbps")
    }
}

pub fn date(ts: i64) -> String {
    Local
        .timestamp_opt(ts, 0)
        .single()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

pub fn relative(ts: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let d = now - ts;
    match d {
        i64::MIN..60 => "just now".into(),
        60..3600 => format!("{}m ago", d / 60),
        3600..86400 => format!("{}h ago", d / 3600),
        86400..2_592_000 => format!("{}d ago", d / 86400),
        _ => date(ts),
    }
}

pub fn resolution(height: Option<i64>, label: Option<&str>) -> Option<String> {
    if let Some(l) = label {
        return Some(match l {
            "4k" => "4K".into(),
            "sd" => "SD".into(),
            l if l.chars().all(|c| c.is_ascii_digit()) => format!("{l}p"),
            l => l.to_uppercase(),
        });
    }
    height.map(|h| match h {
        h if h >= 2000 => "4K".into(),
        h if h >= 1000 => "1080p".into(),
        h if h >= 700 => "720p".into(),
        h => format!("{h}p"),
    })
}

pub fn channels(n: i64) -> String {
    match n {
        1 => "Mono".into(),
        2 => "Stereo".into(),
        6 => "5.1".into(),
        8 => "7.1".into(),
        n => format!("{n}ch"),
    }
}

pub fn codec(c: &str) -> String {
    match c.to_ascii_lowercase().as_str() {
        "h264" => "H.264".into(),
        "hevc" | "h265" => "HEVC".into(),
        "av1" => "AV1".into(),
        "mpeg4" => "MPEG-4".into(),
        "eac3" => "E-AC3".into(),
        "ac3" => "AC3".into(),
        "truehd" => "TrueHD".into(),
        "dca" | "dts" => "DTS".into(),
        "aac" => "AAC".into(),
        "flac" => "FLAC".into(),
        "mp3" => "MP3".into(),
        "opus" => "Opus".into(),
        other => other.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats() {
        assert_eq!(duration_ms(7_380_000), "2h 03m");
        assert_eq!(duration_ms(300_000), "5m");
        assert_eq!(clock_ms(3_725_000), "1:02:05");
        assert_eq!(clock_ms(65_000), "1:05");
        assert_eq!(bytes(512), "512 B");
        assert_eq!(bytes(1536), "1.5 KB");
        assert_eq!(bitrate_kbps(12_400), "12.4 Mbps");
        assert_eq!(resolution(None, Some("1080")).as_deref(), Some("1080p"));
        assert_eq!(resolution(Some(2160), None).as_deref(), Some("4K"));
        assert_eq!(channels(6), "5.1");
        assert_eq!(codec("hevc"), "HEVC");
    }
}
