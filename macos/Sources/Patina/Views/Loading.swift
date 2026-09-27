import SwiftUI

/// The state of something being fetched.
enum Phase<T> {
    case loading
    case failed(String)
    case loaded(T)

    var value: T? {
        if case .loaded(let v) = self { return v }
        return nil
    }
}

/// Fetches with `load` whenever `id` changes (pass `app.revision` to refetch
/// when watch state changes) and shows the result, a spinner, or the error
/// with a Retry button. A refetch keeps showing the previous result.
struct Loading<T, Content: View>: View {
    var id: AnyHashable
    let load: () async throws -> T
    @ViewBuilder let content: (T) -> Content

    @State private var phase: Phase<T> = .loading
    @State private var attempt = 0

    init(id: AnyHashable = 0, _ load: @escaping () async throws -> T, @ViewBuilder content: @escaping (T) -> Content) {
        self.id = id
        self.load = load
        self.content = content
    }

    var body: some View {
        Group {
            switch phase {
            case .loading:
                ProgressView().controlSize(.small).frame(maxWidth: .infinity, minHeight: 200)
            case .failed(let message):
                ErrorState(message: message) { attempt += 1 }
            case .loaded(let value):
                content(value)
            }
        }
        .task(id: [id, AnyHashable(attempt)]) {
            do {
                let value = try await load()
                // Backend calls don't observe cancellation; drop a superseded result.
                guard !Task.isCancelled else { return }
                phase = .loaded(value)
            } catch is CancellationError {
            } catch {
                guard !Task.isCancelled else { return }
                // Keep the previous result on a failed refresh.
                if phase.value == nil { phase = .failed(error.localizedDescription) }
                else { AppModel.shared.error(error) }
            }
        }
    }
}

struct ErrorState: View {
    let message: String
    var retry: (() -> Void)? = nil

    var body: some View {
        VStack(spacing: 10) {
            Text("Couldn't load this").font(Theme.display(17))
            Text(message).font(.callout).foregroundStyle(.secondary).multilineTextAlignment(.center).textSelection(.enabled)
            if let retry { Button("Retry", action: retry) }
        }
        .frame(maxWidth: 420)
        .frame(maxWidth: .infinity, minHeight: 240)
    }
}

struct EmptyState: View {
    let title: String
    var message: String? = nil

    var body: some View {
        VStack(spacing: 6) {
            Text(title).font(Theme.display(17))
            if let message { Text(message).font(.callout).foregroundStyle(.secondary).multilineTextAlignment(.center) }
        }
        .frame(maxWidth: 420)
        .frame(maxWidth: .infinity, minHeight: 240)
    }
}

/// A page's serif title with an optional line beneath and trailing controls.
struct PageHeader<Trailing: View>: View {
    let title: String
    var subtitle: String? = nil
    @ViewBuilder var trailing: () -> Trailing

    var body: some View {
        HStack(alignment: .lastTextBaseline) {
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(Theme.pageTitle).lineLimit(2)
                if let subtitle { Text(subtitle).font(.callout).foregroundStyle(.secondary) }
            }
            Spacer(minLength: 16)
            trailing()
        }
        .padding(.horizontal, Theme.gutter)
        .padding(.top, 28)
        .padding(.bottom, 18)
    }
}

extension PageHeader where Trailing == EmptyView {
    init(_ title: String, subtitle: String? = nil) {
        self.init(title: title, subtitle: subtitle, trailing: { EmptyView() })
    }
}
