// The selected ASS track drawn by libass over the picture (no desktop twin: mpv drew it).
//
// AetherEngine's clock ticks about ten times a second, too coarse for moving signs and fades, so
// the time is carried forward from the last tick on every display refresh while playing. A frame
// is rendered off the main thread, one at a time; a refresh that finds one in flight skips.

import AetherEngine
import Observation
import QuartzCore
import SwiftUI

@Observable
public final class ASSSubtitles {
    /// Whether an ASS track is being drawn here (the text overlay leaves its cues alone then).
    public private(set) var isActive = false
    /// The latest frame, nil when nothing shows.
    private(set) var frame: ASSFrame?

    @ObservationIgnored private var renderer: ASSRenderer?
    @ObservationIgnored private var header: String?
    @ObservationIgnored private var fedCueIDs: Set<Int> = []
    @ObservationIgnored private var generation = 0

    @ObservationIgnored private var anchor: (seconds: Double, at: CFTimeInterval) = (0, 0)
    @ObservationIgnored private var playing = false
    @ObservationIgnored private var rate = 1.0
    @ObservationIgnored private var lastSeconds: Double?

    @ObservationIgnored private var canvas = (width: 0, height: 0)
    @ObservationIgnored private var storage: CGSize?
    @ObservationIgnored private var lastRender: (milliseconds: Int64, width: Int, height: Int)?
    @ObservationIgnored private var rendering = false
    @ObservationIgnored private var surfaces = 0
    @ObservationIgnored private var displayLink: CADisplayLink?

    init() {}

    // MARK: Fed by the engine

    /// Draws the track whose script header is `header`, or stops drawing with nil.
    func setScript(_ header: String?, fonts: @autoclosure () -> [FontAttachment]) {
        guard header != self.header else { return }
        self.header = header
        generation += 1
        renderer = header.map { ASSRenderer(header: $0, fonts: fonts()) }
        fedCueIDs = []
        lastRender = nil
        frame = nil
        isActive = header != nil
        updateDisplayLink()
    }

    /// Hands libass the cues it has not seen; the engine republishes its whole window each time.
    func add(_ cues: [SubtitleCue]) {
        guard let renderer else { return }
        var events: [ASSEvent] = []
        for cue in cues where fedCueIDs.insert(cue.id).inserted {
            if case .text(let text) = cue.body { events.append(ASSEvent(text: text, start: cue.startTime, end: cue.endTime)) }
        }
        if !events.isEmpty {
            renderer.add(events)
            lastRender = nil
        }
    }

    /// The source time of the picture on screen, at each engine tick.
    func setTime(_ seconds: Double) {
        anchor = (seconds, CACurrentMediaTime())
        // A jump back (a seek) must not be held off by the forward-only smoothing in `now`.
        if let lastSeconds, seconds < lastSeconds - 0.25 { self.lastSeconds = nil }
    }

    func setPlaying(_ playing: Bool, rate: Double) {
        anchor = (now(), CACurrentMediaTime())
        self.playing = playing
        self.rate = rate
    }

    /// The coded size of the picture, which libass scales borders, blur and 3D against.
    func setStorage(_ size: CGSize?) {
        guard size != storage else { return }
        storage = size
        lastRender = nil
    }

    // MARK: Driven by the surface

    func attach() {
        surfaces += 1
        updateDisplayLink()
    }

    func detach() {
        surfaces -= 1
        updateDisplayLink()
    }

    func setCanvas(width: Int, height: Int) {
        guard canvas != (width, height) else { return }
        canvas = (width, height)
        render()
    }

    // MARK: Rendering

    private func now() -> Double {
        guard playing else { return anchor.seconds }
        // Past half a second without a tick the clock has stalled; don't run ahead of the picture.
        let elapsed = min(max(0, CACurrentMediaTime() - anchor.at), 0.5)
        return anchor.seconds + elapsed * rate
    }

    private func render() {
        guard let renderer, !rendering, canvas.width > 0, canvas.height > 0 else { return }
        var seconds = now()
        // A tick lands a few milliseconds behind the time carried forward; don't step back for it.
        if playing, let lastSeconds, seconds < lastSeconds { seconds = lastSeconds }
        lastSeconds = seconds
        let milliseconds = Int64((seconds * 1000).rounded())
        if let lastRender, lastRender == (milliseconds, canvas.width, canvas.height) { return }
        lastRender = (milliseconds, canvas.width, canvas.height)
        rendering = true
        let generation = generation
        renderer.render(milliseconds: milliseconds, width: canvas.width, height: canvas.height, storage: storage) { [weak self] output in
            Task { @MainActor [weak self] in
                guard let self else { return }
                rendering = false
                guard generation == self.generation, output.changed else { return }
                frame = output.frame
            }
        }
    }

    private func updateDisplayLink() {
        let wanted = isActive && surfaces > 0
        if wanted, displayLink == nil {
            let link = CADisplayLink(target: DisplayLinkTarget { [weak self] in self?.render() }, selector: #selector(DisplayLinkTarget.fire))
            link.preferredFrameRateRange = CAFrameRateRange(minimum: 24, maximum: 60, preferred: 60)
            link.add(to: .main, forMode: .common)
            displayLink = link
        } else if !wanted, let link = displayLink {
            link.invalidate()
            displayLink = nil
        }
    }

    private final class DisplayLinkTarget: NSObject {
        let tick: () -> Void
        init(_ tick: @escaping () -> Void) { self.tick = tick }
        @objc func fire() { tick() }
    }
}

/// Draws the frames of `subtitles` over a picture laid out at `video` inside `bounds`.
struct ASSSubtitleLayer: View {
    let subtitles: ASSSubtitles
    let video: CGRect
    let bounds: CGRect
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        ZStack {
            // A frame rendered for an older canvas (the fill mode changing) stretches until the next one.
            if let frame = subtitles.frame, frame.canvas.width > 0, frame.canvas.height > 0 {
                let sx = video.width / frame.canvas.width, sy = video.height / frame.canvas.height
                Image(decorative: frame.image, scale: 1)
                    .resizable()
                    .frame(width: frame.rect.width * sx, height: frame.rect.height * sy)
                    .position(x: video.minX + frame.rect.midX * sx, y: video.minY + frame.rect.midY * sy)
            }
        }
        .frame(width: bounds.width, height: bounds.height)
        .allowsHitTesting(false)
        .onAppear { subtitles.attach() }
        .onDisappear { subtitles.detach() }
        .onChange(of: canvasSize, initial: true) { subtitles.setCanvas(width: canvasSize.width, height: canvasSize.height) }
    }

    private var canvasSize: CanvasSize {
        CanvasSize(width: Int((video.width * displayScale).rounded()), height: Int((video.height * displayScale).rounded()))
    }

    private struct CanvasSize: Equatable {
        var width: Int
        var height: Int
    }
}
