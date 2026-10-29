// ASS and SSA subtitles through libass (no desktop twin: mpv drew them with libass itself).
//
// Under `LoadOptions.preserveASSMarkup` AetherEngine hands over each event as the raw Matroska
// line (`ReadOrder,Layer,Style,…,Text`), the track's script header and the container's fonts.
// libass renders them as mpv does, fonts, positions, motion, clips and karaoke included; this
// flattens what it draws into one bitmap per frame. Everything libass touches stays on `queue`.

import AetherEngine
import CoreGraphics
import Foundation
import SwiftLibass

/// One event of an ASS track, as AetherEngine publishes it (seconds on the source axis).
nonisolated struct ASSEvent: Sendable {
    /// One or more raw event lines, newline-separated.
    var text: String
    var start: Double
    var end: Double
}

/// A rendered frame: `image` covers `rect` of a canvas `canvas` pixels in size.
nonisolated struct ASSFrame: @unchecked Sendable {
    let image: CGImage
    let rect: CGRect
    let canvas: CGSize
}

nonisolated final class ASSRenderer: @unchecked Sendable {
    /// What a render produced. `changed` is false when the picture is the previous one.
    struct Output: @unchecked Sendable {
        var changed: Bool
        var frame: ASSFrame?
    }

    private let queue = DispatchQueue(label: "Patina.ASSRenderer", qos: .userInitiated)
    // Queue-confined.
    private var context: Context?
    private var seen: Set<String> = []
    private var canvas = (width: 0, height: 0)
    private var storage = (width: 0, height: 0)

    init(header: String, fonts: [FontAttachment]) {
        queue.async { [self] in context = Context(header: header, fonts: fonts) }
    }

    func add(_ events: [ASSEvent]) {
        queue.async { [self] in
            guard let context else { return }
            for event in events {
                let start = Int64((event.start * 1000).rounded())
                let duration = max(0, Int64((event.end * 1000).rounded()) - start)
                for line in event.text.split(separator: "\n", omittingEmptySubsequences: true) {
                    // ReadOrder can't tell repeats apart (real files set it to 0 on every line), so
                    // the line and its times do, which also absorbs what the engine re-sends after a seek.
                    let fields = line.split(separator: ",", maxSplits: 8, omittingEmptySubsequences: false)
                    guard fields.count == 9, Int(fields[0]) != nil else { continue }
                    guard seen.insert("\(start)|\(duration)|\(line)").inserted else { continue }
                    context.process(String(line), start: start, duration: duration)
                }
            }
        }
    }

    /// Renders the frame at `milliseconds` for a canvas of `width`×`height` pixels laid over a
    /// picture whose coded size is `storage`, and hands the result to `done` on the queue.
    func render(milliseconds: Int64, width: Int, height: Int, storage: CGSize?,
                done: @escaping @Sendable (Output) -> Void) {
        queue.async { [self] in
            guard let context, width > 0, height > 0 else { return done(Output(changed: false)) }
            var resized = false
            if canvas != (width, height) {
                canvas = (width, height)
                ass_set_frame_size(context.renderer, Int32(width), Int32(height))
                resized = true
            }
            let coded = (Int(storage?.width ?? 0), Int(storage?.height ?? 0))
            if self.storage != coded {
                self.storage = coded
                ass_set_storage_size(context.renderer, Int32(coded.0), Int32(coded.1))
                resized = true
            }
            var change: Int32 = 0
            let images = ass_render_frame(context.renderer, context.track, milliseconds, &change)
            guard change != 0 || resized else { return done(Output(changed: false)) }
            done(Output(changed: true, frame: Self.compose(images, canvas: CGSize(width: width, height: height))))
        }
    }

    /// Blends libass's alpha masks into one premultiplied RGBA bitmap over their bounding box.
    private static func compose(_ first: UnsafeMutablePointer<ASS_Image>?, canvas: CGSize) -> ASSFrame? {
        var minX = Int.max, minY = Int.max, maxX = Int.min, maxY = Int.min
        var node = first
        while let image = node?.pointee {
            if image.w > 0, image.h > 0 {
                minX = min(minX, Int(image.dst_x))
                minY = min(minY, Int(image.dst_y))
                maxX = max(maxX, Int(image.dst_x + image.w))
                maxY = max(maxY, Int(image.dst_y + image.h))
            }
            node = image.next
        }
        guard minX < maxX, minY < maxY else { return nil }
        let width = maxX - minX, height = maxY - minY, bytesPerRow = width * 4
        var pixels = Data(count: bytesPerRow * height)
        pixels.withUnsafeMutableBytes { raw in
            let base = raw.baseAddress!.assumingMemoryBound(to: UInt8.self)
            var node = first
            while let image = node?.pointee {
                node = image.next
                guard image.w > 0, image.h > 0, let bitmap = image.bitmap else { continue }
                // RGBA, the last byte being transparency.
                let r = (image.color >> 24) & 0xFF, g = (image.color >> 16) & 0xFF, b = (image.color >> 8) & 0xFF
                let opacity = 255 - (image.color & 0xFF)
                for y in 0..<Int(image.h) {
                    let src = bitmap + y * Int(image.stride)
                    var dst = base + (Int(image.dst_y) - minY + y) * bytesPerRow + (Int(image.dst_x) - minX) * 4
                    for x in 0..<Int(image.w) {
                        let a = (UInt32(src[x]) * opacity + 127) / 255
                        if a > 0 {
                            let keep = 255 - a
                            dst[0] = UInt8((r * a + UInt32(dst[0]) * keep + 127) / 255)
                            dst[1] = UInt8((g * a + UInt32(dst[1]) * keep + 127) / 255)
                            dst[2] = UInt8((b * a + UInt32(dst[2]) * keep + 127) / 255)
                            dst[3] = UInt8(a + (UInt32(dst[3]) * keep + 127) / 255)
                        }
                        dst += 4
                    }
                }
            }
        }
        guard let provider = CGDataProvider(data: pixels as CFData),
              let image = CGImage(
                  width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: bytesPerRow,
                  space: CGColorSpace(name: CGColorSpace.sRGB)!,
                  bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
                  provider: provider, decode: nil, shouldInterpolate: true, intent: .defaultIntent
              )
        else { return nil }
        return ASSFrame(image: image, rect: CGRect(x: minX, y: minY, width: width, height: height), canvas: canvas)
    }

    /// The libass library, renderer and track of one script.
    private final class Context {
        let library: OpaquePointer
        let renderer: OpaquePointer
        let track: UnsafeMutablePointer<ASS_Track>

        init?(header: String, fonts: [FontAttachment]) {
            guard let library = ass_library_init() else { return nil }
            // libass logs every font lookup to stderr otherwise.
            ass_set_message_cb(library, { _, _, _, _ in }, nil)
            for font in fonts {
                font.data.withUnsafeBytes { bytes in
                    ass_add_font(library, font.filename, bytes.baseAddress?.assumingMemoryBound(to: CChar.self), Int32(bytes.count))
                }
            }
            guard let renderer = ass_renderer_init(library) else {
                ass_library_done(library)
                return nil
            }
            // Fonts the script names and the file doesn't carry come from the system, through CoreText.
            ass_set_fonts(renderer, nil, "Helvetica Neue", Int32(ASS_FONTPROVIDER_CORETEXT.rawValue), nil, 0)
            guard let track = ass_new_track(library) else {
                ass_renderer_done(renderer)
                ass_library_done(library)
                return nil
            }
            // Duplicates are dropped by content in `add`.
            ass_set_check_readorder(track, 0)
            // Matroska headers are often NUL-terminated, and libass stops reading at a NUL.
            let header = header.replacingOccurrences(of: "\0", with: "")
            ass_process_codec_private(track, header, Int32(header.utf8.count))
            self.library = library
            self.renderer = renderer
            self.track = track
        }

        func process(_ line: String, start: Int64, duration: Int64) {
            ass_process_chunk(track, line, Int32(line.utf8.count), start, duration)
        }

        deinit {
            ass_free_track(track)
            ass_renderer_done(renderer)
            ass_library_done(library)
        }
    }

    /// libavcodec's header for a track that carries none: one plain style.
    static let defaultHeader = """
        [Script Info]
        ScriptType: v4.00+
        PlayResX: 384
        PlayResY: 288
        ScaledBorderAndShadow: yes

        [V4+ Styles]
        Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
        Style: Default,Arial,16,&Hffffff,&Hffffff,&H0,&H0,0,0,0,0,100,100,0,0,1,1,0,2,10,10,10,1

        [Events]
        Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
        """
}
