// Port of desktop/core/storage.test.ts. The cross-process test and the lock-path test have
// no counterpart: there is one process and no lock file.

import Foundation
import Testing

@testable import PatinaCore

@Suite struct StorageTests {
    let dir = TempDir("patina-storage")

    @Test func unchangedTransactionsDoNotReplaceTheFile() throws {
        let path = dir.file("state.json")
        try Data("[1,2]".utf8).write(to: path)
        try Storage.updateJson(path, { [Int]() }) { _ in }
        #expect(try String(contentsOf: path, encoding: .utf8) == "[1,2]")
    }

    @Test func concurrentWritersPreserveEveryUpdate() async throws {
        let path = dir.file("counter.json")
        // Tasks interleaving their updates, as the poller and a command do.
        let writers = (0..<8).map { _ in
            Task {
                for _ in 0..<20 {
                    _ = try Storage.replaceJson(path, { 0 }) { $0 + 1 }
                    await Task.yield()
                }
            }
        }
        for w in writers { try await w.value }
        #expect(try Storage.readJson(path) { 0 } == 160)
        for _ in 0..<5 { try Storage.replaceJson(path, { 0 }) { $0 + 1 } }
        #expect(try Storage.readJson(path) { 0 } == 165)
    }

    @Test func corruptStateIsNotOverwritten() throws {
        let path = dir.file("state.json")
        try Data("broken".utf8).write(to: path)
        #expect(throws: (any Error).self) { try Storage.updateJson(path, { [String]() }) { $0.removeAll() } }
        #expect(try String(contentsOf: path, encoding: .utf8) == "broken")
    }

    @Test func transferLeaseExcludesAnotherWriter() {
        let path = dir.file("worker")
        let first = Storage.tryLock(path)
        #expect(first != nil)
        #expect(Storage.tryLock(path) == nil)
        first?.release()
        let again = Storage.tryLock(path)
        #expect(again != nil)
        again?.release()
    }

    @Test func writesArePrivateAndAtomic() throws {
        let path = dir.url.appendingPathComponent("sub/file")
        try Storage.write(path, Data("one".utf8))
        try Storage.write(path, Data("two".utf8))
        #expect(try String(contentsOf: path, encoding: .utf8) == "two")
        let mode = try FileManager.default.attributesOfItem(atPath: path.path)[.posixPermissions] as? Int
        #expect(mode.map { $0 & 0o777 } == 0o600)
    }
}
