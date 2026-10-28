// Port of web/src/lib/fuzzy.ts: small fuzzy matcher for the command palette. Every
// character of the query must appear in the text in order. Runs, word starts and
// early matches score higher.

import Foundation

public enum Fuzzy {
    private static let separators: Set<Character> = [" ", "\t", "\n", "-", "_", "/", ":", ".", ","]

    /// A score (higher is better), or nil when `query` doesn't match `text`.
    public static func fuzzy(_ query: String, _ text: String) -> Double? {
        let q = Array(query.lowercased().filter { !$0.isWhitespace })
        if q.isEmpty { return 0 }
        let t = Array(text.lowercased())
        var score = 0.0
        var from = 0
        var previous = -2
        for ch in q {
            guard let at = t[from...].firstIndex(of: ch) else { return nil }
            score += 1
            if at == previous + 1 { score += 4 }
            if at == 0 || separators.contains(t[at - 1]) { score += 6 }
            score -= Double(min(at - from, 6)) * 0.3
            previous = at
            from = at + 1
        }
        let qs = String(q)
        let ts = String(t)
        if ts.hasPrefix(qs) { score += 12 } else if ts.contains(qs) { score += 6 }
        return score - Double(t.count) * 0.02
    }

    /// Match a possibly multi-word query: each word has to match the label fuzzily, or failing
    /// that the start of one of the keywords (which counts for less). The scores add up.
    public static func match(_ query: String, label: String, keywords: String = "") -> Double? {
        let words = query.split(whereSeparator: \.isWhitespace).map(String.init)
        if words.isEmpty { return 0 }
        let extra = keywords.lowercased().split(whereSeparator: \.isWhitespace)
        var total = 0.0
        for word in words {
            if let direct = fuzzy(word, label) {
                total += direct
                continue
            }
            let lower = word.lowercased()
            if !extra.contains(where: { $0.hasPrefix(lower) }) { return nil }
            total += 2
        }
        return total
    }
}
