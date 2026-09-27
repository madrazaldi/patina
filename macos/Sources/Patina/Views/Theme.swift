import AppKit
import SwiftUI

/// Patina's look: quiet and typographic. Colour comes from the artwork; the
/// interface stays neutral, with one muted accent (chosen in Settings) for
/// progress, watched state and selection. No gradients or glows.
enum Theme {
    /// "Paper": near-white on dark, ink on light, so it stays visible in both.
    static let defaultAccent = Color(nsColor: NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.darkAqua, .aqua]) == .darkAqua
            ? NSColor(srgbRed: 0.902, green: 0.882, blue: 0.839, alpha: 1)
            : NSColor(srgbRed: 0.18, green: 0.176, blue: 0.165, alpha: 1)
    })

    /// Page titles and item titles: New York, the system serif.
    static func display(_ size: CGFloat, weight: Font.Weight = .regular) -> Font {
        .system(size: size, weight: weight, design: .serif)
    }

    static let pageTitle = display(30)
    static let sectionTitle = Font.system(size: 15, weight: .semibold)
    static let caption = Font.system(size: 11)
    static let label = Font.system(size: 11, weight: .medium)

    static let gutter: CGFloat = 32
    static let radius: CGFloat = 4
    static let hairline = Color.primary.opacity(0.08)
    static let placeholder = Color.primary.opacity(0.06)
}

extension Color {
    /// "#rrggbb" or "rrggbb".
    init?(hex: String) {
        var s = hex.trimmingCharacters(in: .whitespaces)
        if s.hasPrefix("#") { s.removeFirst() }
        guard s.count == 6, let v = UInt64(s, radix: 16) else { return nil }
        self.init(red: Double((v >> 16) & 0xff) / 255, green: Double((v >> 8) & 0xff) / 255, blue: Double(v & 0xff) / 255)
    }
}

/// A thin divider line.
struct Hairline: View {
    var body: some View {
        Rectangle().fill(Theme.hairline).frame(height: 1)
    }
}

/// A small uppercase label above a group of content.
struct Eyebrow: View {
    let text: String
    init(_ text: String) { self.text = text }
    var body: some View {
        Text(text.uppercased())
            .font(.system(size: 10, weight: .semibold))
            .tracking(0.8)
            .foregroundStyle(.secondary)
    }
}
