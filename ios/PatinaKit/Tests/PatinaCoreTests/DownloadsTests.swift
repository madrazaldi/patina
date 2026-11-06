// Port of desktop/core/downloads.test.ts, plus the iOS-only re-anchoring of stored paths.

import Foundation
import Testing

@testable import PatinaCore

@Suite struct DownloadsTests {
    func offset(_ status: Int, _ range: String?, _ requested: Int) throws -> [Int?] {
        let (o, t) = try Downloads.responseOffset(status, range, requested)
        return [o, t]
    }

    func message(_ f: () throws -> Any) -> String? {
        do {
            _ = try f()
            return nil
        } catch {
            return errorText(error)
        }
    }

    @Test func rangesNeverAppendAFullOrMisalignedResponse() throws {
        #expect(try offset(200, nil, 500) == [0, nil])
        #expect(try offset(206, "bytes 500-999/1000", 500) == [500, 1000])
        #expect(throws: (any Error).self) { try offset(206, "bytes 0-499/1000", 500) }
        #expect(throws: (any Error).self) { try offset(206, nil, 500) }
        #expect(throws: (any Error).self) { try offset(206, "bytes 500-1000/1000", 500) }
    }

    @Test func rangeErrorsKeepTheRustTexts() {
        #expect(message { try offset(500, nil, 0) } == "Unexpected download response: 500")
        #expect(message { try offset(206, nil, 0) } == "Missing Content-Range")
        #expect(message { try offset(206, "bytes 5", 0) } == "Invalid Content-Range")
        #expect(message { try offset(206, "bytes 5/10", 0) } == "Invalid byte range")
        #expect(message { try offset(206, "bytes a-5/10", 0) } == "invalid digit found in string")
        #expect(message { try offset(206, "bytes -5/10", 0) } == "cannot parse integer from empty string")
        #expect(message { try offset(206, "bytes 1-5/10", 0) } == "Server returned the wrong byte range")
    }

    @Test func fileNamesAreCleanedLikeTheRustVersion() {
        #expect(Downloads.clean("A: B/C") == "A B C")
        #expect(Downloads.clean("  ..hidden..  ") == "hidden")
        #expect(Downloads.clean("") == "Untitled")
        #expect(Downloads.clean("///") == "Untitled")
    }

    @Test func storedPathsFollowTheAppContainer() {
        let dir = URL(fileURLWithPath: "/new/Documents/Downloads")
        #expect(Downloads.relocate("/old/Documents/Downloads/Mock/a.mkv", dir) == "/new/Documents/Downloads/Mock/a.mkv")
        #expect(Downloads.relocate("/new/Documents/Downloads/a.mkv", dir) == "/new/Documents/Downloads/a.mkv")
        #expect(Downloads.relocate("/elsewhere/a.mkv", dir) == "/elsewhere/a.mkv")
        #expect(Downloads.relocate(nil, dir) == nil)
    }
}
