//! Scripted input for end-to-end tests (debug builds only).
//!
//! With `PATINA_AUTOMATION=<dir>`, the app reads commands from `<dir>/cmd` (written whole, then
//! renamed into place), runs them in the window as if typed or clicked, deletes the file and
//! writes `<dir>/ack`. This drives the app without focusing it or moving the real pointer.
//! One command per line, positions in window points from the top left:
//!
//! - `click X Y [right|double]`, `move X Y`, `scroll X Y DY` (negative scrolls down)
//! - `type TEXT` types the text; `key KEYSTROKE` presses one key in GPUI syntax (`enter`, `escape`, `cmd-k`, `shift-tab`)
//!
//! GPUI stops drawing a window that is fully covered, so keep it at least partly on screen.

use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::*;

pub fn init(cx: &mut App) {
    let Some(dir) = std::env::var_os("PATINA_AUTOMATION").map(PathBuf::from) else { return };
    cx.spawn(async move |cx| {
        let cmd = dir.join("cmd");
        loop {
            cx.background_executor().timer(Duration::from_millis(50)).await;
            let Ok(script) = std::fs::read_to_string(&cmd) else { continue };
            let _ = std::fs::remove_file(&cmd);
            let result = cx.update(|cx| run(&script, cx));
            let _ = std::fs::write(dir.join("ack"), match result {
                Ok(()) => "ok\n".to_string(),
                Err(e) => format!("error: {e}\n"),
            });
        }
    })
    .detach();
}

fn run(script: &str, cx: &mut App) -> Result<(), String> {
    let window = *cx.windows().first().ok_or("no window")?;
    for line in script.lines().map(str::trim).filter(|l| !l.is_empty()) {
        window
            .update(cx, |_, window, cx| step(line, window, cx))
            .map_err(|e| e.to_string())??;
    }
    Ok(())
}

fn step(line: &str, window: &mut Window, cx: &mut App) -> Result<(), String> {
    let (verb, rest) = line.split_once(' ').unwrap_or((line, ""));
    let args: Vec<&str> = rest.split_whitespace().collect();
    let at = |i: usize| -> Result<Point<Pixels>, String> {
        let n = |s: Option<&&str>| s.and_then(|s| s.parse::<f32>().ok()).ok_or(format!("{verb}: expected X Y"));
        Ok(point(px(n(args.get(i))?), px(n(args.get(i + 1))?)))
    };
    let modifiers = Modifiers::default();
    match verb {
        "move" => {
            window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position: at(0)?, pressed_button: None, modifiers }), cx);
        }
        "click" => {
            let position = at(0)?;
            let button = if args.get(2) == Some(&"right") { MouseButton::Right } else { MouseButton::Left };
            let clicks = if args.get(2) == Some(&"double") { 2 } else { 1 };
            window.dispatch_event(PlatformInput::MouseMove(MouseMoveEvent { position, pressed_button: None, modifiers }), cx);
            for click_count in 1..=clicks {
                window.dispatch_event(PlatformInput::MouseDown(MouseDownEvent { button, position, modifiers, click_count, first_mouse: false }), cx);
                window.dispatch_event(PlatformInput::MouseUp(MouseUpEvent { button, position, modifiers, click_count }), cx);
            }
        }
        "scroll" => {
            let dy: f32 = args.get(2).and_then(|s| s.parse().ok()).ok_or("scroll: expected X Y DY")?;
            window.dispatch_event(
                PlatformInput::Scroll(ScrollEvent {
                    position: at(0)?,
                    delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
                    modifiers,
                    touch_phase: TouchPhase::Moved,
                    source: ScrollSource::default(),
                }),
                cx,
            );
        }
        "type" => {
            for ch in rest.chars() {
                let key = if ch == ' ' { "space".to_string() } else { ch.to_string() };
                let mut keystroke = Keystroke::parse_with_os(&key, cx.platform_info().os).map_err(|e| e.to_string())?;
                keystroke.key_char = Some(ch.to_string());
                window.dispatch_keystroke(keystroke, cx);
            }
        }
        "key" => {
            let keystroke = Keystroke::parse_with_os(rest.trim(), cx.platform_info().os).map_err(|e| e.to_string())?;
            window.dispatch_keystroke(keystroke, cx);
        }
        _ => return Err(format!("unknown command: {line}")),
    }
    window.refresh();
    Ok(())
}
