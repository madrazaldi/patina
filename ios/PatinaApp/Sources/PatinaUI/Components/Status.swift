// Port of web/src/components/Status.svelte: the loading, empty and error states of a
// page, plus the skeletons shown while content loads and a small loader for one value
// (web/src/lib/resource.svelte.ts).

import PatinaCore
import SwiftUI

/// What a page says when it has nothing to show.
public struct StatusView: View {
    var title: String?
    var text: String?
    var error: String?
    var systemImage: String?
    var retry: (() -> Void)?

    public init(
        title: String? = nil, text: String? = nil, error: String? = nil, systemImage: String? = nil,
        retry: (() -> Void)? = nil
    ) {
        self.title = title
        self.text = text
        self.error = error
        self.systemImage = systemImage
        self.retry = retry
    }

    public var body: some View {
        ContentUnavailableView {
            Label(error != nil ? "Something went wrong" : (title ?? ""), systemImage: error != nil ? "exclamationmark.triangle" : (systemImage ?? "sparkles.tv"))
        } description: {
            if let error {
                Text(error).textSelection(.enabled)
            } else if let text {
                Text(text)
            }
        } actions: {
            if let retry {
                Button("Try Again", action: retry)
                    .buttonStyle(.bordered)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 40)
    }
}

/// A slim banner over stale content when refreshing it failed.
struct StaleBanner: View {
    var error: String
    var retry: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(Theme.danger)
            Text(error).font(.footnote).foregroundStyle(Theme.secondaryText).lineLimit(2)
            Spacer()
            Button("Retry", action: retry).font(.footnote.weight(.semibold))
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .panel(cornerRadius: 12)
    }
}

/// A shimmering placeholder block.
struct Skeleton: View {
    var cornerRadius: CGFloat = Theme.cornerRadius
    @State private var phase: CGFloat = -1

    var body: some View {
        RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
            .fill(Theme.raised)
            .overlay {
                GeometryReader { proxy in
                    LinearGradient(
                        colors: [.clear, .white.opacity(0.06), .clear], startPoint: .leading, endPoint: .trailing
                    )
                    .frame(width: proxy.size.width * 0.8)
                    .offset(x: phase * proxy.size.width * 1.4)
                }
                .clipShape(.rect(cornerRadius: cornerRadius, style: .continuous))
            }
            .onAppear {
                withAnimation(.linear(duration: 1.3).repeatForever(autoreverses: false)) { phase = 1 }
            }
            .accessibilityHidden(true)
    }
}

/// A skeleton of a card: the artwork and two lines.
struct SkeletonCard: View {
    var shape: CardShape = .poster

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Skeleton().aspectRatio(shape.aspect, contentMode: .fit)
            Skeleton(cornerRadius: 4).frame(height: 11).padding(.trailing, 24)
            Skeleton(cornerRadius: 4).frame(width: 48, height: 9)
        }
    }
}

/// A row of skeleton cards under a skeleton heading.
struct SkeletonRow: View {
    var shape: CardShape = .poster
    var width: CGFloat

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Skeleton(cornerRadius: 5).frame(width: 160, height: 18)
            HStack(alignment: .top, spacing: 14) {
                ForEach(0..<8, id: \.self) { _ in SkeletonCard(shape: shape).frame(width: width) }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .clipped()
    }
}

/// One value loaded from the core, refetched on demand (`resource()`): the last good value
/// stays while a reload runs, and its error shows beside it.
@MainActor
@Observable
final class Loader<T> {
    private(set) var value: T?
    private(set) var error: String?
    private(set) var loading = false
    /// The revision the value was loaded at (see `loadIfStale`).
    @ObservationIgnored private(set) var revision = -1
    @ObservationIgnored private var generation = 0
    @ObservationIgnored private var fetch: (() async throws -> T)?

    init() {}

    /// Fetch with `fetch` (kept for `reload`).
    func load(_ fetch: @escaping () async throws -> T) async {
        self.fetch = fetch
        await run()
    }

    /// Fetch unless the value is from `revision` already (views come back without refetching).
    func load(at revision: Int, _ fetch: @escaping () async throws -> T) async {
        if value != nil && self.revision == revision { return }
        self.revision = revision
        await load(fetch)
    }

    func refresh() async {
        await run()
    }

    func reload() {
        Task { await run() }
    }

    private func run() async {
        guard let fetch else { return }
        generation += 1
        let mine = generation
        loading = true
        do {
            let v = try await fetch()
            guard mine == generation else { return }
            value = v
            error = nil
        } catch is CancellationError {
        } catch {
            guard mine == generation else { return }
            self.error = errorText(error)
        }
        if mine == generation { loading = false }
    }

    /// Replace the value without fetching (optimistic edits).
    func set(_ v: T?) {
        value = v
    }
}

#Preview("States") {
    ScrollView {
        StatusView(title: "Nothing here yet", text: "Your libraries are empty or still scanning.")
        StatusView(error: "The server did not answer.") {}
        SkeletonRow(width: 120).padding()
    }
    .preferredColorScheme(.dark)
}
