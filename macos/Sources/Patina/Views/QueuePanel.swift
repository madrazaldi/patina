import SwiftUI

/// The running play queue, in a popover over the now-playing bar. Rows
/// reorder by dragging; double-click jumps to an entry.
struct QueuePanel: View {
    @Environment(AppModel.self) private var app

    var body: some View {
        VStack(spacing: 0) {
            if let queue = app.queue {
                header(queue)
                Hairline()
                if queue.items.isEmpty {
                    SheetMessage(text: "The queue is empty.")
                } else {
                    list(queue)
                }
            } else {
                SheetMessage(text: "Nothing is playing.")
            }
        }
        .frame(width: 380, height: 440)
    }

    private func header(_ q: QueueView) -> some View {
        let current = q.items.firstIndex { $0.uid == q.current } ?? -1
        let upcoming = q.items.count - current - 1
        return HStack(spacing: 8) {
            Text("Up Next").font(.system(size: 13, weight: .semibold))
            Text(upcoming > 0 ? Fmt.plural(upcoming, "item") : "Nothing")
                .font(.system(size: 12)).foregroundStyle(.secondary)
            Spacer()
            Toggle(isOn: Binding(get: { q.shuffled }, set: { sendQueue(.shuffle($0)) })) {
                Image(systemName: "shuffle")
            }
            .help(q.shuffled ? "Unshuffle" : "Shuffle")
            Toggle(isOn: Binding(get: { q.stopAfter }, set: { sendQueue(.stopAfter($0)) })) {
                Image(systemName: "stop.circle")
            }
            .help(q.stopAfter ? "Keep playing after this" : "Stop after this")
        }
        .toggleStyle(.button)
        .controlSize(.small)
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
    }

    private func list(_ q: QueueView) -> some View {
        let current = q.items.firstIndex { $0.uid == q.current } ?? -1
        return ScrollViewReader { proxy in
            List {
                ForEach(Array(q.items.enumerated()), id: \.element.uid) { i, entry in
                    QueueRow(entry: entry, playing: i == current, past: current >= 0 && i < current)
                        .id(entry.uid)
                }
                .onMove { from, to in move(q, from: from, to: to) }
            }
            .listStyle(.plain)
            .onAppear {
                // Open with the playing entry near the top, one earlier entry above it.
                let target = current > 0 ? q.items[current - 1].uid : q.items.first?.uid
                if let target { proxy.scrollTo(target, anchor: .top) }
            }
        }
    }

    private func move(_ q: QueueView, from: IndexSet, to: Int) {
        guard let i = from.first, from.count == 1, to != i, to != i + 1 else { return }
        let uid = q.items[i].uid
        let before = to < q.items.count ? q.items[to].uid : nil
        sendQueue(.move(uid, before: before))
    }
}

struct QueueRow: View {
    let entry: QueueItem
    let playing: Bool
    let past: Bool
    @Environment(AppModel.self) private var app
    @State private var hover = false

    var body: some View {
        let item = entry.item
        let (t, s) = item.playingLines
        HStack(spacing: 10) {
            Artwork(item.posterPath ?? item.thumb, width: 72, height: 72)
                .frame(width: 34, height: 34)
                .clipShape(RoundedRectangle(cornerRadius: 3))
            VStack(alignment: .leading, spacing: 1) {
                Text(t).font(.system(size: 12, weight: playing ? .semibold : .regular)).lineLimit(1)
                if !s.isEmpty { Text(s).font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(1) }
            }
            Spacer(minLength: 6)
            if playing {
                Image(systemName: "waveform").font(.system(size: 12)).foregroundStyle(app.accent)
            } else if hover {
                Button { sendQueue(.remove(entry.uid)) } label: {
                    Image(systemName: "xmark").font(.system(size: 10, weight: .semibold))
                }
                .buttonStyle(.borderless)
                .help("Remove from Queue")
            } else if let d = item.duration {
                Text(item.kind == "track" ? Fmt.clock(d) : Fmt.duration(d))
                    .font(.system(size: 11).monospacedDigit())
                    .foregroundStyle(.secondary)
            }
        }
        .opacity(past ? 0.5 : 1)
        .padding(.vertical, 2)
        .contentShape(Rectangle())
        .onHover { hover = $0 }
        .onTapGesture(count: 2) { jump() }
        .contextMenu {
            Button("Play Now", systemImage: "play") { jump() }.disabled(playing)
            Button("Remove from Queue", systemImage: "minus.circle") { sendQueue(.remove(entry.uid)) }.disabled(playing)
            Divider()
            Button("Show \(item.kind == "track" ? "Track" : "Details")") { app.showQueue = false; app.open(item) }
        }
    }

    private func jump() {
        if !playing { sendQueue(.jump(entry.uid)) }
    }
}
