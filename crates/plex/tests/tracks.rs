use patina_plex::{Part, Stream, stream_type};

fn audio(id: i64, lang: &str, codec: &str, channels: i64, title: Option<&str>) -> Stream {
    Stream {
        id: Some(id),
        stream_type: Some(stream_type::AUDIO),
        language_code: Some(lang.into()),
        codec: Some(codec.into()),
        channels: Some(channels),
        title: title.map(Into::into),
        index: Some(id),
        ..Default::default()
    }
}

fn sub(id: i64, lang: &str, forced: bool, sdh: bool, title: Option<&str>) -> Stream {
    Stream {
        id: Some(id),
        stream_type: Some(stream_type::SUBTITLE),
        language_code: Some(lang.into()),
        codec: Some("srt".into()),
        forced,
        hearing_impaired: sdh,
        title: title.map(Into::into),
        index: Some(id),
        ..Default::default()
    }
}

fn part(streams: Vec<Stream>) -> Part {
    Part { id: Some(1), streams, ..Default::default() }
}

fn matched(p: &Part, template: &Stream) -> Option<i64> {
    p.matching_stream(template).and_then(|s| s.id)
}

#[test]
fn audio_requires_language_and_prefers_closest() {
    let template = audio(0, "jpn", "flac", 2, None);
    let p = part(vec![audio(1, "eng", "flac", 2, None), audio(2, "jpn", "aac", 6, None), audio(3, "jpn", "flac", 2, None)]);
    assert_eq!(matched(&p, &template), Some(3));

    let english_only = part(vec![audio(1, "eng", "flac", 2, None)]);
    assert_eq!(matched(&english_only, &template), None);
}

#[test]
fn title_outweighs_codec() {
    let template = audio(0, "eng", "ac3", 6, Some("Commentary"));
    let p = part(vec![audio(1, "eng", "ac3", 6, None), audio(2, "eng", "aac", 2, Some("Commentary"))]);
    assert_eq!(matched(&p, &template), Some(2));
}

#[test]
fn subtitles_keep_forced_apart() {
    let full = sub(0, "eng", false, false, None);
    let forced = sub(0, "eng", true, false, Some("Signs"));
    let p = part(vec![sub(1, "eng", true, false, Some("Signs")), sub(2, "eng", false, true, None), sub(3, "eng", false, false, None)]);
    assert_eq!(matched(&p, &full), Some(3));
    assert_eq!(matched(&p, &forced), Some(1));

    // Only forced subs available: a full-subtitle choice finds nothing.
    let only_forced = part(vec![sub(1, "eng", true, false, None)]);
    assert_eq!(matched(&only_forced, &full), None);
}

#[test]
fn ties_go_to_the_first_stream() {
    let template = audio(0, "eng", "aac", 2, None);
    let p = part(vec![audio(5, "eng", "aac", 2, None), audio(6, "eng", "aac", 2, None)]);
    assert_eq!(matched(&p, &template), Some(5));
}
