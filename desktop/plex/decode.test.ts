// Port of crates/plex/tests/decode.rs.

import { expect, test } from "bun:test";

import {
  ConnectionCandidate,
  Media,
  MediaContainer,
  defaultMedia,
  Metadata,
  Part,
  Stream,
  normalizeEnvelope,
  normalizeResource,
  Resource,
} from "./index";

function envelope(json: string): MediaContainer {
  return normalizeEnvelope(JSON.parse(json)).MediaContainer;
}

test("tolerates_plex_type_quirks", () => {
  // ids as numbers or strings, booleans as 0/1/"1", unknown fields kept in `extra`.
  const json = `{"MediaContainer":{"size":"1","totalSize":40,"offset":0,"allowSync":1,"friendlyName":"Den",
      "Metadata":[{"ratingKey":123,"type":"episode","title":"Pilot","index":"1","parentIndex":2,"viewOffset":"60000","duration":120000,
        "grandparentTitle":"Show","someNewField":{"x":1},
        "Media":[{"id":"7","optimizedForStreaming":"1","videoResolution":1080,"Part":[{"id":8,"key":"/library/parts/8/1/file.mkv",
          "Stream":[{"id":9,"streamType":2,"index":1,"selected":true,"DOVIPresent":0},{"id":10,"streamType":3,"key":"/library/streams/10","selected":"1"}]}]}],
        "Marker":[{"type":"intro","startTimeOffset":1000,"endTimeOffset":30000,"final":"1"}]}]}}`;
  const mc = envelope(json);
  expect(mc.size).toBe(1);
  expect(MediaContainer.hasMore(mc)).toBe(true);
  expect(MediaContainer.attr(mc, "friendlyName")).toBe("Den");
  const m = mc.Metadata[0]!;
  expect(Metadata.ratingKey(m)).toBe("123");
  expect(Metadata.episodeCode(m)).toBe("S02E01");
  expect(Metadata.progress(m)).toBe(0.5);
  expect("someNewField" in Metadata.extra(m)).toBe(true);
  const media = m.Media[0]!;
  expect(media.optimizedForStreaming).toBe(true);
  expect(media.videoResolution).toBe("1080");
  const part = media.Part[0]!;
  expect(Part.selectedAudio(part)?.id).toBe(9);
  expect(Stream.isExternal(Part.selectedSubtitle(part)!)).toBe(true);
  expect(Metadata.markersOf(m, "intro")[0]!.final).toBe(true);
});

test("ranks_connections", () => {
  const r = normalizeResource(
    JSON.parse(`{"name":"S","provides":"server","connections":[
        {"protocol":"https","address":"1.2.3.4","port":32400,"uri":"https://relay.plex.direct:8443","local":false,"relay":true},
        {"protocol":"https","address":"192.168.1.5","port":32400,"uri":"https://192-168-1-5.abc.plex.direct:32400","local":true,"relay":false}]}`),
  );
  expect(Resource.isServer(r)).toBe(true);
  const c = ConnectionCandidate.candidates(r);
  expect(c[0]!.local && !c[0]!.relay).toBe(true);
  expect(c.some((x) => x.uri === "http://192.168.1.5:32400")).toBe(true);
  expect(c[c.length - 1]!.relay).toBe(true);
});

test("accepts_single_object_where_a_list_is_expected", () => {
  // Real servers send `OnDeck.Metadata` as an object, not an array.
  const json = `{"MediaContainer":{"Metadata":[{"ratingKey":"2606","type":"show","title":"Show",
      "OnDeck":{"Metadata":{"ratingKey":"2610","type":"episode","title":"Next"}},
      "Related":{"Hub":[{"hubIdentifier":"similar","title":"Similar","Metadata":[]}]},
      "Extras":{"size":0}}]}}`;
  const mc = envelope(json);
  const m = mc.Metadata[0]!;
  const next = Metadata.onDeckItem(m);
  expect(next === null ? null : Metadata.ratingKey(next)).toBe("2610");
  expect(m.Related === null ? null : m.Related.Hub.length).toBe(1);
});

// ---- additions: the serialized shape the UI and downloads.json rely on ----

test("serialized shape: nulls for absent options, empty lists, false bools, flattened extras", () => {
  const m = envelope(`{"MediaContainer":{"Metadata":[{"ratingKey":5,"type":"movie","collectionSort":"1","accessToken":"secret"}]}}`)
    .Metadata[0]!;
  const out = JSON.parse(JSON.stringify(m)) as Record<string, unknown>;
  expect(out.ratingKey).toBe("5");
  expect(out.type).toBe("movie");
  expect(out.title).toBeNull();
  expect(out.Media).toEqual([]);
  expect(out.smart).toBe(false);
  expect(out.collectionSort).toBe("1");
  expect(out.User).toBeNull();
  expect(out.Extras).toBeNull();
  expect(Media.label(defaultMedia())).toBe("");
});

test("server tag round trips through extra", () => {
  const m = envelope(`{"MediaContainer":{"Metadata":[{"ratingKey":"1"}]}}`).Metadata[0]!;
  expect(Metadata.serverId(m)).toBeNull();
  Metadata.setServerId(m, "abc");
  expect(Metadata.serverId(m)).toBe("abc");
  expect(JSON.parse(JSON.stringify(m)).patinaServer).toBe("abc");
  Metadata.setServerId(m, null);
  expect(Metadata.serverId(m)).toBeNull();
  expect("patinaServer" in m).toBe(false);
});

test("media label", () => {
  const m = envelope(
    `{"MediaContainer":{"Metadata":[{"Media":[{"videoResolution":"4k","videoCodec":"hevc","audioCodec":"truehd","audioChannels":8,
      "Part":[{"Stream":[{"streamType":1,"DOVIPresent":1}]}]}]}]}}`,
  ).Metadata[0]!.Media[0]!;
  expect(Media.label(m)).toBe("4K · HEVC · Dolby Vision · TRUEHD 7.1");
});

test("decode errors name the path", () => {
  expect(() => envelope(`{"MediaContainer":{"Metadata":[{"Media":"x"}]}}`)).toThrow();
  try {
    envelope(`{"MediaContainer":{"Metadata":[{"Media":"x"}]}}`);
  } catch (e) {
    expect((e as Error).message).toBe("MediaContainer.Metadata[0].Media: invalid type: string \"x\", expected a sequence");
  }
});
