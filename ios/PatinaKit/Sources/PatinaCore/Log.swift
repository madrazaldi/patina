// Port of desktop/core/log.ts.
// Logging for the core. PATINA_LOG ("debug", "info", "warn", "error", or "off") sets the
// level (default: info). Messages go to the unified log (Console.app, `log stream`) under
// the app's subsystem, with the module name as the category, instead of stderr.

import Foundation
import os

nonisolated private enum Level: Int {
    case debug = 10, info = 20, warn = 30, error = 40, off = 99
}

nonisolated private let threshold: Int = {
    let raw = (ProcessInfo.processInfo.environment["PATINA_LOG"] ?? "").lowercased()
    // Accepts tracing-style filters like "warn,patina=info,mpv=warn": the first bare level wins.
    let names: [String: Level] = ["debug": .debug, "info": .info, "warn": .warn, "error": .error, "off": .off]
    let first = raw.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.first { names[$0] != nil }
    return (first.flatMap { names[$0] } ?? .info).rawValue
}()

/// A logger tagged with a module name, like `tracing`'s target.
public nonisolated struct Log: Sendable {
    public let scope: String
    private let sink: os.Logger

    init(scope: String) {
        self.scope = scope
        sink = os.Logger(subsystem: "io.github.madrazaldi.patina", category: scope)
    }

    public func debug(_ message: @autoclosure () -> String) {
        guard Level.debug.rawValue >= threshold else { return }
        let text = message()
        sink.debug("\(text, privacy: .public)")
    }

    public func info(_ message: @autoclosure () -> String) {
        guard Level.info.rawValue >= threshold else { return }
        let text = message()
        sink.info("\(text, privacy: .public)")
    }

    public func warn(_ message: @autoclosure () -> String) {
        guard Level.warn.rawValue >= threshold else { return }
        let text = message()
        sink.warning("\(text, privacy: .public)")
    }

    public func error(_ message: @autoclosure () -> String) {
        guard Level.error.rawValue >= threshold else { return }
        let text = message()
        sink.error("\(text, privacy: .public)")
    }
}

/// `logger("state")`.
public nonisolated func logger(_ scope: String) -> Log {
    Log(scope: scope)
}
