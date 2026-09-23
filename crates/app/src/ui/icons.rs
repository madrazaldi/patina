//! Inline SVG icons (Lucide-style strokes).

use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ic {
    Home,
    Search,
    Film,
    Tv,
    Music,
    Image,
    Folder,
    List,
    Bookmark,
    BookmarkCheck,
    Activity,
    Server,
    Sliders,
    Play,
    Pause,
    Shuffle,
    Check,
    CheckCircle,
    Circle,
    Plus,
    X,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    More,
    Refresh,
    Trash,
    User,
    Users,
    LogOut,
    Star,
    Clock,
    Restart,
    SkipForward,
    SkipBack,
    Square,
    Layers,
    Download,
    Sort,
    Info,
    Zap,
    Wrench,
    ExternalLink,
    Sparkles,
    Alert,
    Subtitles,
    Volume,
    Eye,
    Link,
}

#[component]
pub fn Icon(name: Ic, #[props(default)] class: String, #[props(default)] filled: bool) -> Element {
    let cls = format!("icon {class}{}", if filled { " filled" } else { "" });
    rsx! {
        svg { class: "{cls}", view_box: "0 0 24 24", xmlns: "http://www.w3.org/2000/svg",
            {paths(name)}
        }
    }
}

fn paths(name: Ic) -> Element {
    match name {
        Ic::Home => rsx! {
            path { d: "M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8" }
            path { d: "M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" }
        },
        Ic::Search => rsx! {
            circle { cx: "11", cy: "11", r: "8" }
            path { d: "m21 21-4.3-4.3" }
        },
        Ic::Film => rsx! {
            rect { width: "18", height: "18", x: "3", y: "3", rx: "2" }
            path { d: "M7 3v18M3 7.5h4M3 12h18M3 16.5h4M17 3v18M17 7.5h4M17 16.5h4" }
        },
        Ic::Tv => rsx! {
            rect { width: "20", height: "15", x: "2", y: "7", rx: "2", ry: "2" }
            polyline { points: "17 2 12 7 7 2" }
        },
        Ic::Music => rsx! {
            path { d: "M9 18V5l12-2v13" }
            circle { cx: "6", cy: "18", r: "3" }
            circle { cx: "18", cy: "16", r: "3" }
        },
        Ic::Image => rsx! {
            rect { width: "18", height: "18", x: "3", y: "3", rx: "2", ry: "2" }
            circle { cx: "9", cy: "9", r: "2" }
            path { d: "m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" }
        },
        Ic::Folder => rsx! {
            path { d: "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" }
        },
        Ic::List => rsx! {
            path { d: "M3 12h.01M3 18h.01M3 6h.01M8 12h13M8 18h13M8 6h13" }
        },
        Ic::Bookmark => rsx! {
            path { d: "m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z" }
        },
        Ic::BookmarkCheck => rsx! {
            path { d: "m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2Z" }
            path { d: "m9 10 2 2 4-4" }
        },
        Ic::Activity => rsx! {
            path { d: "M22 12h-4l-3 9L9 3l-3 9H2" }
        },
        Ic::Server => rsx! {
            rect { width: "20", height: "8", x: "2", y: "2", rx: "2", ry: "2" }
            rect { width: "20", height: "8", x: "2", y: "14", rx: "2", ry: "2" }
            path { d: "M6 6h.01M6 18h.01" }
        },
        Ic::Sliders => rsx! {
            path { d: "M21 4h-7M10 4H3M21 12h-9M8 12H3M21 20h-5M12 20H3M14 2v4M8 10v4M16 18v4" }
        },
        Ic::Play => rsx! {
            polygon { points: "6 3 20 12 6 21 6 3", fill: "currentColor" }
        },
        Ic::Pause => rsx! {
            rect { x: "14", y: "4", width: "4", height: "16", rx: "1", fill: "currentColor" }
            rect { x: "6", y: "4", width: "4", height: "16", rx: "1", fill: "currentColor" }
        },
        Ic::Shuffle => rsx! {
            path { d: "m18 14 4 4-4 4M18 2l4 4-4 4" }
            path { d: "M2 18h1.973a4 4 0 0 0 3.3-1.7l5.454-7.6a4 4 0 0 1 3.3-1.7H22" }
            path { d: "M2 6h1.972a4 4 0 0 1 3.6 2.2M22 18h-6.041a4 4 0 0 1-3.3-1.8l-.359-.45" }
        },
        Ic::Check => rsx! {
            path { d: "M20 6 9 17l-5-5" }
        },
        Ic::CheckCircle => rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "m9 12 2 2 4-4" }
        },
        Ic::Circle => rsx! {
            circle { cx: "12", cy: "12", r: "10" }
        },
        Ic::Plus => rsx! {
            path { d: "M5 12h14M12 5v14" }
        },
        Ic::X => rsx! {
            path { d: "M18 6 6 18M6 6l12 12" }
        },
        Ic::ChevronLeft => rsx! {
            path { d: "m15 18-6-6 6-6" }
        },
        Ic::ChevronRight => rsx! {
            path { d: "m9 18 6-6-6-6" }
        },
        Ic::ChevronDown => rsx! {
            path { d: "m6 9 6 6 6-6" }
        },
        Ic::More => rsx! {
            circle { cx: "12", cy: "12", r: "1.4", fill: "currentColor" }
            circle { cx: "19", cy: "12", r: "1.4", fill: "currentColor" }
            circle { cx: "5", cy: "12", r: "1.4", fill: "currentColor" }
        },
        Ic::Refresh => rsx! {
            path { d: "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" }
            path { d: "M21 3v5h-5" }
            path { d: "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" }
            path { d: "M8 16H3v5" }
        },
        Ic::Trash => rsx! {
            path { d: "M3 6h18M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" }
        },
        Ic::User => rsx! {
            path { d: "M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2" }
            circle { cx: "12", cy: "7", r: "4" }
        },
        Ic::Users => rsx! {
            path { d: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" }
            circle { cx: "9", cy: "7", r: "4" }
            path { d: "M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75" }
        },
        Ic::LogOut => rsx! {
            path { d: "M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" }
            polyline { points: "16 17 21 12 16 7" }
            path { d: "M21 12H9" }
        },
        Ic::Star => rsx! {
            polygon { points: "12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2" }
        },
        Ic::Clock => rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            polyline { points: "12 6 12 12 16 14" }
        },
        Ic::Restart => rsx! {
            path { d: "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" }
            path { d: "M3 3v5h5" }
        },
        Ic::SkipForward => rsx! {
            polygon { points: "5 4 15 12 5 20 5 4", fill: "currentColor" }
            path { d: "M19 5v14" }
        },
        Ic::SkipBack => rsx! {
            polygon { points: "19 20 9 12 19 4 19 20", fill: "currentColor" }
            path { d: "M5 19V5" }
        },
        Ic::Square => rsx! {
            rect { x: "5", y: "5", width: "14", height: "14", rx: "2", fill: "currentColor" }
        },
        Ic::Layers => rsx! {
            path { d: "m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z" }
            path { d: "m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65M22 12.65l-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65" }
        },
        Ic::Download => rsx! {
            path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
            polyline { points: "7 10 12 15 17 10" }
            path { d: "M12 15V3" }
        },
        Ic::Sort => rsx! {
            path { d: "m21 16-4 4-4-4M17 20V4M3 8l4-4 4 4M7 4v16" }
        },
        Ic::Info => rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M12 16v-4M12 8h.01" }
        },
        Ic::Zap => rsx! {
            polygon { points: "13 2 3 14 12 14 11 22 21 10 12 10 13 2" }
        },
        Ic::Wrench => rsx! {
            path { d: "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" }
        },
        Ic::ExternalLink => rsx! {
            path { d: "M15 3h6v6M10 14 21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" }
        },
        Ic::Sparkles => rsx! {
            path { d: "M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z" }
        },
        Ic::Alert => rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M12 8v4M12 16h.01" }
        },
        Ic::Subtitles => rsx! {
            rect { width: "18", height: "14", x: "3", y: "5", rx: "2" }
            path { d: "M7 15h4M15 15h2M7 11h2M13 11h4" }
        },
        Ic::Volume => rsx! {
            polygon { points: "11 5 6 9 2 9 2 15 6 15 11 19 11 5" }
            path { d: "M15.54 8.46a5 5 0 0 1 0 7.07M19.07 4.93a10 10 0 0 1 0 14.14" }
        },
        Ic::Eye => rsx! {
            path { d: "M2 12s3-7 10-7 10 7 10 7-3 7-10 7-10-7-10-7Z" }
            circle { cx: "12", cy: "12", r: "3" }
        },
        Ic::Link => rsx! {
            path { d: "M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" }
            path { d: "M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" }
        },
    }
}

/// Icon for a library section type.
pub fn section_icon(kind: &str) -> Ic {
    match kind {
        "movie" => Ic::Film,
        "show" => Ic::Tv,
        "artist" => Ic::Music,
        "photo" => Ic::Image,
        _ => Ic::Folder,
    }
}
