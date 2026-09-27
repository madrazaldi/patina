import AppKit
import CPatina
import Foundation

/// A command failure, carrying the backend's message.
struct CoreError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

/// Encodes any argument value as JSON; `nil` optionals become null.
private struct AnyEncodable: Encodable {
    let value: any Encodable
    func encode(to encoder: Encoder) throws { try value.encode(to: encoder) }
}

/// The Rust backend (crates/ffi): commands by name, as in the Tauri app's
/// `web/src/lib/api.ts`, and core events by name.
enum Core {
    typealias Args = [String: (any Encodable)?]

    /// Start the backend. `onEvent` runs on the main actor with each event's name and JSON payload.
    static func start(onEvent: @escaping @MainActor (String, Data) -> Void) {
        let box = Unmanaged.passRetained(EventBox(onEvent)).toOpaque()
        patina_start(box) { ctx, name, payload in
            guard let ctx, let name, let payload else { return }
            let handler = Unmanaged<EventBox>.fromOpaque(ctx).takeUnretainedValue().handler
            let n = String(cString: name)
            let data = Data(bytes: payload, count: strlen(payload))
            // The main queue keeps events in order, unlike separate tasks.
            DispatchQueue.main.async { MainActor.assumeIsolated { handler(n, data) } }
        }
    }

    static func shutdown() {
        patina_shutdown()
    }

    /// Call a command, returning its raw JSON result.
    static func raw(_ name: String, _ args: Args = [:]) async throws -> Data {
        var object: [String: AnyEncodable] = [:]
        for (k, v) in args {
            object[k] = AnyEncodable(value: v ?? Optional<String>.none)
        }
        let json = String(decoding: try JSONEncoder.patina.encode(object), as: UTF8.self)
        return try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Data, Error>) in
            let box = Unmanaged.passRetained(ReplyBox(cont)).toOpaque()
            patina_call(name, json, box) { ctx, ok, result in
                guard let ctx else { return }
                let cont = Unmanaged<ReplyBox>.fromOpaque(ctx).takeRetainedValue().cont
                let data = result.map { Data(bytes: $0, count: strlen($0)) } ?? Data("null".utf8)
                if ok {
                    cont.resume(returning: data)
                } else {
                    let message = (try? JSONDecoder.patina.decode(String.self, from: data)) ?? "Something went wrong."
                    cont.resume(throwing: CoreError(message: message))
                }
            }
        }
    }

    /// Call a command and decode its result.
    static func call<T: Decodable>(_ name: String, _ args: Args = [:], as type: T.Type = T.self) async throws -> T {
        let data = try await raw(name, args)
        do {
            return try JSONDecoder.patina.decode(T.self, from: data)
        } catch {
            NSLog("Patina: could not decode %@: %@", name, String(describing: error))
            throw CoreError(message: "Unexpected response from \(name).")
        }
    }

    /// Call a command whose result doesn't matter.
    static func run(_ name: String, _ args: Args = [:]) async throws {
        _ = try await raw(name, args)
    }

    /// Plex artwork, resized and cached by the backend.
    static func image(_ source: String, width: Int, height: Int) async -> Data? {
        await withCheckedContinuation { (cont: CheckedContinuation<Data?, Never>) in
            let box = Unmanaged.passRetained(BytesBox(cont)).toOpaque()
            patina_image(source, UInt32(max(width, 1)), UInt32(max(height, 1)), box, bytesCallback)
        }
    }

    /// The saved artwork of a download (works offline).
    static func downloadImage(_ id: String) async -> Data? {
        await withCheckedContinuation { (cont: CheckedContinuation<Data?, Never>) in
            let box = Unmanaged.passRetained(BytesBox(cont)).toOpaque()
            patina_download_image(id, box, bytesCallback)
        }
    }
}

private let bytesCallback: patina_bytes_cb = { ctx, data, len in
    guard let ctx else { return }
    let cont = Unmanaged<BytesBox>.fromOpaque(ctx).takeRetainedValue().cont
    cont.resume(returning: data.flatMap { len > 0 ? Data(bytes: $0, count: len) : nil })
}

private final class EventBox {
    let handler: @MainActor (String, Data) -> Void
    init(_ handler: @escaping @MainActor (String, Data) -> Void) { self.handler = handler }
}

private final class ReplyBox {
    let cont: CheckedContinuation<Data, Error>
    init(_ cont: CheckedContinuation<Data, Error>) { self.cont = cont }
}

private final class BytesBox {
    let cont: CheckedContinuation<Data?, Never>
    init(_ cont: CheckedContinuation<Data?, Never>) { self.cont = cont }
}
