// Request shapes of the server client against a fake server: paths, query parameters and headers
// are the ones the Rust implementation sends.

import { afterAll, beforeAll, expect, test } from "bun:test";

import {
  ClientInfo,
  LibraryQuery,
  MetadataOptions,
  Params,
  PlayQueueRequest,
  PlexError,
  PlexServer,
  PlexTv,
  TimelineState,
  TranscodeOptions,
  connectResource,
  defaultConnection,
  defaultResource,
} from "./index";

interface Seen {
  method: string;
  path: string;
  query: [string, string][];
  headers: Headers;
}

let seen: Seen[] = [];
let server: ReturnType<typeof Bun.serve>;
let base: string;

const info = new ClientInfo("test-client", "Patina", "1.2.3");

beforeAll(() => {
  server = Bun.serve({
    port: 0,
    async fetch(req) {
      const url = new URL(req.url);
      seen.push({
        method: req.method,
        path: url.pathname,
        query: [...url.searchParams.entries()],
        headers: req.headers,
      });
      switch (url.pathname) {
        case "/identity":
          return Response.json({ MediaContainer: { machineIdentifier: "m-1", version: "1.0", claimed: 1 } });
        case "/unauthorized":
          return new Response("nope", { status: 401 });
        case "/missing":
          return new Response("no such item", { status: 404 });
        case "/html":
          return new Response("<html>bad gateway</html>", { status: 502 });
        case "/empty":
          return new Response("", { status: 200 });
        case "/library/sections/1/all":
          return Response.json({
            MediaContainer: { size: 1, totalSize: 3, offset: 0, Metadata: [{ ratingKey: 7, type: "movie", title: "A" }] },
          });
        case "/playQueues":
          return Response.json({
            MediaContainer: { playQueueID: 9, playQueueSelectedItemID: 11, Metadata: [{ ratingKey: "7", playQueueItemID: 11 }] },
          });
        case "/image":
          return new Response(new Uint8Array([1, 2, 3]), { headers: { "content-type": "image/jpeg" } });
        case "/range":
          return new Response("x");
        case "/:/eventsource/notifications":
          return new Response(
            'event: playing\ndata: {"a":1}\n\n: comment\ndata: plain\n\nevent: x\n\nevent: status\ndata: l1\ndata: l2\n\n',
            { headers: { "content-type": "text/event-stream" } },
          );
        default:
          return Response.json({ MediaContainer: {} });
      }
    },
  });
  base = `http://127.0.0.1:${server.port}`;
});

afterAll(() => {
  server.stop(true);
});

function plex(): PlexServer {
  seen = [];
  return new PlexServer(info, base, "tok-123");
}

function last(): Seen {
  return seen[seen.length - 1]!;
}

function pairs(s: Seen): string[] {
  return s.query.map(([k, v]) => `${k}=${v}`);
}

test("library items query", async () => {
  const s = plex();
  const mc = await s.sectionItems("1", {
    kind: 4,
    sort: "addedAt:desc",
    unwatched: true,
    filters: new Params().set("genre", 12),
    start: 0,
    size: 50,
  });
  expect(mc.Metadata[0]!.ratingKey).toBe("7");
  const r = last();
  expect(r.method).toBe("GET");
  expect(r.path).toBe("/library/sections/1/all");
  expect(pairs(r)).toEqual([
    "type=4",
    "sort=addedAt:desc",
    "unwatched=1",
    "genre=12",
    "includeGuids=1",
    "X-Plex-Container-Start=0",
    "X-Plex-Container-Size=50",
  ]);
  expect(r.headers.get("x-plex-token")).toBe("tok-123");
  expect(r.headers.get("x-plex-client-identifier")).toBe("test-client");
  expect(r.headers.get("x-plex-product")).toBe("Patina");
  expect(r.headers.get("x-plex-version")).toBe("1.2.3");
  expect(r.headers.get("x-plex-model")).toBe("desktop");
  expect(r.headers.get("x-plex-provides")).toBe("player");
  expect(r.headers.get("accept")).toBe("application/json");
  // the token travels in a header, not in the URL
  expect(r.query.some(([k]) => k === "X-Plex-Token")).toBe(false);
});

test("library query without size or filters", async () => {
  const s = plex();
  await s.sectionItems("2", LibraryQuery.page(40, 0));
  expect(pairs(last())).toEqual(["includeGuids=1", "X-Plex-Container-Start=40"]);
});

test("timeline report", async () => {
  const s = plex();
  await s.timeline({
    ratingKey: "7",
    key: "/library/metadata/7",
    state: TimelineState.Playing,
    timeMs: -5,
    durationMs: 120000,
    playQueueItemId: 11,
    session: "sess",
  });
  const r = last();
  expect(r.method).toBe("GET");
  expect(r.path).toBe("/:/timeline");
  expect(pairs(r)).toEqual([
    "ratingKey=7",
    "key=/library/metadata/7",
    "state=playing",
    "time=0",
    "duration=120000",
    "identifier=com.plexapp.plugins.library",
    "hasMDE=1",
    "playQueueItemID=11",
    "X-Plex-Session-Identifier=sess",
  ]);
});

test("select streams", async () => {
  const s = plex();
  await s.selectStreams(55, 3, 0, true);
  const r = last();
  expect(r.method).toBe("PUT");
  expect(r.path).toBe("/library/parts/55");
  expect(pairs(r)).toEqual(["audioStreamID=3", "subtitleStreamID=0", "allParts=1"]);

  await s.selectStreams(55, null, null, false);
  expect(last().query).toEqual([]);
});

test("play queue create", async () => {
  const s = plex();
  const mc = await s.createPlayQueue(
    new PlayQueueRequest({
      uri: "server://m/com.plexapp.plugins.library/library/metadata/7",
      kind: "video",
      key: "/library/metadata/7",
      continuous: true,
      includeChapters: true,
    }),
  );
  expect(mc.playQueueID).toBe(9);
  const r = last();
  expect(r.method).toBe("POST");
  expect(r.path).toBe("/playQueues");
  expect(pairs(r)).toEqual([
    "type=video",
    "uri=server://m/com.plexapp.plugins.library/library/metadata/7",
    "key=/library/metadata/7",
    "shuffle=0",
    "repeat=0",
    "continuous=1",
    "includeChapters=1",
    "own=1",
  ]);
});

test("metadata options", async () => {
  const s = plex();
  await s.metadata("7", MetadataOptions.full());
  expect(last().path).toBe("/library/metadata/7");
  expect(pairs(last())).toEqual([
    "includeExtras=1",
    "includeRelated=1",
    "includeMarkers=1",
    "includeChapters=1",
    "includeOnDeck=1",
    "includeReviews=1",
    "includeGuids=1",
    "asyncAugmentMetadata=0",
  ]);
});

test("paths with a colon and encoded values", async () => {
  const s = plex();
  await s.scrobble("7");
  expect(last().path).toBe("/:/scrobble");
  expect(pairs(last())).toEqual(["key=7", "identifier=com.plexapp.plugins.library"]);
  await s.searchHubs("a b&c", 5, null);
  expect(last().path).toBe("/hubs/search");
  expect(last().query[0]).toEqual(["query", "a b&c"]);
  await s.lineupChannels("/tv.plex.providers.epg.cloud:2/", "lineup://x");
  expect(last().path).toBe("/tv.plex.providers.epg.cloud:2/lineups/dvr/channels");
});

test("edit metadata and tags", async () => {
  const s = plex();
  await s.editMetadata("7", 3, 1, [["title", "New"]], true);
  expect(last().method).toBe("PUT");
  expect(last().path).toBe("/library/sections/3/all");
  expect(pairs(last())).toEqual(["type=1", "id=7", "title.value=New", "title.locked=1"]);
  await s.editTags("7", 1, "genre", ["A", "B"], ["C", "D"]);
  expect(last().path).toBe("/library/metadata/7");
  expect(pairs(last())).toEqual(["type=1", "id=7", "genre[0].tag.tag=A", "genre[1].tag.tag=B", "genre[].tag.tag-=C,D"]);
});

test("transcode url carries the token and parameters", () => {
  const s = new PlexServer(info, base, "tok-123");
  const url = s.transcodeUrl("/library/metadata/7", new TranscodeOptions({ session: "abc", maxVideoBitrate: 4000, offsetSecs: 12.5 }));
  expect(url.pathname).toBe("/video/:/transcode/universal/start.m3u8");
  expect(url.searchParams.get("X-Plex-Token")).toBe("tok-123");
  expect(url.searchParams.get("offset")).toBe("12.500");
  expect(url.searchParams.get("maxVideoBitrate")).toBe("4000");
  expect(url.searchParams.get("videoBitrate")).toBe("4000");
  expect(url.searchParams.get("session")).toBe("abc");
  expect(url.searchParams.get("directStream")).toBe("1");
  expect(url.searchParams.get("X-Plex-Platform")).toBe("Chrome");
  const audio = s.audioTranscodeUrl("/library/metadata/9", new TranscodeOptions({ session: "abc" }));
  expect(audio.pathname).toBe("/music/:/transcode/universal/start.mp3");
  expect(audio.searchParams.get("protocol")).toBe("http");
  expect(audio.searchParams.get("musicBitrate")).toBe("320");
});

test("authed urls and library uris", () => {
  const s = new PlexServer(info, `${base}/prefix`, "tok").withMachineIdentifier("mach");
  expect(s.url("/library/x").href).toBe(`${base}/prefix/library/x`);
  expect(s.itemUri(["1", "2"])).toBe("server://mach/com.plexapp.plugins.library/library/metadata/1,2");
  const u = s.authedUrl("/library/x?a=b%20c", new Params().set("d", "e f"));
  expect(u.search).toBe("?a=b%20c&d=e+f&X-Plex-Token=tok");
  expect(s.url("http://other/x").href).toBe("http://other/x");
  expect(s.imageUrl("/library/metadata/1/thumb", 100, 150).searchParams.get("upscale")).toBe("1");
});

test("errors: unauthorized, status, html snippet, empty body", async () => {
  const s = plex();
  const err = (await s.raw("GET", "/unauthorized").catch((e: unknown) => e)) as PlexError;
  expect(err).toBeInstanceOf(PlexError);
  expect(err.message).toBe("not authorized (token missing, expired or lacking access)");
  expect(err.status).toBe(401);

  const missing = (await s.raw("GET", "/missing").catch((e: unknown) => e)) as PlexError;
  expect(missing.message).toBe("GET /missing failed with HTTP 404: no such item");
  expect(missing.isNotFound()).toBe(true);

  const html = (await s.raw("GET", "/html").catch((e: unknown) => e)) as PlexError;
  expect(html.message).toBe("GET /html failed with HTTP 502");

  expect(await s.raw("GET", "/empty")).toEqual({});
  expect((await s.fetch("/empty")).Metadata).toEqual([]);
});

test("network errors read like the Rust ones and hide the token", async () => {
  const s = new PlexServer(info, "http://127.0.0.1:1", "secret-token");
  const err = (await s.raw("GET", "/x").catch((e: unknown) => e)) as PlexError;
  expect(err.kind).toBe("http");
  expect(err.message.startsWith("network error: error sending request for url (http://127.0.0.1:1/x")).toBe(true);
  expect(err.message).not.toContain("secret-token");
});

test("bytes returns the content type; identity decodes", async () => {
  const s = plex();
  const [bytes, ctype] = await s.bytes("/image");
  expect([...bytes]).toEqual([1, 2, 3]);
  expect(ctype).toBe("image/jpeg");
  const id = await s.identity();
  expect(id.machineIdentifier).toBe("m-1");
  expect(id.claimed).toBe(true);
});

test("ranged download headers", async () => {
  const s = plex();
  await (await s.downloadRange("/range", 100, "etag-1")).text();
  const r = last();
  expect(pairs(r)).toEqual(["download=1"]);
  expect(r.headers.get("range")).toBe("bytes=100-");
  expect(r.headers.get("if-range")).toBe("etag-1");
  expect(r.headers.get("accept-encoding")).toBe("identity");
  await (await s.downloadRange("/range", 0)).text();
  expect(last().headers.get("range")).toBeNull();
});

test("notifications stream", async () => {
  const s = plex();
  const got = [];
  for await (const n of await s.notifications(["playing", "status"])) got.push(n);
  expect(pairs(last())).toEqual(["filters=playing,status"]);
  expect(last().headers.get("accept")).toBe("text/event-stream");
  expect(last().headers.get("x-plex-token")).toBe("tok-123");
  expect(got).toEqual([
    { event: "playing", data: { a: 1 } },
    { event: "message", data: "plain" },
    { event: "status", data: "l1\nl2" },
  ]);
});

test("notifications stream can be aborted", async () => {
  const slow = Bun.serve({
    port: 0,
    fetch() {
      const body = new ReadableStream({
        start(c) {
          c.enqueue(new TextEncoder().encode('event: a\ndata: 1\n\n'));
        },
      });
      return new Response(body, { headers: { "content-type": "text/event-stream" } });
    },
  });
  try {
    const s = new PlexServer(info, `http://127.0.0.1:${slow.port}`, "t");
    const ac = new AbortController();
    const got: string[] = [];
    for await (const n of await s.notifications([], ac.signal)) {
      got.push(n.event);
      ac.abort();
    }
    expect(got).toEqual(["a"]);
  } finally {
    slow.stop(true);
  }
});

test("connectResource picks the reachable connection and binds the machine id", async () => {
  const tv = new PlexTv(info, "account-token");
  const resource = defaultResource({
    name: "Den",
    accessToken: "server-token",
    connections: [
      defaultConnection({ uri: "http://127.0.0.1:1", local: true, address: "127.0.0.1", port: 1 }),
      defaultConnection({ uri: base, local: false }),
    ],
  });
  seen = [];
  const s = await connectResource(tv, resource);
  expect(s.baseUrl().href).toBe(`${base}/`);
  expect(s.machineIdentifier()).toBe("m-1");
  expect(s.token()).toBe("server-token");
  expect(seen.some((r) => r.path === "/identity" && r.headers.get("x-plex-token") === "server-token")).toBe(true);

  const none = defaultResource({ name: "Gone", accessToken: "t", connections: [] });
  expect((await connectResource(tv, none).catch((e: unknown) => e)) as Error).toMatchObject({
    message: 'no reachable connection for server "Gone"',
  });
  const noToken = defaultResource({ name: "X", connections: [defaultConnection({ uri: base })] });
  expect(((await connectResource(new PlexTv(info), noToken).catch((e: unknown) => e)) as Error).message).toBe(
    "not authorized (token missing, expired or lacking access)",
  );
});
