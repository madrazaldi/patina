// The picture and its subtitles (no desktop twin: mpv drew both in its own window).
//
// AetherEngine leaves subtitles to the host: it publishes the cues, and this view draws them
// over the picture, text at the bottom of the visible picture (or where a cue asks) and bitmaps
// (PGS, DVB, VobSub) where the disc placed them. ASS and SSA tracks go to libass instead
// (`ASSSubtitles`).

import AetherEngine
import SwiftUI

/// How text subtitles look.
nonisolated public struct SubtitleStyle: Equatable, Sendable {
    public enum Background: String, CaseIterable, Sendable {
        /// A dark outline around each letter.
        case outline
        /// A translucent box behind each line.
        case box
    }

    /// 1 is the default size; 0.5 to 2 is sensible.
    public var scale: Double
    public var background: Background

    public init(scale: Double = 1, background: Background = .outline) {
        self.scale = scale
        self.background = background
    }
}

public struct PlayerSurface: View {
    private let player: AetherPlaybackEngine
    private let fill: PlayerVideoFill
    private let subtitleStyle: SubtitleStyle
    private let subtitleLift: CGFloat

    /// - Parameter subtitleLift: How far to raise bottom subtitles, to clear the controls while they show.
    public init(player: AetherPlaybackEngine, fill: PlayerVideoFill = .fit, subtitleStyle: SubtitleStyle = .init(), subtitleLift: CGFloat = 0) {
        self.player = player
        self.fill = fill
        self.subtitleStyle = subtitleStyle
        self.subtitleLift = subtitleLift
    }

    public var body: some View {
        GeometryReader { geo in
            let bounds = CGRect(origin: .zero, size: geo.size)
            let video = PlayerGeometry.videoRect(aspect: player.videoAspect, in: bounds, fill: fill)
            ZStack {
                Color.black
                AetherPlayerSurface(engine: player.engine)
                    .scaleEffect(fill == .zoom ? PlayerVideoFill.zoomScale : 1)
                    .animation(.spring(response: 0.4, dampingFraction: 0.9), value: fill)
                SubtitleOverlay(cues: player.subtitleCues, video: video, bounds: bounds, style: subtitleStyle, lift: subtitleLift)
                ASSSubtitleLayer(subtitles: player.assSubtitles, video: video, bounds: bounds)
            }
            .frame(width: geo.size.width, height: geo.size.height)
            .clipped()
        }
        .background(.black)
        .accessibilityHidden(true)
        .onAppear { applyGravity() }
        .onChange(of: fill) { applyGravity() }
    }

    private func applyGravity() {
        player.engine.videoGravity = fill == .fill ? .resizeAspectFill : .resizeAspect
    }
}

nonisolated enum PlayerGeometry {
    /// Where the picture lands in `bounds`. Without an aspect yet, the whole of `bounds`.
    static func videoRect(aspect: CGFloat?, in bounds: CGRect, fill: PlayerVideoFill) -> CGRect {
        guard let aspect, aspect > 0, bounds.width > 0, bounds.height > 0 else { return bounds }
        let boundsAspect = bounds.width / bounds.height
        let widthLimited = aspect > boundsAspect
        var size: CGSize
        switch fill {
        case .fit, .zoom:
            size = widthLimited ? CGSize(width: bounds.width, height: bounds.width / aspect)
                : CGSize(width: bounds.height * aspect, height: bounds.height)
            if fill == .zoom { size = CGSize(width: size.width * PlayerVideoFill.zoomScale, height: size.height * PlayerVideoFill.zoomScale) }
        case .fill:
            size = widthLimited ? CGSize(width: bounds.height * aspect, height: bounds.height)
                : CGSize(width: bounds.width, height: bounds.width / aspect)
        }
        return CGRect(x: bounds.midX - size.width / 2, y: bounds.midY - size.height / 2, width: size.width, height: size.height)
    }
}

/// Draws subtitle cues over a picture laid out at `video` inside `bounds`.
struct SubtitleOverlay: View {
    let cues: [SubtitleCue]
    let video: CGRect
    let bounds: CGRect
    let style: SubtitleStyle
    let lift: CGFloat

    var body: some View {
        // Text stays on screen even where the picture is cropped (fill, zoom).
        let visible = video.intersection(bounds)
        let fontSize = max(15, min(visible.height, visible.width / 1.6) * 0.052 * style.scale)
        let margin = max(visible.height * 0.05, 12)
        let side = max(visible.width * 0.06, 16)
        ZStack {
            ForEach(imageCues, id: \.id) { cue in
                if case .image(let image) = cue.body {
                    let rect = Self.imageRect(image, in: video)
                    Image(decorative: image.cgImage, scale: 1)
                        .resizable()
                        .interpolation(.high)
                        .frame(width: rect.width, height: rect.height)
                        .position(x: rect.midX, y: rect.midY)
                }
            }
            ForEach(Array(textGroups.enumerated()), id: \.offset) { _, group in
                let alignment = group.alignment
                let lines = VStack(spacing: fontSize * 0.15) {
                    ForEach(group.cues, id: \.id) { cue in
                        SubtitleLine(text: Self.attributed(cue), alignment: alignment.text, fontSize: fontSize, style: style)
                    }
                }
                if let anchor = group.anchor {
                    // `\pos` names the point the cue's alignment corner sits on.
                    Color.clear
                        .frame(width: 0, height: 0)
                        .overlay(alignment: alignment.frame) { lines.fixedSize() }
                        .position(x: video.minX + anchor.x * video.width, y: video.minY + anchor.y * video.height)
                } else {
                    lines
                        .frame(width: visible.width - side * 2, height: visible.height - margin * 2 - (alignment.isBottom ? lift : 0),
                               alignment: alignment.frame)
                        .position(x: visible.midX, y: visible.minY + margin + (visible.height - margin * 2 - (alignment.isBottom ? lift : 0)) / 2)
                }
            }
        }
        .frame(width: bounds.width, height: bounds.height)
        .animation(.spring(response: 0.35, dampingFraction: 0.9), value: lift)
        .allowsHitTesting(false)
    }

    private var imageCues: [SubtitleCue] {
        cues.filter { if case .image = $0.body { true } else { false } }
    }

    private struct TextGroup {
        var alignment: CueAlignment
        var anchor: CGPoint?
        var cues: [SubtitleCue]
    }

    /// Cues sharing a placement stack together, in arrival order.
    private var textGroups: [TextGroup] {
        var groups: [TextGroup] = []
        for cue in cues {
            if case .image = cue.body { continue }
            let alignment = CueAlignment(numpad: cue.placement?.alignment ?? 2)
            let anchor = cue.placement?.position
            if let index = groups.firstIndex(where: { $0.alignment == alignment && $0.anchor == anchor }) {
                groups[index].cues.append(cue)
            } else {
                groups.append(TextGroup(alignment: alignment, anchor: anchor, cues: [cue]))
            }
        }
        return groups
    }

    /// A bitmap's rectangle: positions are normalized against its canvas, which is laid width-aligned
    /// and centred on the picture (a cropped rip's canvas can be taller than the picture).
    static func imageRect(_ image: SubtitleImage, in video: CGRect) -> CGRect {
        var canvas = video
        if image.canvasSize.width > 0, image.canvasSize.height > 0 {
            let height = video.width * image.canvasSize.height / image.canvasSize.width
            canvas = CGRect(x: video.minX, y: video.midY - height / 2, width: video.width, height: height)
        }
        let p = image.position
        return CGRect(x: canvas.minX + p.minX * canvas.width, y: canvas.minY + p.minY * canvas.height,
                      width: p.width * canvas.width, height: p.height * canvas.height)
    }

    static func attributed(_ cue: SubtitleCue) -> AttributedString {
        switch cue.body {
        case .text(let text):
            return AttributedString(text)
        case .richText(let runs):
            var result = AttributedString()
            for run in runs {
                var part = AttributedString(run.text)
                if let color = run.color {
                    part.foregroundColor = Color(red: Double(color.r) / 255, green: Double(color.g) / 255, blue: Double(color.b) / 255)
                }
                var intents: InlinePresentationIntent = []
                if run.isBold { intents.insert(.stronglyEmphasized) }
                if run.isItalic { intents.insert(.emphasized) }
                if !intents.isEmpty { part.inlinePresentationIntent = intents }
                if run.isUnderlined { part.underlineStyle = .single }
                if run.isStruckThrough { part.strikethroughStyle = .single }
                result += part
            }
            return result
        case .image:
            return AttributedString()
        }
    }
}

/// ASS numpad alignment: 1 to 3 along the bottom, 4 to 6 the middle, 7 to 9 the top.
private struct CueAlignment: Equatable {
    var vertical: VerticalAlignment
    var horizontal: HorizontalAlignment
    var isBottom: Bool

    init(numpad: Int) {
        let n = (1...9).contains(numpad) ? numpad : 2
        switch n {
        case 1...3: vertical = .bottom; isBottom = true
        case 4...6: vertical = .center; isBottom = false
        default: vertical = .top; isBottom = false
        }
        switch n % 3 {
        case 1: horizontal = .leading
        case 0: horizontal = .trailing
        default: horizontal = .center
        }
    }

    var frame: Alignment { Alignment(horizontal: horizontal, vertical: vertical) }

    var text: TextAlignment {
        switch horizontal {
        case .leading: .leading
        case .trailing: .trailing
        default: .center
        }
    }

    static func == (a: Self, b: Self) -> Bool {
        a.isBottom == b.isBottom && a.text == b.text && a.vertical == b.vertical
    }
}

/// One cue's text, readable on any picture.
private struct SubtitleLine: View {
    let text: AttributedString
    let alignment: TextAlignment
    let fontSize: CGFloat
    let style: SubtitleStyle

    var body: some View {
        let label = Text(text)
            .font(.system(size: fontSize, weight: .semibold))
            .foregroundStyle(.white)
            .multilineTextAlignment(alignment)
            .lineSpacing(fontSize * 0.05)
        switch style.background {
        case .outline:
            // Tight shadows on every side read as an outline; the soft one lifts it off bright scenes.
            let edge = max(1, fontSize / 22)
            label
                .shadow(color: .black, radius: 0, x: edge, y: edge)
                .shadow(color: .black, radius: 0, x: -edge, y: -edge)
                .shadow(color: .black, radius: 0, x: edge, y: -edge)
                .shadow(color: .black, radius: 0, x: -edge, y: edge)
                .shadow(color: .black.opacity(0.6), radius: fontSize / 6)
        case .box:
            label
                .padding(.horizontal, fontSize * 0.35)
                .padding(.vertical, fontSize * 0.1)
                .background(.black.opacity(0.62), in: .rect(cornerRadius: fontSize * 0.2))
        }
    }
}
