// Port of desktop/core/ops/util.ts.
// Small helpers shared by the ops modules (the Rust code used futures streams and anyhow
// here). `errorText` is the one in State.swift.

import Foundation
import PlexKit

/// The modules of `desktop/core/ops/`: `Ops.Util`, `Ops.Server`, `Ops.Pick`, ...
public enum Ops {}

extension Ops {
    public enum Util {
        /// Run `fn` over `items` with at most `limit` in flight and collect the results in
        /// input order (`stream::iter(..).map(fn).buffered(limit).try_collect()`). The first
        /// failure fails the whole call; work already started is not cancelled but nothing new
        /// is started.
        public static func mapLimit<T: Sendable, R: Sendable>(
            _ items: [T], _ limit: Int, _ fn: @escaping @MainActor @Sendable (T, Int) async throws -> R
        ) async throws -> [R] {
            var out = [R?](repeating: nil, count: items.count)
            var firstError: (any Error)?
            await withThrowingTaskGroup(of: (Int, R).self) { group in
                var next = 0
                func start() {
                    guard next < items.count else { return }
                    let at = next
                    let item = items[at]
                    next += 1
                    group.addTask { (at, try await fn(item, at)) }
                }
                for _ in 0..<min(max(limit, 1), items.count) { start() }
                while let result = await group.nextResult() {
                    switch result {
                    case .success(let (at, value)):
                        out[at] = value
                        if firstError == nil { start() }
                    case .failure(let e):
                        if firstError == nil { firstError = e }
                    }
                }
            }
            if let firstError { throw firstError }
            return out.map { $0! }
        }

        /// Run `fn` over `items` with at most `limit` in flight and hand each result to
        /// `onResult` in completion order (`buffer_unordered(limit)` consumed in a loop).
        public static func eachCompleted<T: Sendable, R: Sendable>(
            _ items: [T], _ limit: Int, _ fn: @escaping @MainActor @Sendable (T) async throws -> R,
            _ onResult: (R, T) -> Void
        ) async throws {
            try await withThrowingTaskGroup(of: (R, T).self) { group in
                var next = 0
                func start() {
                    guard next < items.count else { return }
                    let item = items[next]
                    next += 1
                    group.addTask { (try await fn(item), item) }
                }
                for _ in 0..<min(max(limit, 1), items.count) { start() }
                while let (result, item) = try await group.next() {
                    onResult(result, item)
                    start()
                }
            }
        }

        /// `tokio::time::timeout`, or `Promise.race` with a timer: `op`'s result, or `nil` when
        /// it takes longer than `limit`. Like there, `op` is not cancelled; it finishes on its own.
        public static func within<T: Sendable>(_ limit: Duration, _ op: @escaping @MainActor () async -> T) async -> T? {
            let once = WithinState()
            return await withCheckedContinuation { (k: CheckedContinuation<T?, Never>) in
                once.timer = Task {
                    try? await Task.sleep(for: limit)
                    if once.done { return }
                    once.done = true
                    k.resume(returning: nil)
                }
                Task {
                    let value = await op()
                    once.timer?.cancel()
                    if once.done { return }
                    once.done = true
                    k.resume(returning: value)
                }
            }
        }

        /// `MediaType::from_name(name).map(MediaType::id)`.
        public nonisolated static func mediaTypeId(_ name: String) -> Int? {
            MediaType.fromName(name)?.rawValue
        }

        /// `serde_json::from_value::<Metadata>`: the lenient Plex normalizer.
        public nonisolated static func metadataFromJson(_ v: JSONValue) throws -> Metadata {
            try Metadata(json: v)
        }
    }
}

/// Whether `within` has answered, and its timer.
private final class WithinState {
    var done = false
    var timer: Task<Void, Never>?
}
