// Ports of web/src/pages/Requests.svelte and SeerrTitle.svelte, with the components they use
// (SeerrCard, SeerrRequestRow, SeerrRequestDialog) and web/src/lib/seerr.ts: discover titles
// to request from Seerr, follow (or manage) requests, and one title's page. Unlike the web,
// where Settings connects an instance, the Requests page offers the connect form itself.

import PatinaCore
import PlexKit
import SeerrKit
import SwiftUI

// MARK: Labels and rules (web/src/lib/seerr.ts)

enum SeerrUI {
    static func availabilityLabel(_ a: Availability?) -> String? {
        switch a {
        case .pending: "Requested"
        case .processing: "Processing"
        case .partial: "Partly available"
        case .available: "Available"
        case .blocklisted: "Blocklisted"
        case .deleted: "Deleted"
        case .unknown, nil: nil
        }
    }

    static func requestLabel(_ s: RequestState) -> String {
        switch s {
        case .pending: "Pending approval"
        case .approved: "Approved"
        case .declined: "Declined"
        case .failed: "Failed"
        case .completed: "Completed"
        }
    }

    /// Nothing has been requested or added yet.
    static func isNew(_ a: Availability?) -> Bool { a == nil || a == .unknown || a == .deleted }

    static func canRequestKind(_ can: SeerrCan, _ mediaType: SeerrMediaType, _ is4k: Bool) -> Bool {
        mediaType == .movie ? (is4k ? can.request4kMovie : can.requestMovie) : (is4k ? can.request4kTv : can.requestTv)
    }

    /// Seasons of a show that can still be requested in this quality.
    static func openSeasons(_ d: SeerrDetails, _ is4k: Bool) -> [SeerrSeason] {
        d.seasons.filter { s in
            let status = is4k ? s.status4k : s.status
            let requested = is4k ? s.requested4k : s.requested
            // Seerr keeps a failed request's seasons until it is retried or deleted.
            return status != .available && status != .partial && status != .processing && requested != .pending
                && requested != .approved && requested != .failed
        }
    }

    /// Whether a request button makes sense for this title in this quality.
    static func requestable(_ d: SeerrDetails, _ can: SeerrCan, _ is4k: Bool) -> Bool {
        if !canRequestKind(can, d.mediaType, is4k) { return false }
        if d.mediaType == .tv { return !openSeasons(d, is4k).isEmpty }
        return isNew(is4k ? d.status4k : d.status)
    }

    // The status (user and permissions) is fetched once per instance and account (a Plex Home
    // switch signs in to Seerr as someone else) and shared by the pages.
    private static var cached: (key: String, task: Task<SeerrStatus, any Error>)?

    static func status(_ store: AppStore) async throws -> SeerrStatus {
        let key = "\(Commands.seerrConfig(store.app)?.url ?? "")|\(store.scope)"
        if cached?.key != key {
            let app = store.app!
            cached = (key, Task { try await Commands.seerrStatus(app) })
        }
        let task = cached!.task
        do {
            return try await task.value
        } catch {
            // A failure is not remembered: the next page asks again.
            if cached?.key == key { cached = nil }
            throw error
        }
    }

    static func forgetStatus() { cached = nil }
}

// MARK: Requests

struct RequestsScreen: View {
    enum Tab: Hashable {
        case discover, requests
    }

    static let lists = [
        ("trending", "Trending"), ("movies", "Popular Movies"), ("tv", "Popular Shows"),
        ("upcoming-movies", "Upcoming Movies"), ("upcoming-tv", "Upcoming Shows"),
    ]
    static let filters = [
        ("all", "All"), ("pending", "Pending Approval"), ("approved", "Approved"), ("processing", "Processing"),
        ("available", "Available"), ("failed", "Failed"),
    ]

    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var tab = Tab.discover
    @State private var status: SeerrStatus?
    @State private var statusError: String?

    // Discover pages accumulate as the grid scrolls; switching lists starts over.
    @State private var list = "trending"
    @State private var titles: [SeerrTitle] = []
    @State private var shown = (list: "", query: "", page: 0, pages: 0)
    @State private var loading = false
    @State private var discoverError: String?
    @State private var generation = 0
    @State private var query = ""

    @State private var filter = "all"
    @State private var page = 1
    @State private var requests = Loader<SeerrPage<SeerrRequest>>()

    var body: some View {
        Group {
            if !store.seerr {
                SeerrConnectForm()
            } else {
                ScrollView {
                    VStack(alignment: .leading, spacing: 14) {
                        Text(subtitle)
                            .font(.subheadline).foregroundStyle(Theme.secondaryText)
                        PillTabs(
                            selection: $tab,
                            options: [(.discover, "Discover"), (.requests, status?.can.manage == true ? "All Requests" : "My Requests")])
                        if let statusError { StaleBanner(error: statusError) { loadStatus() } }
                        switch tab {
                        case .discover: discover
                        case .requests: requestList
                        }
                    }
                    .padding(.horizontal, Theme.margin(sizeClass))
                    .padding(.bottom, 24)
                }
                .searchable(text: $query, prompt: "Search Seerr")
                .onSubmit(of: .search) { if tab == .discover { load(1) } }
                .onChange(of: query) { if query.isEmpty && !shown.query.isEmpty { load(1) } }
                .toolbar {
                    ToolbarItem(placement: .topBarTrailing) {
                        Menu {
                            Button("Disconnect Seerr", systemImage: "xmark.circle", role: .destructive) { disconnect() }
                        } label: {
                            Image(systemName: "ellipsis.circle")
                        }
                    }
                }
                .refreshable {
                    if tab == .discover { load(1) } else { await requests.refresh() }
                }
            }
        }
        .navigationTitle("Requests")
        .task(id: "\(store.seerr):\(store.scope)") { if store.seerr { loadStatus() } }
        .task(id: "\(store.seerr):\(list)") {
            if store.seerr && shown.list != list { load(1) }
        }
        .task(id: "\(store.seerr):\(tab):\(filter):\(page)") {
            guard store.seerr, tab == .requests else { return }
            let (f, p, app) = (filter, page, store.app!)
            await requests.load { try await Commands.seerrRequests(app, filter: f, page: p) }
            // Deleting the last request of the last page leaves it empty: step back.
            if let r = requests.value, r.page == page, r.results.isEmpty, page > 1 { page = max(1, min(page - 1, r.pages)) }
        }
    }

    private var subtitle: String {
        if let status { return "Seerr at \(status.url) · signed in as \(status.user.name)" }
        if let c = Commands.seerrConfig(store.app) { return "Seerr at \(c.url)" }
        return "Request movies and shows the server doesn't have."
    }

    private func loadStatus() {
        status = nil
        statusError = nil
        Task {
            do { status = try await SeerrUI.status(store) } catch { statusError = errorText(error) }
        }
    }

    private func disconnect() {
        Task {
            guard await store.confirm("Disconnect Seerr?", text: "Requests made so far stay on the Seerr server.", action: "Disconnect", danger: true) else { return }
            await store.act { try Commands.seerrDisconnect(store.app) }
            SeerrUI.forgetStatus()
            store.reloadSettings()
        }
    }

    // MARK: Discover

    private func load(_ page: Int) {
        generation += 1
        let mine = generation
        let (which, q, app) = (list, query.trimmingCharacters(in: .whitespaces), store.app!)
        loading = true
        discoverError = nil
        Task {
            do {
                let got =
                    q.isEmpty
                    ? try await Commands.seerrDiscover(app, list: which, page: page)
                    : try await Commands.seerrSearch(app, query: q, page: page)
                guard mine == generation else { return }
                // TMDB pages can overlap as popularity shifts.
                let seen = Set(page == 1 ? [] : titles.map { "\($0.mediaType):\($0.tmdbId)" })
                let fresh = got.results.filter { !seen.contains("\($0.mediaType):\($0.tmdbId)") }
                titles = page == 1 ? fresh : titles + fresh
                shown = (which, q, got.page, got.pages)
            } catch {
                if mine == generation { discoverError = errorText(error) }
            }
            if mine == generation { loading = false }
        }
    }

    @ViewBuilder
    private var discover: some View {
        if shown.query.isEmpty {
            Picker("List", selection: $list) {
                ForEach(Self.lists, id: \.0) { Text($0.1).tag($0.0) }
            }
            .pickerStyle(.menu)
            .tint(.primary)
        } else {
            Text("Results for “\(shown.query)”").font(.headline)
        }
        if let discoverError, titles.isEmpty {
            StatusView(error: discoverError) { load(1) }
        } else if titles.isEmpty {
            if loading {
                LazyVGrid(columns: cardColumns(.poster, sizeClass), spacing: 22) {
                    ForEach(0..<8, id: \.self) { _ in SkeletonCard(shape: .poster) }
                }
            } else {
                StatusView(title: "Nothing here", text: "Seerr returned no titles for this list.", systemImage: "tray")
            }
        } else {
            LazyVGrid(columns: cardColumns(.poster, sizeClass), spacing: 22) {
                ForEach(titles, id: \.key) { t in
                    SeerrCard(title: t)
                        .onAppear {
                            if t.key == titles.last?.key, !loading, shown.page < shown.pages { load(shown.page + 1) }
                        }
                }
            }
            if let discoverError { Text(discoverError).font(.footnote).foregroundStyle(Theme.danger) }
            if loading { ProgressView().frame(maxWidth: .infinity) }
        }
    }

    // MARK: Requests

    @ViewBuilder
    private var requestList: some View {
        Picker("Show", selection: $filter) {
            ForEach(Self.filters, id: \.0) { Text($0.1).tag($0.0) }
        }
        .pickerStyle(.menu)
        .tint(.primary)
        .onChange(of: filter) { page = 1 }
        if let r = requests.value {
            if let error = requests.error { StaleBanner(error: error) { requests.reload() } }
            if r.results.isEmpty {
                StatusView(
                    title: "No requests",
                    text: filter == "all"
                        ? "Requests made in Patina or on the Seerr website show up here." : "No requests match this filter.",
                    systemImage: "tray")
            } else {
                VStack(spacing: 0) {
                    ForEach(r.results, id: \.id) { req in
                        SeerrRequestRow(request: req, can: status?.can) { requests.reload() }
                        Divider().opacity(0.5)
                    }
                }
                if r.pages > 1 {
                    HStack {
                        Button("Newer", systemImage: "chevron.left") { page -= 1 }.disabled(page <= 1)
                        Spacer()
                        Text("Page \(r.page) of \(r.pages)").monospacedDigit().foregroundStyle(Theme.secondaryText)
                        Spacer()
                        Button("Older", systemImage: "chevron.right") { page += 1 }.disabled(page >= r.pages)
                            .labelStyle(TrailingIconLabelStyle())
                    }
                    .font(.subheadline)
                }
            }
        } else if let error = requests.error {
            StatusView(error: error) { requests.reload() }
        } else {
            ProgressView().frame(maxWidth: .infinity).padding(.top, 40)
        }
    }
}

extension SeerrTitle {
    fileprivate var key: String { "\(mediaType):\(tmdbId)" }
}

/// Connect a Seerr, Overseerr or Jellyseerr server (the Seerr section of web Settings).
struct SeerrConnectForm: View {
    @Environment(AppStore.self) private var store
    @State private var url = ""
    @State private var apiKey = ""
    @State private var busy = false
    @State private var error: String?

    var body: some View {
        Form {
            Section {
                ContentUnavailableView(
                    "Seerr is not set up", systemImage: "tray",
                    description: Text("Connect a Seerr, Overseerr or Jellyseerr server to request movies and shows."))
                    .listRowBackground(Color.clear)
            }
            Section {
                TextField("https://seerr.example.com", text: $url)
                    .keyboardType(.URL).textContentType(.URL)
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
                SecureField("API key (optional)", text: $apiKey)
                    .textInputAutocapitalization(.never).autocorrectionDisabled()
            } header: {
                Text("Server")
            } footer: {
                Text("Leave the API key empty to sign in with your Plex account.")
            }
            if let error {
                Section { Text(error).foregroundStyle(Theme.danger) }
            }
            Section {
                Button {
                    connect()
                } label: {
                    HStack {
                        Text(busy ? "Connecting…" : "Connect")
                        if busy {
                            Spacer()
                            ProgressView()
                        }
                    }
                }
                .disabled(busy || url.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
    }

    private func connect() {
        busy = true
        error = nil
        let (u, k) = (url.trimmingCharacters(in: .whitespaces), apiKey.trimmingCharacters(in: .whitespaces))
        Task {
            do {
                let s = try await Commands.seerrConnect(store.app, url: u, apiKey: k)
                SeerrUI.forgetStatus()
                store.reloadSettings()
                store.toast("Connected to Seerr as \(s.user.name)")
            } catch {
                self.error = errorText(error)
            }
            busy = false
        }
    }
}

/// Poster card for a Seerr title (SeerrCard.svelte); opens its Seerr page.
struct SeerrCard: View {
    let title: SeerrTitle
    @Environment(AppStore.self) private var store

    var body: some View {
        let t = title
        let label = SeerrUI.availabilityLabel(t.status)
        let kind = t.mediaType == .tv ? "Show" : "Movie"
        Button {
            store.navigate(to: .seerr(mediaType: t.mediaType, tmdbId: t.tmdbId))
        } label: {
            VStack(alignment: .leading, spacing: 4) {
                Artwork(t.poster, width: 300, height: 450, fallback: t.title)
                    .aspectRatio(2 / 3, contentMode: .fit)
                    .clipShape(.rect(cornerRadius: Theme.cornerRadius, style: .continuous))
                    .overlay(alignment: .topTrailing) {
                        if t.status == .available {
                            Image(systemName: "checkmark")
                                .font(.caption2.weight(.bold)).foregroundStyle(.black)
                                .padding(5).background(store.accent, in: .circle).padding(6)
                        } else if let label {
                            Text(label).font(.caption2.weight(.semibold))
                                .foregroundStyle(t.status == .partial ? store.accent : .primary)
                                .padding(.horizontal, 7).padding(.vertical, 3)
                                .background(.ultraThinMaterial, in: .capsule).padding(6)
                        }
                    }
                Text(t.title).font(.subheadline.weight(.medium)).lineLimit(1)
                Text([t.year.map(String.init), kind].compactMap { $0 }.joined(separator: " · "))
                    .font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(1)
            }
        }
        .buttonStyle(CardButtonStyle())
        .accessibilityLabel("\(t.title)\(t.year.map { " (\($0))" } ?? ""), \(kind)\(label.map { ", \($0)" } ?? "")")
    }
}

/// One request, with what the user may do to it (SeerrRequestRow.svelte): approve or decline
/// (managers), retry a failed one, or withdraw it.
struct SeerrRequestRow: View {
    let request: SeerrRequest
    let can: SeerrCan?
    var linked = true
    let onChange: () -> Void

    @Environment(AppStore.self) private var store
    @State private var busy = false

    init(request: SeerrRequest, can: SeerrCan?, linked: Bool = true, onChange: @escaping () -> Void) {
        self.request = request
        self.can = can
        self.linked = linked
        self.onChange = onChange
    }

    private var manage: Bool { can?.manage ?? false }
    private var withdraw: Bool { manage || (request.mine && request.state == .pending) }

    private var details: String {
        let r = request
        let what =
            r.mediaType == .tv
            ? (r.seasons.isEmpty
                ? "Show" : "Season\(r.seasons.count == 1 ? "" : "s") \(r.seasons.map(String.init).joined(separator: ", "))")
            : "Movie"
        let when = r.createdAt.flatMap { try? Date($0, strategy: .iso8601) }?.formatted(date: .abbreviated, time: .omitted)
        return [what, r.is4k ? "4K" : nil, r.requestedBy, when].compactMap { $0 }.filter { !$0.isEmpty }
            .joined(separator: " · ")
    }

    /// Once approved, how far the title got says more than the request itself.
    private var shown: String {
        let r = request
        if r.state == .approved || r.state == .completed, [.processing, .partial, .available].contains(r.status),
            let l = SeerrUI.availabilityLabel(r.status)
        {
            return l
        }
        return SeerrUI.requestLabel(r.state)
    }

    var body: some View {
        let r = request
        HStack(spacing: 12) {
            Artwork(r.poster, width: 96, height: 144)
                .frame(width: 40, height: 60)
                .clipShape(.rect(cornerRadius: 6))
            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 4) {
                    Text(r.title).font(.subheadline.weight(.semibold)).lineLimit(1)
                    if let y = r.year { Text("(\(String(y)))").font(.subheadline).foregroundStyle(Theme.secondaryText) }
                }
                Text(details).font(.caption).foregroundStyle(Theme.secondaryText).lineLimit(2)
                Badge(shown, accent: r.status == .available && r.state != .declined)
                    .foregroundStyle(r.state == .failed || r.state == .declined ? Theme.danger : Theme.secondaryText)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if manage && r.state == .pending {
                icon("checkmark", "Approve") { run("approve", "Request approved") }
                icon("xmark", "Decline") { run("decline", "Request declined") }
            }
            if manage && r.state == .failed { icon("arrow.counterclockwise", "Retry") { run("retry", "Sent again") } }
            if withdraw { icon("trash", "Delete Request") { run("delete", "Request deleted") } }
        }
        .padding(.vertical, 8)
        .contentShape(.rect)
        .onTapGesture { if linked { store.navigate(to: .seerr(mediaType: r.mediaType, tmdbId: r.tmdbId)) } }
        .disabled(busy)
    }

    private func icon(_ name: String, _ label: String, _ action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: name).frame(width: 32, height: 32)
        }
        .buttonStyle(.borderless)
        .accessibilityLabel(label)
    }

    private func run(_ action: String, _ done: String) {
        let r = request
        Task {
            if action == "delete" || action == "decline" {
                let ok = await store.confirm(
                    action == "delete" ? "Delete the request for “\(r.title)”?" : "Decline “\(r.title)”?",
                    text: action == "delete" ? "The request is removed from Seerr." : "\(r.requestedBy) is told it was declined.",
                    action: action == "delete" ? "Delete" : "Decline", danger: true)
                if !ok { return }
            }
            busy = true
            if await store.act(done, { try await Commands.seerrRequestAction(store.app, id: r.id, action: action) }) {
                onChange()
            }
            busy = false
        }
    }
}

// MARK: A title

/// A movie or show as Seerr knows it (SeerrTitle.svelte): request it (or more seasons),
/// follow its requests, or open the copy on the Plex server once it is there.
struct SeerrTitleScreen: View {
    let mediaType: SeerrMediaType
    let tmdbId: Int

    @Environment(AppStore.self) private var store
    @Environment(\.horizontalSizeClass) private var sizeClass
    @State private var data = Loader<SeerrDetails>()
    @State private var status: SeerrStatus?
    @State private var asking: Asking?

    struct Asking: Identifiable {
        var is4k: Bool
        var id: Bool { is4k }
    }

    var body: some View {
        Group {
            if let d = data.value {
                content(d)
            } else if let error = data.error {
                StatusView(error: error) { data.reload() }
            } else {
                ProgressView().frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .navigationTitle(data.value?.title ?? (mediaType == .tv ? "Show" : "Movie"))
        .navigationBarTitleDisplayMode(.inline)
        .refreshable { await data.refresh() }
        .task { await data.load { try await Commands.seerrDetails(store.app, mediaType: mediaType, tmdbId: tmdbId) } }
        .task { status = try? await SeerrUI.status(store) }
        .sheet(item: $asking) { a in
            if let d = data.value, let can = status?.can {
                SeerrRequestSheet(details: d, is4k: a.is4k, can: can) { data.reload() }
            }
        }
    }

    private func content(_ d: SeerrDetails) -> some View {
        let label = SeerrUI.availabilityLabel(d.status)
        let label4k = SeerrUI.availabilityLabel(d.status4k)
        let can = status?.can
        let meta: [String] =
            [
                d.rating.map { "★ \(String(format: "%.1f", $0))" }, d.year.map(String.init),
                d.runtime.map { Format.duration(ms: Int64($0) * 60000) },
                d.mediaType == .tv ? Format.plural(d.seasons.count, "season") : nil,
            ].compactMap { $0 } + d.genres.prefix(3)
        return ScrollView {
            VStack(alignment: .leading, spacing: 18) {
                ZStack(alignment: .bottomLeading) {
                    Artwork(d.backdrop, width: 1280, height: 720)
                        .aspectRatio(16 / 9, contentMode: .fill)
                        .frame(maxWidth: .infinity, maxHeight: 280)
                        .clipped()
                        .overlay(LinearGradient(colors: [.clear, Theme.background], startPoint: .center, endPoint: .bottom))
                    HStack(alignment: .bottom, spacing: 16) {
                        Artwork(d.poster, width: 500, height: 750, fallback: d.title)
                            .frame(width: 110, height: 165)
                            .clipShape(.rect(cornerRadius: Theme.cornerRadius, style: .continuous))
                            .shadow(radius: 12)
                        VStack(alignment: .leading, spacing: 4) {
                            Text("\(d.mediaType == .tv ? "Show" : "Movie") · Seerr")
                                .font(.caption.weight(.semibold)).foregroundStyle(store.accent)
                            Text(d.title).font(.title2.weight(.bold)).lineLimit(3)
                            Text(meta.joined(separator: " · ")).font(.caption).foregroundStyle(Theme.secondaryText)
                            HStack(spacing: 6) {
                                if let label { Badge(label, accent: d.status == .available) }
                                if let label4k { Badge("4K · \(label4k)") }
                            }
                        }
                    }
                    .padding(.horizontal, Theme.margin(sizeClass))
                    .offset(y: 40)
                }
                .padding(.bottom, 40)
                VStack(alignment: .leading, spacing: 16) {
                    actions(d, can)
                    if let status, !(d.mediaType == .tv ? status.can.requestTv : status.can.requestMovie) {
                        Text("Your Seerr account may not request \(d.mediaType == .tv ? "shows" : "movies").")
                            .font(.footnote).foregroundStyle(Theme.secondaryText)
                    }
                    if !d.tagline.isEmpty { Text(d.tagline).font(.headline).italic() }
                    if !d.overview.isEmpty { Text(d.overview).font(.body) }
                    if !d.seasons.isEmpty {
                        Text("Seasons").font(.title3.weight(.semibold)).padding(.top, 6)
                        VStack(spacing: 0) {
                            ForEach(d.seasons, id: \.number) { s in season(s) }
                        }
                        .panel()
                    }
                    if !d.requests.isEmpty {
                        Text("Requests").font(.title3.weight(.semibold)).padding(.top, 6)
                        VStack(spacing: 0) {
                            ForEach(d.requests, id: \.id) { r in
                                SeerrRequestRow(request: r, can: can, linked: false) { data.reload() }
                                    .padding(.horizontal, 12)
                            }
                        }
                        .panel()
                    }
                }
                .padding(.horizontal, Theme.margin(sizeClass))
                .padding(.bottom, 24)
            }
        }
        .ignoresSafeArea(edges: .top)
    }

    private func actions(_ d: SeerrDetails, _ can: SeerrCan?) -> some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 10) {
                if let rk = d.ratingKey {
                    Button("Open in Plex", systemImage: "play.fill") { store.navigate(to: .item(ratingKey: rk)) }
                        .buttonStyle(.borderedProminent)
                        .foregroundStyle(.black)
                }
                if let can, SeerrUI.requestable(d, can, false) {
                    let more = d.mediaType == .tv && d.seasons.contains { $0.status != nil || $0.requested != nil }
                    let request = Button(more ? "Request More" : "Request", systemImage: "plus") {
                        asking = Asking(is4k: false)
                    }
                    if d.ratingKey == nil { request.buttonStyle(.borderedProminent).foregroundStyle(.black) } else { request.buttonStyle(.bordered) }
                }
                if let can, SeerrUI.requestable(d, can, true) {
                    Button("Request in 4K") { asking = Asking(is4k: true) }.buttonStyle(.bordered)
                }
                Button("Open in Seerr", systemImage: "arrow.up.right.square") {
                    store.perform { try await Commands.seerrOpen(store.app, store.host, mediaType: d.mediaType, tmdbId: d.tmdbId) }
                }
                .buttonStyle(.bordered)
            }
            .controlSize(.large)
        }
    }

    private func season(_ s: SeerrSeason) -> some View {
        let label: String? =
            switch s.status {
            case .available: "Available"
            case .partial: "Partly available"
            case .processing: "Processing"
            default: s.requested.map(SeerrUI.requestLabel)
            }
        return HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(s.name)
                let sub = [s.episodes.map { Format.plural($0, "episode") }, s.airDate.map(Format.airDate)].compactMap { $0 }
                if !sub.isEmpty {
                    Text(sub.joined(separator: " · ")).font(.caption).foregroundStyle(Theme.secondaryText)
                }
            }
            Spacer()
            if let label { Badge(label, accent: s.status == .available) }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
    }
}

/// Request a movie, or seasons of a show (SeerrRequestDialog.svelte). Users allowed advanced
/// requests also choose the Radarr or Sonarr server, quality profile and folder.
struct SeerrRequestSheet: View {
    let details: SeerrDetails
    let is4k: Bool
    let can: SeerrCan
    let onDone: () -> Void

    @Environment(AppStore.self) private var store
    @Environment(\.dismiss) private var dismiss
    @State private var picked: [Int] = []
    @State private var busy = false
    @State private var servers: [Seerr.RequestOptionServer]?
    @State private var optionsError: String?
    @State private var serverId: Int?
    @State private var profileId: Int?
    @State private var rootFolder: String?
    /// Requests belong to the account this opened for.
    @State private var scope = ""

    private var seasons: [SeerrSeason] { details.mediaType == .tv ? SeerrUI.openSeasons(details, is4k) : [] }
    private var server: Seerr.RequestOptionServer? { servers?.first { $0.id == serverId } }

    var body: some View {
        let d = details
        NavigationStack {
            Form {
                Section {
                    Text(
                        d.mediaType == .tv
                            ? "Choose the seasons to add to the server."
                            : "Seerr asks Radarr to find it and adds it to the server once downloaded.")
                    .foregroundStyle(Theme.secondaryText)
                }
                if d.mediaType == .tv {
                    Section("Seasons") {
                        Toggle(
                            "All Seasons",
                            isOn: Binding(
                                get: { !seasons.isEmpty && picked.count == seasons.count },
                                set: { picked = $0 ? seasons.map(\.number) : [] }))
                        ForEach(seasons, id: \.number) { s in
                            Toggle(isOn: Binding(get: { picked.contains(s.number) }, set: { toggle(s.number, $0) })) {
                                VStack(alignment: .leading, spacing: 2) {
                                    Text(s.name)
                                    let sub = [s.episodes.map { Format.plural($0, "episode") }, s.airDate.map { String($0.prefix(4)) }]
                                        .compactMap { $0 }
                                    if !sub.isEmpty {
                                        Text(sub.joined(separator: " · ")).font(.caption).foregroundStyle(Theme.secondaryText)
                                    }
                                }
                            }
                        }
                    }
                }
                if can.advanced {
                    Section("Options") {
                        if let optionsError {
                            Text(optionsError).foregroundStyle(Theme.danger)
                        } else if let servers, !servers.isEmpty {
                            Picker(
                                "\(d.mediaType == .tv ? "Sonarr" : "Radarr") Server",
                                selection: Binding(get: { serverId }, set: { id in choose(servers.first { $0.id == id }) })
                            ) {
                                ForEach(servers, id: \.id) { s in
                                    Text("\(s.name)\(s.isDefault ? " (default)" : "")").tag(Optional(s.id))
                                }
                            }
                            Picker("Quality Profile", selection: $profileId) {
                                ForEach(server?.profiles ?? [], id: \.id) { p in Text(p.name).tag(Optional(p.id)) }
                            }
                            Picker("Folder", selection: $rootFolder) {
                                ForEach(server?.rootFolders ?? [], id: \.path) { f in
                                    Text("\(f.path)\(f.freeSpace.map { " (\(Format.bytes(Int64($0))) free)" } ?? "")")
                                        .tag(Optional(f.path))
                                }
                            }
                        } else if servers == nil {
                            ProgressView()
                        }
                    }
                }
            }
            .disabled(busy)
            .navigationTitle("Request \(d.title)\(is4k ? " in 4K" : "")")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() }.disabled(busy) }
                ToolbarItem(placement: .confirmationAction) {
                    Button(
                        "Request\(d.mediaType == .tv && !picked.isEmpty ? " \(Format.plural(picked.count, "Season"))" : "")"
                    ) { submit() }
                    .disabled(busy || (d.mediaType == .tv && picked.isEmpty))
                }
            }
        }
        .interactiveDismissDisabled(busy)
        .onAppear { scope = store.scope }
        // Advanced options: servers of the right kind and quality, with their defaults preselected.
        .task {
            guard can.advanced else { return }
            do {
                let list = try await Commands.seerrRequestOptions(store.app, mediaType: d.mediaType, is4k: is4k)
                servers = list
                choose(list.first { $0.isDefault } ?? list.first)
            } catch {
                optionsError = errorText(error)
            }
        }
    }

    private func toggle(_ n: Int, _ on: Bool) {
        picked = on ? (picked + [n]).sorted() : picked.filter { $0 != n }
    }

    private func choose(_ s: Seerr.RequestOptionServer?) {
        serverId = s?.id
        profileId = s?.activeProfileId ?? s?.profiles.first?.id
        rootFolder = s?.activeDirectory ?? s?.rootFolders.first?.path
    }

    private func submit() {
        guard scope == store.scope else { return dismiss() }
        let d = details
        let request = Seerr.NewRequest(
            mediaType: d.mediaType, tmdbId: d.tmdbId, tvdbId: d.tvdbId, seasons: d.mediaType == .tv ? picked : nil,
            is4k: is4k, serverId: server != nil ? serverId : nil, profileId: server != nil ? profileId : nil,
            rootFolder: server != nil ? rootFolder : nil)
        busy = true
        Task {
            do {
                let state = try await Commands.seerrRequest(store.app, request: request)
                store.toast(state == .approved ? "\(d.title) requested and approved" : "\(d.title) requested")
                Haptics.success()
                busy = false
                onDone()
                dismiss()
            } catch {
                busy = false
                store.error(error)
            }
        }
    }
}
