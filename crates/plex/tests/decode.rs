use patina_plex::{Envelope, Resource};

#[test]
fn tolerates_plex_type_quirks() {
    // ids as numbers or strings, booleans as 0/1/"1", unknown fields kept in `extra`.
    let json = r#"{"MediaContainer":{"size":"1","totalSize":40,"offset":0,"allowSync":1,"friendlyName":"Den",
      "Metadata":[{"ratingKey":123,"type":"episode","title":"Pilot","index":"1","parentIndex":2,"viewOffset":"60000","duration":120000,
        "grandparentTitle":"Show","someNewField":{"x":1},
        "Media":[{"id":"7","optimizedForStreaming":"1","videoResolution":1080,"Part":[{"id":8,"key":"/library/parts/8/1/file.mkv",
          "Stream":[{"id":9,"streamType":2,"index":1,"selected":true,"DOVIPresent":0},{"id":10,"streamType":3,"key":"/library/streams/10","selected":"1"}]}]}],
        "Marker":[{"type":"intro","startTimeOffset":1000,"endTimeOffset":30000,"final":"1"}]}]}}"#;
    let mc = serde_json::from_str::<Envelope>(json).unwrap().media_container;
    assert_eq!(mc.size, Some(1));
    assert!(mc.has_more());
    assert_eq!(mc.attr("friendlyName").as_deref(), Some("Den"));
    let m = &mc.metadata[0];
    assert_eq!(m.rating_key(), "123");
    assert_eq!(m.episode_code(), "S02E01");
    assert_eq!(m.progress(), Some(0.5));
    assert!(m.extra.contains_key("someNewField"));
    let media = &m.media[0];
    assert!(media.optimized_for_streaming);
    assert_eq!(media.video_resolution.as_deref(), Some("1080"));
    let part = &media.parts[0];
    assert_eq!(part.selected_audio().and_then(|s| s.id), Some(9));
    assert!(part.selected_subtitle().unwrap().is_external());
    assert!(m.markers[0].is_final);
}

#[test]
fn ranks_connections() {
    let r: Resource = serde_json::from_str(r#"{"name":"S","provides":"server","connections":[
        {"protocol":"https","address":"1.2.3.4","port":32400,"uri":"https://relay.plex.direct:8443","local":false,"relay":true},
        {"protocol":"https","address":"192.168.1.5","port":32400,"uri":"https://192-168-1-5.abc.plex.direct:32400","local":true,"relay":false}]}"#).unwrap();
    assert!(r.is_server());
    let c = patina_plex::ConnectionCandidate::candidates(&r);
    assert!(c[0].local && !c[0].relay);
    assert!(c.iter().any(|c| c.uri == "http://192.168.1.5:32400"));
    assert!(c.last().unwrap().relay);
}

#[test]
fn accepts_single_object_where_a_list_is_expected() {
    // Real servers send `OnDeck.Metadata` as an object, not an array.
    let json = r#"{"MediaContainer":{"Metadata":[{"ratingKey":"2606","type":"show","title":"Show",
      "OnDeck":{"Metadata":{"ratingKey":"2610","type":"episode","title":"Next"}},
      "Related":{"Hub":[{"hubIdentifier":"similar","title":"Similar","Metadata":[]}]},
      "Extras":{"size":0}}]}}"#;
    let mc = serde_json::from_str::<Envelope>(json).unwrap().media_container;
    let m = &mc.metadata[0];
    assert_eq!(m.on_deck_item().map(|e| e.rating_key()), Some("2610"));
    assert_eq!(m.related.as_ref().map(|r| r.hub.len()), Some(1));
}
