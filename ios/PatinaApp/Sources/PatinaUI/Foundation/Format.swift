// Port of web/src/lib/format.ts: small formatting helpers for the UI.

import Foundation

public enum Format {
    /// "2h 14m", "48m", "35s".
    public static func duration(ms: Int64) -> String {
        let secs = max(0, ms) / 1000
        let h = secs / 3600
        let m = (secs % 3600) / 60
        if h == 0 && m == 0 { return "\(secs)s" }
        if h == 0 { return "\(m)m" }
        if m == 0 { return "\(h)h" }
        return "\(h)h \(m)m"
    }

    /// "1:02:03" / "4:05".
    public static func clock(ms: Int64) -> String {
        clock(seconds: Double(ms) / 1000)
    }

    public static func clock(seconds: Double) -> String {
        let secs = Int64(max(0, seconds.isFinite ? seconds : 0))
        let h = secs / 3600
        let m = (secs % 3600) / 60
        let s = secs % 60
        func pad(_ n: Int64) -> String { n < 10 ? "0\(n)" : "\(n)" }
        return h > 0 ? "\(h):\(pad(m)):\(pad(s))" : "\(m):\(pad(s))"
    }

    /// "48m left", or nil when there is no resume point.
    public static func remaining(durationMs: Int64?, viewOffsetMs: Int64?) -> String? {
        guard let duration = durationMs, let offset = viewOffsetMs else { return nil }
        let left = duration - offset
        return left > 0 ? "\(Self.duration(ms: left)) left" : nil
    }

    public static func bytes(_ n: Int64) -> String {
        let units = ["B", "KB", "MB", "GB", "TB"]
        var v = Double(n)
        var i = 0
        while v >= 1024 && i < units.count - 1 {
            v /= 1024
            i += 1
        }
        return i == 0 ? "\(n) B" : String(format: "%.1f %@", v, units[i])
    }

    public static func bitrate(kbps: Int64) -> String {
        kbps >= 1000 ? String(format: "%.1f Mbps", Double(kbps) / 1000) : "\(kbps) kbps"
    }

    private static let dateFormatter: DateFormatter = {
        let f = DateFormatter()
        f.locale = Locale(identifier: "en_US")
        f.dateFormat = "MMM d, yyyy"
        return f
    }()

    /// "Mar 4, 2024" from a unix timestamp.
    public static func date(_ ts: Int64) -> String {
        dateFormatter.string(from: Date(timeIntervalSince1970: TimeInterval(ts)))
    }

    /// "3 h ago", "yesterday", "Mar 4, 2024".
    public static func ago(_ ts: Int64, now: Date = Date()) -> String {
        let d = now.timeIntervalSince1970 - TimeInterval(ts)
        if d < 60 { return "just now" }
        if d < 3600 { return "\(Int(d / 60)) min ago" }
        if d < 86400 { return "\(Int(d / 3600)) h ago" }
        if d < 172_800 { return "yesterday" }
        if d < 86400 * 7 { return "\(Int(d / 86400)) days ago" }
        return date(ts)
    }

    /// "2024-03-04" → "Mar 4, 2024".
    public static func airDate(_ s: String) -> String {
        let parts = s.split(separator: "-")
        guard parts.count == 3, parts[0].count == 4, let y = Int(parts[0]), let m = Int(parts[1]), let d = Int(parts[2]),
              let date = Calendar(identifier: .gregorian).date(from: DateComponents(year: y, month: m, day: d))
        else { return s }
        return dateFormatter.string(from: date)
    }

    public static func initials(_ name: String) -> String {
        name.split(whereSeparator: \.isWhitespace).prefix(2).compactMap(\.first).map(String.init).joined().uppercased()
    }

    private static let countFormatter: NumberFormatter = {
        let f = NumberFormatter()
        f.locale = Locale(identifier: "en_US")
        f.numberStyle = .decimal
        return f
    }()

    public static func count(_ n: Int) -> String {
        countFormatter.string(from: NSNumber(value: n)) ?? "\(n)"
    }

    public static func plural(_ n: Int, _ word: String, _ many: String? = nil) -> String {
        "\(count(n)) \(n == 1 ? word : (many ?? word + "s"))"
    }
}
