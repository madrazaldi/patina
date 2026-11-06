// Port of web/src/components/StarRating.svelte: your rating, 1–5 stars (Plex stores 0–10).
// Tapping the current value clears it; dragging across the stars scrubs.

import SwiftUI

struct StarRating: View {
    /// 0–10.
    var value: Double
    var onRate: (Double?) -> Void

    @Environment(AppStore.self) private var store
    @State private var dragging: Int?

    private var stars: Int { Int((value / 2).rounded()) }

    var body: some View {
        let shown = dragging ?? stars
        HStack(spacing: 4) {
            ForEach(1...5, id: \.self) { n in
                Image(systemName: n <= shown ? "star.fill" : "star")
                    .font(.subheadline)
                    .foregroundStyle(n <= shown ? store.accent : Theme.tertiaryText)
                    .frame(width: 26, height: 30)
                    .contentShape(.rect)
                    .onTapGesture {
                        Haptics.selection()
                        onRate(n == stars ? nil : Double(n * 2))
                    }
            }
        }
        .gesture(
            DragGesture(minimumDistance: 6)
                .onChanged { g in
                    let n = min(5, max(1, Int(g.location.x / 30) + 1))
                    if n != dragging { Haptics.selection() }
                    dragging = n
                }
                .onEnded { _ in
                    if let n = dragging, n != stars { onRate(Double(n * 2)) }
                    dragging = nil
                }
        )
        .accessibilityElement()
        .accessibilityLabel("Your rating")
        .accessibilityValue(stars == 0 ? "Not rated" : "\(stars) of 5 stars")
        .accessibilityAdjustableAction { direction in
            switch direction {
            case .increment: onRate(Double(min(5, stars + 1) * 2))
            case .decrement: onRate(stars <= 1 ? nil : Double((stars - 1) * 2))
            @unknown default: break
            }
        }
    }
}

#Preview {
    StarRating(value: 6) { _ in }
        .padding()
        .environment(AppStore.preview())
        .preferredColorScheme(.dark)
}
