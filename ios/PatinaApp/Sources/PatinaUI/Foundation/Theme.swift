// The look of the app: dark only, neutral surfaces, one accent (the user's `ui.accent`).
// The iOS counterpart of the tokens in web/src/app.css.

import SwiftUI

public enum Theme {
    /// The brand green of the app icon; `ui.accent` overrides it.
    public static let defaultAccent = Color(red: 0.498, green: 0.702, blue: 0.627)

    public static let background = Color(white: 0.055)
    public static let surface = Color(white: 0.095)
    public static let raised = Color(white: 0.14)
    public static let hairline = Color.white.opacity(0.08)
    public static let secondaryText = Color.white.opacity(0.62)
    public static let tertiaryText = Color.white.opacity(0.4)
    public static let danger = Color(red: 0.95, green: 0.42, blue: 0.4)

    /// Poster corners.
    public static let cornerRadius: CGFloat = 10
    /// Horizontal page margin, by size class.
    public static func margin(_ sizeClass: UserInterfaceSizeClass?) -> CGFloat {
        sizeClass == .regular ? 28 : 16
    }

    /// A colour from `#rrggbb` (the format `ui.accent` is saved in).
    public static func color(hex: String) -> Color? {
        var s = hex.trimmingCharacters(in: .whitespaces)
        if s.hasPrefix("#") { s.removeFirst() }
        guard s.count == 6, let v = UInt32(s, radix: 16) else { return nil }
        return Color(
            red: Double((v >> 16) & 0xFF) / 255,
            green: Double((v >> 8) & 0xFF) / 255,
            blue: Double(v & 0xFF) / 255
        )
    }
}

extension View {
    /// A rounded, hairline-bordered panel on the surface colour.
    public func panel(cornerRadius: CGFloat = 14) -> some View {
        background(Theme.surface, in: RoundedRectangle(cornerRadius: cornerRadius, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: cornerRadius, style: .continuous).strokeBorder(Theme.hairline))
    }
}
