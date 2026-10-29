// The app's audio session for playback (no desktop twin: mpv opened its own output).
//
// `.playback` keeps audio going with the silent switch on and in the background (the app
// declares the `audio` background mode). AetherEngine declares the category only while nothing
// else has, and does not activate the session on its AVPlayer paths, so the app declares and
// activates it before each load. Activation can block while another app's audio winds down, so it
// runs off the main actor.

import AVFoundation
import os

nonisolated enum AudioSession {
    private static let log = Logger(subsystem: "io.github.madrazaldi.patina", category: "audio-session")

    static func activate(audioOnly: Bool) async {
        await Task.detached(priority: .userInitiated) {
            let session = AVAudioSession.sharedInstance()
            do {
                // The default route-sharing policy: a long-form one leaves Picture in Picture impossible on iOS.
                try session.setCategory(.playback, mode: audioOnly ? .default : .moviePlayback, policy: .default)
                try session.setActive(true)
            } catch {
                log.error("could not activate the audio session: \(error.localizedDescription, privacy: .public)")
            }
        }.value
    }

    /// Lets other apps' audio resume once the player is closed.
    static func deactivate() {
        Task.detached(priority: .utility) {
            try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
        }
    }
}
