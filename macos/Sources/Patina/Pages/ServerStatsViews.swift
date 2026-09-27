import Charts
import SwiftUI

/// A big number over a small label, for server and stream figures.
struct ServerFigure: View {
    let value: String
    let label: String

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(value).font(Theme.display(26)).monospacedDigit()
            Text(label).font(Theme.caption).foregroundStyle(.secondary)
        }
    }
}

/// A quiet outlined label: "Direct play", "Home", "Disabled", ...
struct ServerTag: View {
    let text: String
    var accent = false
    init(_ text: String, accent: Bool = false) {
        self.text = text
        self.accent = accent
    }

    @Environment(AppModel.self) private var app

    var body: some View {
        Text(text)
            .font(.system(size: 10.5, weight: .medium))
            .foregroundStyle(accent ? AnyShapeStyle(app.accent) : AnyShapeStyle(.secondary))
            .padding(.horizontal, 7)
            .padding(.vertical, 2.5)
            .overlay(
                RoundedRectangle(cornerRadius: 3)
                    .strokeBorder(accent ? app.accent.opacity(0.6) : Color.primary.opacity(0.15))
            )
            .lineLimit(1)
    }
}

/// Data sent per time bucket as stacked columns: remote on the baseline,
/// local network above it (web/src/components/StatsBandwidth.svelte). Hover a
/// column for its values; the same numbers are available as a table.
struct ServerBandwidthChart: View {
    let buckets: [ServerStats.Bucket]
    /// Start of the period, unix seconds.
    let since: Int
    /// Bucket length in seconds.
    let step: Int

    @Environment(AppModel.self) private var app
    @State private var hover: Date?
    @State private var asTable = false

    struct Slot: Identifiable {
        var at: Int64
        var lan: Double
        var wan: Double
        var id: Int64 { at }
        var date: Date { Date(timeIntervalSince1970: TimeInterval(at)) }
        var total: Double { lan + wan }
    }

    private struct Mark: Identifiable {
        var slot: Slot
        var series: String
        var bytes: Double
        var id: String { "\(slot.at):\(series)" }
    }

    private var hourly: Bool { step < 86400 }

    /// Buckets on a continuous axis aligned to the server's own boundaries, so quiet periods show as gaps.
    private var slots: [Slot] {
        let step = Int64(self.step)
        let end = max(Int64(Date().timeIntervalSince1970), buckets.last?.at ?? 0)
        let base = buckets.first?.at ?? Int64(since)
        let start = base - max(0, (base - Int64(since)) / step) * step
        let n = max(1, Int((end - start) / step) + 1)
        var out = (0..<n).map { Slot(at: start + Int64($0) * step, lan: 0, wan: 0) }
        for b in buckets {
            let i = Int((b.at - start) / step)
            guard i >= 0, i < n else { continue }
            out[i].lan += b.lan
            out[i].wan += b.wan
        }
        return out
    }

    var body: some View {
        let slots = self.slots
        let totals = slots.reduce((lan: 0.0, wan: 0.0)) { ($0.lan + $1.lan, $0.wan + $1.wan) }
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .firstTextBaseline, spacing: 20) {
                legend("Remote", totals.wan, app.accent)
                legend("Local network", totals.lan, Self.localColor)
                Spacer()
                Button(asTable ? "Show Chart" : "Show as Table") { asTable.toggle() }
                    .buttonStyle(.link)
                    .font(Theme.caption)
            }
            if asTable {
                table(slots)
            } else {
                chart(slots)
            }
        }
    }

    static let localColor = Color.primary.opacity(0.28)

    private func legend(_ label: String, _ bytes: Double, _ color: Color) -> some View {
        HStack(spacing: 6) {
            RoundedRectangle(cornerRadius: 2).fill(color).frame(width: 10, height: 10)
            Text(label).foregroundStyle(.secondary)
            Text(Fmt.bytes(Int64(bytes))).fontWeight(.medium).monospacedDigit()
        }
        .font(Theme.caption)
    }

    private func chart(_ slots: [Slot]) -> some View {
        let marks = slots.flatMap { s in
            [Mark(slot: s, series: "Remote", bytes: s.wan), Mark(slot: s, series: "Local network", bytes: s.lan)]
        }
        let unit: Calendar.Component = hourly ? .hour : .day
        let selected = hover.flatMap { h in slots.min { abs($0.date.timeIntervalSince(h)) < abs($1.date.timeIntervalSince(h)) } }
        return Chart {
            ForEach(marks) { m in
                BarMark(
                    x: .value("Time", m.slot.date, unit: unit),
                    y: .value("Bytes", m.bytes),
                    width: .ratio(0.62)
                )
                .foregroundStyle(by: .value("Network", m.series))
                .opacity(selected == nil || selected?.at == m.slot.at ? 1 : 0.45)
            }
            if let s = selected {
                RuleMark(x: .value("Time", s.date, unit: unit))
                    .foregroundStyle(Color.primary.opacity(0.12))
                    .zIndex(-1)
                    .annotation(position: .top, spacing: 4, overflowResolution: .init(x: .fit(to: .chart), y: .disabled)) {
                        tip(s)
                    }
            }
        }
        .chartForegroundStyleScale(["Remote": app.accent, "Local network": Self.localColor])
        .chartLegend(.hidden)
        .chartXSelection(value: $hover)
        .chartYAxis {
            AxisMarks(position: .leading, values: .automatic(desiredCount: 4)) { v in
                AxisGridLine().foregroundStyle(Theme.hairline)
                AxisValueLabel {
                    if let b = v.as(Double.self) { Text(b == 0 ? "0" : Fmt.bytes(Int64(b))).monospacedDigit() }
                }
            }
        }
        .chartXAxis {
            if hourly {
                AxisMarks(values: .stride(by: .hour, count: 6)) { _ in
                    AxisValueLabel(format: .dateTime.hour())
                }
            } else {
                AxisMarks(values: .stride(by: .day, count: 7)) { _ in
                    AxisValueLabel(format: .dateTime.month(.abbreviated).day())
                }
            }
        }
        .frame(height: 200)
    }

    private func tip(_ s: Slot) -> some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(s.date, format: hourly ? .dateTime.weekday().hour().minute() : .dateTime.weekday().month().day())
                .fontWeight(.medium)
            if s.total == 0 {
                Text("Nothing streamed").foregroundStyle(.secondary)
            } else {
                Text("\(Fmt.bytes(Int64(s.wan))) remote").monospacedDigit()
                Text("\(Fmt.bytes(Int64(s.lan))) local network").monospacedDigit()
            }
        }
        .font(Theme.caption)
        .padding(8)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 6))
        .overlay(RoundedRectangle(cornerRadius: 6).strokeBorder(Theme.hairline))
    }

    private func table(_ slots: [Slot]) -> some View {
        let rows = slots.filter { $0.total > 0 }.reversed()
        return Table(Array(rows)) {
            TableColumn(hourly ? "Hour" : "Day") { s in
                Text(s.date, format: hourly ? .dateTime.weekday().hour().minute() : .dateTime.weekday().month().day())
            }
            TableColumn("Remote") { s in Text(Fmt.bytes(Int64(s.wan))).monospacedDigit() }
            TableColumn("Local network") { s in Text(Fmt.bytes(Int64(s.lan))).monospacedDigit() }
            TableColumn("Total") { s in Text(Fmt.bytes(Int64(s.total))).monospacedDigit() }
        }
        .frame(height: 240)
    }
}

/// Ranked list with a thin bar per row, scaled to the largest entry
/// (web/src/components/StatsRanking.svelte).
struct ServerRanking: View {
    let rows: [ServerStats.Usage]
    var limit = 5
    var empty = "Nothing yet."

    @Environment(AppModel.self) private var app

    var body: some View {
        let top = Array(rows.sorted { $0.bytes > $1.bytes }.prefix(limit))
        let max = Swift.max(1, top.map(\.bytes).max() ?? 1)
        let total = Swift.max(1, rows.reduce(0) { $0 + $1.bytes })
        if top.isEmpty {
            Text(empty).foregroundStyle(.secondary)
        } else {
            VStack(alignment: .leading, spacing: 12) {
                ForEach(Array(top.enumerated()), id: \.offset) { _, r in
                    VStack(alignment: .leading, spacing: 5) {
                        HStack {
                            Text(r.name).lineLimit(1)
                            Spacer()
                            Text(Fmt.bytes(Int64(r.bytes))).font(Theme.caption).foregroundStyle(.secondary).monospacedDigit()
                        }
                        GeometryReader { g in
                            RoundedRectangle(cornerRadius: 2)
                                .fill(app.accent)
                                .frame(width: Swift.max(2, g.size.width * r.bytes / max))
                        }
                        .frame(height: 4)
                    }
                    .help("\(Int((r.bytes / total * 100).rounded()))% of all data")
                }
            }
        }
    }
}
