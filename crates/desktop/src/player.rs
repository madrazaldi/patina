//! Playback chrome: the now-playing bar, the playback-failure and resume banners.

use gpui_kit::*;

/// The bar at the bottom of the window while something plays.
pub fn now_playing_bar(_window: &mut Window, _cx: &mut App) -> AnyElement {
    div().into_any_element()
}

/// Why playback failed, with ways to recover.
pub fn failure_banner(_window: &mut Window, _cx: &mut App) -> AnyElement {
    div().into_any_element()
}

/// The offer to resume the queue that was playing when Patina last stopped.
pub fn resume_banner(_window: &mut Window, _cx: &mut App) -> AnyElement {
    div().into_any_element()
}
