import SwiftUI

/// The Library tab: filter bar, the paged grid, and the A–Z rail.
struct LibraryBrowse<Header: View>: View {
    @Bindable var model: LibraryModel
    @ViewBuilder var header: () -> Header

    @Environment(AppModel.self) private var app
    @State private var restored = false

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    header().id("top")
                    LibraryFilterBar(model: model) {
                        withAnimation(nil) { proxy.scrollTo("top", anchor: .top) }
                    }
                    content
                }
                .padding(.bottom, 32)
            }
            .overlay(alignment: .topTrailing) {
                if model.railOn && !model.chars.isEmpty && (model.pages.total ?? 0) > 0 {
                    LetterRail(starts: model.starts) { index in jump(to: index, proxy) }
                        .padding(.top, 150)
                        .padding(.trailing, 6)
                }
            }
            .onAppear {
                // Freshly created (e.g. coming back from another sidebar place): restore the position.
                guard !restored else { return }
                restored = true
                if let focus = model.focus, focus > 0 {
                    Task { @MainActor in
                        try? await Task.sleep(for: .milliseconds(60))
                        proxy.scrollTo(focus, anchor: .top)
                    }
                }
            }
            .onChange(of: model.requestKey) { _, _ in
                proxy.scrollTo("top", anchor: .top)
            }
        }
        .safeAreaInset(edge: .bottom, spacing: 0) {
            if model.selecting { LibraryBulkBar(model: model) }
        }
        .task { await model.loadSorts() }
        .task(id: model.mediaType) { await model.loadFilters() }
        .task(id: "\(model.mediaType ?? 0)|\(model.railOn)|\(app.revision)") { await model.loadChars() }
        .task(id: "\(model.requestKey)|\(app.revision)") { model.configurePages(revision: app.revision) }
        .onDisappear { model.pages.detach() }
    }

    @ViewBuilder private var content: some View {
        let pages = model.pages
        if let error = pages.error, (pages.total ?? 0) > 0 {
            StaleNotice(message: error) { pages.refresh() }
        }
        if pages.total == 0 && pages.loading == 0 {
            EmptyState(title: model.filtered ? "No matches" : "Nothing here yet",
                       message: model.filtered ? "Try a different filter." : nil)
        } else if pages.total == nil {
            if let error = pages.error {
                ErrorState(message: error) { pages.refresh() }
            } else {
                ProgressView().controlSize(.small).frame(maxWidth: .infinity, minHeight: 240)
            }
        } else {
            LibraryGrid(model: model)
                .padding(.trailing, model.railOn && !model.chars.isEmpty ? 14 : 0)
        }
    }

    private func jump(to index: Int, _ proxy: ScrollViewProxy) {
        model.pages.ensure(index, index + LibraryPages.size)
        proxy.scrollTo(index, anchor: .top)
    }
}

/// The poster grid over a sparse listing; unloaded cells are placeholders
/// that load their page when they appear.
private struct LibraryGrid: View {
    let model: LibraryModel
    @Environment(AppModel.self) private var app

    private var shape: CardShape { model.shape ?? .poster }

    private var minWidth: CGFloat {
        switch shape {
        case .landscape: app.posterWidth * 1.75
        case .square: app.posterWidth * 1.05
        case .poster: app.posterWidth
        }
    }

    var body: some View {
        let total = model.pages.total ?? 0
        LazyVGrid(
            columns: [GridItem(.adaptive(minimum: minWidth, maximum: minWidth * 1.4), spacing: 18, alignment: .top)],
            alignment: .leading, spacing: 22
        ) {
            ForEach(0..<total, id: \.self) { i in
                cell(i)
                    .id(i)
                    .onAppear { model.appeared(i) }
                    .onDisappear { model.disappeared(i) }
            }
        }
        .padding(.horizontal, Theme.gutter)
    }

    @ViewBuilder private func cell(_ i: Int) -> some View {
        if let item = model.pages.at(i) {
            Card(item: item, shape: model.shape, selection: model.selecting ? selection(item) : nil)
        } else {
            VStack(alignment: .leading, spacing: 6) {
                Theme.placeholder
                    .aspectRatio(1 / shape.ratio, contentMode: .fit)
                    .clipShape(RoundedRectangle(cornerRadius: Theme.radius))
                VStack(alignment: .leading, spacing: 1) {
                    Text(" ").font(.system(size: 12, weight: .medium))
                    Text(" ").font(Theme.caption)
                }
            }
            .accessibilityHidden(true)
        }
    }

    private func selection(_ item: Metadata) -> Binding<Bool> {
        Binding(get: { model.isSelected(item) }, set: { model.setSelected(item, $0) })
    }
}

/// Jump to the first item under a letter.
private struct LetterRail: View {
    let starts: [String: Int]
    let jump: (Int) -> Void

    var body: some View {
        VStack(spacing: 0) {
            ForEach(LibraryModel.letters, id: \.self) { letter in
                let at = starts[letter]
                Button {
                    if let at { jump(at) }
                } label: {
                    Text(letter)
                        .font(.system(size: 10.5, design: .monospaced))
                        .frame(width: 18, height: 17)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .foregroundStyle(at == nil ? .tertiary : .secondary)
                .disabled(at == nil)
                .help(at == nil ? "" : "Jump to \(letter)")
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Jump to letter")
    }
}

/// A failed refresh over results that are still shown.
private struct StaleNotice: View {
    let message: String
    let retry: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: "exclamationmark.triangle").foregroundStyle(.secondary)
            Text("Couldn't refresh: \(message)").font(.callout).foregroundStyle(.secondary).lineLimit(2)
            Button("Retry", action: retry).buttonStyle(.link)
            Spacer()
        }
        .padding(.horizontal, Theme.gutter)
        .padding(.bottom, 14)
    }
}
