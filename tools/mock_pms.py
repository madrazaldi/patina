#!/usr/bin/env python3
"""A tiny fake Plex Media Server for developing and testing Patina offline.

It serves realistic JSON for the endpoints the app uses, gradient artwork,
and a real video file (with two audio tracks, one subtitle track and intro /
credits markers), and logs every playback report so you can check timeline,
scrobble and stream-selection behaviour.

    python3 tools/mock_pms.py --media /path/to/dir --port 32499

`--media` must contain `movie.mkv` and `img/poster{0..7}.jpg`, `img/art{0..7}.jpg`.
Connect Patina with "Connect to a server directly", URL http://127.0.0.1:32499
and any token.
"""

import argparse
import json
import os
import re
import sys
import time
import zlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

MACHINE = "mock-machine-0001"
MEDIA = "."
EVENTS = []
DOWNLOAD_DELAY = 0
RESUME_VALIDATOR = "normal"
MISSING_METADATA = set()
DROP_DOWNLOAD_ONCE = False
DROPPED_DOWNLOAD = False
# --large N adds an "Archive" library (section 4) and a "Marathon" playlist (960) of N entries each,
# to exercise paging, on-demand loading and cancellation.
LARGE = 0

GENRES = ["Drama", "Science Fiction", "Thriller", "Comedy", "Animation", "Mystery"]
MOVIE_TITLES = [
    "The Verdigris Line", "Copper Sky", "Afterglow", "Quiet Harbor", "Northbound",
    "Paper Moons", "The Long Exposure", "Salt & Static", "Glass Orchard", "Lanterns",
    "Midnight Transit", "A Field of Echoes", "Tidewater", "The Cartographer", "Low Orbit",
    "Signal Fires", "Hollow Crown", "Bright Minutes", "The Understudy", "Ember Season",
]
SHOW_TITLES = ["Station Eleven Below", "The Weatherman", "Parallel Lines", "Harbor Lights"]
EPG = "/tv.plex.providers.epg.cloud:2"
CHANNELS = [("2.1", "Harbor One"), ("4.1", "Copper News"), ("7.1", "Verdigris Classics")]
RULES = {"41": "Copper News Tonight"}
DELETED = set()


def img(kind, n):
    return f"/library/metadata/{n}/{kind}/{1700000000 + n}"


def stream_list(part_id):
    streams = [
        {"id": part_id * 10 + 1, "streamType": 1, "codec": "h264", "index": 0, "height": 720, "width": 1280,
         "displayTitle": "720p (H.264)", "frameRate": 24.0},
        {"id": part_id * 10 + 2, "streamType": 2, "codec": "aac", "index": 1, "channels": 2, "language": "English",
         "languageCode": "eng", "displayTitle": "English (AAC Stereo)", "extendedDisplayTitle": "English (AAC Stereo)"},
        {"id": part_id * 10 + 3, "streamType": 2, "codec": "aac", "index": 2, "channels": 2, "language": "Japanese",
         "languageCode": "jpn", "selected": True, "displayTitle": "Japanese (AAC Stereo)",
         "extendedDisplayTitle": "Japanese (AAC Stereo)"},
        {"id": part_id * 10 + 4, "streamType": 3, "codec": "srt", "index": 3, "language": "English",
         "languageCode": "eng", "selected": True, "displayTitle": "English (SRT)",
         "extendedDisplayTitle": "English (SRT Embedded)"},
    ]
    # Every show's S02E03 lacks the Japanese dub, to exercise "no matching track".
    if 1000 <= part_id < 2000 and part_id % 100 == 23:
        streams = [s for s in streams if s.get("languageCode") != "jpn" or s["streamType"] != 2]
        streams[1]["selected"] = True
    sel = SELECTED.get(part_id, {})
    for s in streams:
        kind = {2: "audio", 3: "subtitle"}.get(s["streamType"])
        if kind in sel:
            s["selected"] = s["id"] == sel[kind]
    return streams


def media_for(rk):
    part_id = int(rk)
    return [{
        "id": part_id, "duration": 180000, "bitrate": 6200, "width": 1280, "height": 720, "aspectRatio": 1.78,
        "audioChannels": 2, "audioCodec": "aac", "videoCodec": "h264", "videoResolution": "720",
        "container": "mkv", "videoFrameRate": "24p",
        "Part": [{"id": part_id, "key": f"/library/parts/{part_id}/1700000000/file.mkv", "duration": 180000,
                  "file": f"/media/movies/{rk}.mkv", "size": os.path.getsize(os.path.join(MEDIA, "movie.mkv")),
                  "container": "mkv", "Stream": stream_list(part_id)}],
    }]


def movie(i):
    rk = str(100 + i)
    state = STATE.setdefault(rk, {"viewCount": 1 if i % 5 == 3 else 0, "viewOffset": 61000 if i == 0 else 0})
    m = {
        "ratingKey": rk, "key": f"/library/metadata/{rk}", "guid": f"plex://movie/5d77{rk}", "type": "movie",
        "title": MOVIE_TITLES[i % len(MOVIE_TITLES)], "year": 2014 + i % 11, "contentRating": ["PG-13", "R", "PG"][i % 3],
        "summary": "A quiet engineer discovers that the old copper lines beneath the city still carry voices — "
                   "and one of them knows her name. A mock-server feature used to exercise Patina's detail page.",
        "tagline": "Every signal leaves a trace.", "studio": "Mock Pictures",
        "rating": 7.1 + (i % 3) / 2, "audienceRating": 8.0 - (i % 4) / 3,
        "duration": 180000, "addedAt": int(time.time()) - i * 86400, "originallyAvailableAt": f"{2014 + i % 11}-05-0{1 + i % 9}",
        "thumb": img("thumb", i % 8), "art": img("art", i % 8), "librarySectionID": 1,
        "Genre": [{"id": 10 + (i % len(GENRES)), "tag": GENRES[i % len(GENRES)], "filter": f"genre={10 + i % len(GENRES)}"}],
        "Director": [{"id": 200, "tag": "Ada Lindqvist", "filter": "director=200"}],
        "Role": [{"id": 300 + j, "tag": n, "role": r, "filter": f"actor={300 + j}"} for j, (n, r) in
                 enumerate([("Mara Okafor", "Iris"), ("Tomas Reyes", "Hale"), ("June Park", "Operator"),
                            ("Elias Brandt", "The Voice")])],
        "Media": media_for(rk),
        "UltraBlurColors": {"topLeft": "1f4d44", "topRight": "3a2a1a", "bottomRight": "0f1a26", "bottomLeft": "2a1f3a"},
    }
    m.update({k: v for k, v in state.items() if v})
    return m


def big_movie(i):
    """Entry `i` of the --large library: a generated movie with a stable, sortable title."""
    base = movie(i % 20)
    rk = str(10000 + i)
    return dict(base, ratingKey=rk, key=f"/library/metadata/{rk}", guid=f"plex://movie/big{rk}", title=f"Archive {i + 1:05d}",
                librarySectionID=4, viewCount=1 if i % 7 == 0 else 0, addedAt=int(time.time()) - i * 60)


def show(i):
    rk = str(500 + i)
    return {
        "ratingKey": rk, "key": f"/library/metadata/{rk}/children", "guid": f"plex://show/5e{rk}", "type": "show",
        "title": SHOW_TITLES[i], "year": 2019 + i, "summary": "An anthology of small towns and strange weather.",
        "thumb": img("thumb", (i + 3) % 8), "art": img("art", (i + 3) % 8), "childCount": 2, "leafCount": 6,
        "viewedLeafCount": 1 if i == 0 else 0, "librarySectionID": 2, "contentRating": "TV-14",
        "Genre": [{"id": 12, "tag": "Mystery", "filter": "genre=12"}],
    }


def season(show_i, s):
    rk = str(600 + show_i * 10 + s)
    return {
        "ratingKey": rk, "key": f"/library/metadata/{rk}/children", "type": "season", "title": f"Season {s}",
        "index": s, "parentRatingKey": str(500 + show_i), "parentTitle": SHOW_TITLES[show_i],
        "thumb": img("thumb", (show_i + s) % 8), "art": img("art", (show_i + 3) % 8), "leafCount": 3,
        "viewedLeafCount": 1 if (show_i == 0 and s == 1) else 0, "librarySectionID": 2,
    }


def episode(show_i, s, e):
    rk = str(1000 + show_i * 100 + s * 10 + e)
    state = STATE.setdefault(rk, {"viewCount": 1 if (show_i == 0 and s == 1 and e == 1) else 0, "viewOffset": 0})
    m = {
        "ratingKey": rk, "key": f"/library/metadata/{rk}", "type": "episode", "title": f"Chapter {e}: {['Static', 'Undertow', 'Relay'][e - 1]}",
        "index": e, "parentIndex": s, "parentRatingKey": str(600 + show_i * 10 + s), "parentTitle": f"Season {s}",
        "grandparentRatingKey": str(500 + show_i), "grandparentTitle": SHOW_TITLES[show_i],
        "grandparentThumb": img("thumb", (show_i + 3) % 8), "grandparentArt": img("art", (show_i + 3) % 8),
        "thumb": img("art", (e + s) % 8), "summary": "The storm arrives early and the radio tower starts talking back.",
        "duration": 180000, "originallyAvailableAt": f"202{s}-0{e}-12", "librarySectionID": 2,
        "Media": media_for(rk),
        "Marker": [
            {"id": int(rk) * 10 + 1, "type": "intro", "startTimeOffset": 5000, "endTimeOffset": 25000},
            {"id": int(rk) * 10 + 2, "type": "credits", "startTimeOffset": 165000, "endTimeOffset": 180000, "final": True},
        ],
    }
    m.update({k: v for k, v in state.items() if v})
    return m


# A small music library: one artist, one album of four tracks, each with synced (LRC) lyrics.
TRACK_TITLES = ["Dial Tone", "Undersea Cable", "Relay Station", "Last Transmission"]


def artist():
    return {"ratingKey": "3000", "key": "/library/metadata/3000/children", "type": "artist", "title": "The Copper Lines",
            "summary": "A band that only exists on the mock server.", "thumb": img("thumb", 6), "art": img("art", 6),
            "librarySectionID": 3, "childCount": 1, "Genre": [{"id": 40, "tag": "Ambient", "filter": "genre=40"}]}


def album():
    return {"ratingKey": "3100", "key": "/library/metadata/3100/children", "type": "album", "title": "Low Tide Radio",
            "parentRatingKey": "3000", "parentTitle": "The Copper Lines", "year": 2021, "thumb": img("thumb", 7),
            "librarySectionID": 3, "leafCount": len(TRACK_TITLES), "studio": "Mock Records"}


def track(i):
    rk = str(3101 + i)
    part_id = int(rk)
    return {"ratingKey": rk, "key": f"/library/metadata/{rk}", "type": "track", "title": TRACK_TITLES[i], "index": i + 1,
            "parentIndex": 1, "parentRatingKey": "3100", "parentTitle": "Low Tide Radio", "grandparentRatingKey": "3000",
            "grandparentTitle": "The Copper Lines", "thumb": img("thumb", 7), "librarySectionID": 3, "duration": 180000,
            "Media": [{"id": part_id, "duration": 180000, "container": "mkv", "audioCodec": "aac", "audioChannels": 2,
                       "Part": [{"id": part_id, "key": f"/library/parts/{part_id}/1700000000/file.mkv", "duration": 180000,
                                 "file": f"/media/music/{rk}.mkv", "container": "mkv",
                                 "Stream": [{"id": part_id * 10 + 2, "streamType": 2, "codec": "aac", "channels": 2, "selected": True,
                                             "displayTitle": "AAC Stereo"},
                                            {"id": part_id * 10 + 5, "streamType": 4, "codec": "lrc", "format": "lrc",
                                             "key": f"/library/streams/{part_id * 10 + 5}"}]}]}]}


def lyrics(stream_id):
    title = TRACK_TITLES[(stream_id // 10 - 3101) % len(TRACK_TITLES)]
    words = ["The line goes quiet", "then a voice comes through", "static on the water", "someone calling you",
             "copper under harbour", "carrying the tide", "every signal leaves a trace", "and none of them have died"]
    lines = [f"[ti:{title}]"] + [f"[{(5 * n) // 60:02}:{(5 * n) % 60:02}.00]{words[n % len(words)]}" for n in range(36)]
    return "\n".join(lines)


STATE = {}
# Rating keys removed from Continue Watching.
HIDDEN = set()
# part id -> {"audio": stream id, "subtitle": stream id or 0}, set through PUT /library/parts/<id>.
SELECTED = {}


# One collection (custom sort) and one playlist, editable through their endpoints.
COLLECTION = {"items": ["100", "101", "102", "103"], "collectionSort": "2", "collectionMode": "-1", "title": "Heist Nights", "gone": False}
PLAYLIST = {"items": ["104", "105", "1012"], "title": "Evening Mix", "summary": "", "gone": False}


def collection():
    return {"ratingKey": "900", "key": "/library/collections/900/children", "type": "collection", "subtype": "movie",
            "title": COLLECTION["title"], "librarySectionID": 1, "childCount": len(COLLECTION["items"]),
            "collectionSort": COLLECTION["collectionSort"], "collectionMode": COLLECTION["collectionMode"], "thumb": img("thumb", 5)}


def playlist():
    return {"ratingKey": "950", "key": "/playlists/950/items", "type": "playlist", "playlistType": "video", "smart": False,
            "title": PLAYLIST["title"], "summary": PLAYLIST["summary"], "leafCount": len(PLAYLIST["items"]), "duration": 180000 * len(PLAYLIST["items"])}


def big_playlist():
    return {"ratingKey": "960", "key": "/playlists/960/items", "type": "playlist", "playlistType": "video", "smart": False,
            "title": "Marathon", "summary": "", "leafCount": LARGE, "duration": 180000 * LARGE}


def artwork(rk, kind):
    return [{"key": f"/library/metadata/{rk}/{kind}/{1700000000 + i}", "ratingKey": f"upload://{kind}/{i}",
             "thumb": img(kind if kind == "art" else "thumb", i), "provider": ["local", "tmdb", "fanart"][i % 3], "selected": i == 0}
            for i in range(3)]


def edit_lists(self, p, q):
    """Collection, playlist and artwork endpoints; None when `p` isn't one."""
    if p == "/library/sections/1/collections":
        return mc(Metadata=[] if COLLECTION["gone"] else [collection()])
    if p == "/library/collections" and self.command == "POST":
        self.record("create_collection", q)
        return mc(Metadata=[dict(collection(), ratingKey="901", title=q.get("title", [""])[0])])
    m = re.fullmatch(r"/library/collections/900(/children|/items(?:/(\d+)(/move)?)?)?", p)
    if m:
        sub, rk, move = m.groups()
        items = COLLECTION["items"]
        if sub == "/children":
            return mc(Metadata=[find(k) for k in items])
        if self.command == "DELETE" and not sub:
            COLLECTION["gone"] = True
        elif move:
            items.remove(rk)
            after = q.get("after", [None])[0]
            items.insert(items.index(after) + 1 if after in items else 0, rk)
        elif rk and self.command == "DELETE":
            items.remove(rk)
        elif sub == "/items" and self.command == "PUT":
            items.append(q["uri"][0].rsplit("/", 1)[-1])
        self.record(f"collection {self.command} {p}", q)
        return mc()
    if p == "/library/metadata/900/prefs":
        self.record("collection prefs", q)
        for k in ("collectionSort", "collectionMode"):
            if k in q:
                COLLECTION[k] = q[k][0]
        return mc()
    if p == "/playlists" and self.command == "GET":
        kind = q.get("playlistType", ["video"])[0]
        lists = [] if PLAYLIST["gone"] or kind != "video" else [playlist()]
        return mc(Metadata=lists + ([big_playlist()] if LARGE and kind == "video" else []))
    if p == "/playlists" and self.command == "POST":
        self.record("create_playlist", q)
        return mc(Metadata=[dict(playlist(), ratingKey="951", title=q.get("title", [""])[0])])
    if LARGE and p in ("/playlists/960", "/playlists/960/items"):
        if p.endswith("/items"):
            self.record("big playlist page", q)
            return page([dict(big_movie(i), playlistItemID=70000 + i) for i in range(LARGE)], q)
        return mc(Metadata=[big_playlist()])
    m = re.fullmatch(r"/playlists/950(/items(?:/(\d+)(/move)?)?)?", p)
    if m:
        sub, entry, move = m.groups()
        items = PLAYLIST["items"]
        if not sub and self.command == "GET":
            return mc(Metadata=[playlist()])
        if sub == "/items" and not entry and self.command == "GET":
            out = [dict(find(k), playlistItemID=7000 + int(k)) for k in items]
            return page(out, q)
        if not sub and self.command == "PUT":
            for k in ("title", "summary"):
                if k in q:
                    PLAYLIST[k] = q[k][0]
        elif not sub and self.command == "DELETE":
            PLAYLIST["gone"] = True
        elif sub == "/items" and not entry and self.command == "DELETE":
            items.clear()
        elif sub == "/items" and not entry and self.command == "PUT":
            items.append(q["uri"][0].rsplit("/", 1)[-1])
        elif move:
            rk = str(int(entry) - 7000)
            items.remove(rk)
            after = str(int(q["after"][0]) - 7000) if "after" in q else None
            items.insert(items.index(after) + 1 if after in items else 0, rk)
        elif entry and self.command == "DELETE":
            items.remove(str(int(entry) - 7000))
        self.record(f"playlist {self.command} {p}", q)
        return mc()
    if self.command == "PUT" and "id" in q and (re.fullmatch(r"/library/sections/\d+/all", p) or re.fullmatch(r"/library/metadata/\d+", p)):
        rk = q["id"][0]
        item, st = find(rk) or {}, STATE.setdefault(rk, {})
        fields = {f["name"]: f for f in st.get("Field", [])}
        for k, v in q.items():
            if k.endswith(".value"):
                st[k[:-6]] = int(v[0]) if k == "year.value" and v[0].isdigit() else v[0]
            elif k.endswith(".locked"):
                fields[k[:-7]] = {"name": k[:-7], "locked": v[0] == "1"}
            m = re.fullmatch(r"(\w+)\[(\d*)\]\.tag\.tag(-?)", k)
            if m:
                key = m.group(1).capitalize()
                tags = st.setdefault(key, list(item.get(key, [])))
                gone = v[0].lower().split(",") if m.group(3) else []
                tags[:] = [t for t in tags if t["tag"].lower() not in gone] + ([] if m.group(3) else [{"tag": v[0]}])
        st["Field"] = list(fields.values())
        self.record("edit " + p, q)
        return mc()
    m = re.fullmatch(r"/library/metadata/(\d+)/(matches|match|unmatch)", p)
    if m:
        rk, what = m.groups()
        if what == "matches":
            title = q.get("title", [(find(rk) or {}).get("title", "")])[0]
            return mc(SearchResult=[{"guid": f"plex://movie/{i}", "name": title if i == 0 else f"{title} {i + 1}", "year": 2014 + i,
                                     "score": 100 - 10 * i, "matched": i == 0} for i in range(3)])
        self.record(what, q)
        return mc()
    m = re.fullmatch(r"/library/metadata/(\d+)/(posters|arts|poster|art)", p)
    if m:
        rk, what = m.groups()
        if what in ("posters", "arts"):
            return mc(Metadata=artwork(rk, "art" if what == "arts" else "thumb"))
        self.record(f"set {what}", q)
        return mc()
    return None


def all_movies():
    return [movie(i) for i in range(20)]


def all_shows():
    return [show(i) for i in range(len(SHOW_TITLES))]


def find(rk):
    rk = str(rk)
    n = int(rk)
    if 100 <= n < 120:
        return movie(n - 100)
    if 500 <= n < 500 + len(SHOW_TITLES):
        return show(n - 500)
    if 600 <= n < 700:
        return season((n - 600) // 10, n % 10)
    if 1000 <= n < 2000:
        return episode((n - 1000) // 100, (n % 100) // 10, n % 10)
    if LARGE and 10000 <= n < 10000 + LARGE:
        return big_movie(n - 10000)
    if n == 900 and not COLLECTION["gone"]:
        return collection()
    if n == 3000:
        return artist()
    if n == 3100:
        return album()
    if 3101 <= n < 3101 + len(TRACK_TITLES):
        return track(n - 3101)
    return None


def leaves(rk):
    item = find(rk)
    if item is None:
        return []
    if item["type"] == "show":
        return [e for s in children(rk) for e in children(s["ratingKey"])]
    if item["type"] in ("season", "album"):
        return children(rk)
    if item["type"] == "artist":
        return children("3100")
    return [item]


def children(rk):
    n = int(rk)
    if 500 <= n < 600:
        return [season(n - 500, s) for s in (1, 2)]
    if 600 <= n < 700:
        return [episode((n - 600) // 10, n % 10, e) for e in (1, 2, 3)]
    if n == 3000:
        return [album()]
    if n == 3100:
        return [track(i) for i in range(len(TRACK_TITLES))]
    return []


def livetv(self, p, q):
    """Live TV & DVR: one tuner, three channels, half-hour programs, recording rules."""
    now = int(time.time()) // 1800 * 1800
    if p == "/livetv/dvrs":
        maps = [{"channelKey": f"k{n}", "enabled": "1"} for n, _ in CHANNELS]
        return mc(Dvr=[{"key": "7", "epgIdentifier": EPG[1:], "lineup": "lineup://mock", "Device": [{"ChannelMapping": maps}]}])
    if p == EPG + "/lineups/dvr/channels":
        return mc(Channel=[{"identifier": f"ch{n}", "key": f"k{n}", "channelVcn": n, "title": t} for n, t in CHANNELS])
    if p == EPG + "/grid":
        out = []
        for c, (n, t) in enumerate(CHANNELS):
            for i in range(-1, 12):
                b = now + i * 1800
                show = i % 2 == 0
                out.append({"type": "episode" if show else "movie", "guid": f"plex://episode/{c}-{i}",
                            "title": f"Episode {i + 2}" if show else MOVIE_TITLES[(c * 5 + i) % 20],
                            "grandparentTitle": SHOW_TITLES[c % 4], "year": 1999 + i, "summary": f"Live on {t}.",
                            "Media": [{"channelIdentifier": f"ch{n}", "beginsAt": b, "endsAt": b + 1800}]})
        return mc(Metadata=out)
    m = re.fullmatch(r"/livetv/dvrs/7/channels/([^/]+)/tune", p)
    if m:
        self.record("tune " + m.group(1), q)
        return mc(MediaSubscription=[{"MediaGrabOperation": [{"Metadata": {"key": "/livetv/sessions/mock/index.m3u8",
                                                                           "title": "Live", "type": "clip"}}]}])
    if p == "/media/subscriptions/template":
        return mc(SubscriptionTemplate=[{"MediaSubscription": [{"type": 4, "targetLibrarySectionID": 2, "parameters": "hints[guid]=x",
                                                                 "Setting": [{"id": "minVideoQuality", "default": 0}]}]}])
    if p == "/media/subscriptions" and self.command == "POST":
        self.record("record", q)
        RULES[str(40 + len(RULES) + 1)] = q.get("hints[guid]", ["New rule"])[0]
        return mc()
    m = re.fullmatch(r"/media/subscriptions/(\d+)", p)
    if m and self.command == "DELETE":
        self.record("delete rule", q)
        RULES.pop(m.group(1), None)
        return mc()
    if p == "/media/subscriptions":
        return mc(MediaSubscription=[{"key": f"/media/subscriptions/{k}", "title": t, "type": "4"} for k, t in RULES.items()])
    if p == "/media/subscriptions/scheduled":
        return mc(MediaGrabOperation=[{"id": "g1", "status": "scheduled", "Metadata": {"type": "episode", "grandparentTitle": "Copper News Tonight",
                                       "title": "Evening edition", "Media": [{"beginsAt": now + 7200, "channelTitle": "Copper News"}]}}])
    return None


def admin(self, p, q):
    """Server tools: activities, transcodes, stats, updates, users, libraries, diagnostics."""
    now = int(time.time())
    if p == "/activities":
        return mc(Activity=[{"uuid": "act-1", "type": "media.generate.thumbs", "cancellable": True, "title": "Generating thumbnails",
                             "subtitle": "Copper Sky", "progress": 42}])
    if p.startswith("/activities/") or p.startswith("/transcode/sessions/") or p.startswith("/updater/") \
            or (p.startswith("/butler") and self.command != "GET") or p in ("/library/clean/bundles", "/library/optimize"):
        self.record(self.command + " " + p, q)
        return mc()
    if p == "/transcode/sessions":
        return mc(TranscodeSession=[{"key": "/transcode/sessions/tr-1", "progress": 37.5, "speed": 2.4, "context": "streaming",
                                     "protocol": "hls", "sourceVideoCodec": "hevc", "videoCodec": "h264", "audioCodec": "aac",
                                     "videoDecision": "transcode", "audioDecision": "copy", "transcodeHwRequested": True}])
    if p == "/updater/status":
        return mc(canInstall=True, Release=[{"version": "1.42.0.1000", "added": "Faster scans.", "state": "available"}])
    if p == "/statistics/bandwidth":
        rows = [{"accountID": 1, "deviceID": 1 + h % 2, "at": now - h * 3600, "lan": h % 2 == 1, "bytes": (h + 1) * 40_000_000}
                for h in range(0, 24, 3)]
        return mc(Account=[{"id": 1, "name": "tester"}], Device=[{"id": 1, "name": "Living room TV"}, {"id": 2, "name": "Patina"}],
                  StatisticsBandwidth=rows)
    if p == "/statistics/media":
        return mc(StatisticsMedia=[{"accountID": 1, "at": now - 7200, "count": 3, "duration": 9000}])
    if p == "/devices":
        return mc(Device=[{"id": 1, "name": "Living room TV", "platform": "tvOS", "createdAt": now - 86400 * 30}])
    if p == "/clients":
        return mc(Server=[{"name": "Kitchen speaker", "product": "Plexamp", "address": "192.168.1.20", "machineIdentifier": "c1"}])
    if p == "/system/agents":
        return mc(Agent=[{"identifier": "tv.plex.agents.movie", "title": "Plex Movie"}, {"identifier": "com.plexapp.agents.none", "title": "Personal Media"}])
    if p == "/system/scanners":
        return mc(Scanner=[{"name": "Plex Movie"}, {"name": "Plex Video Files Scanner"}])
    if p == "/library/sections" and self.command == "POST":
        self.record("create library", q)
        return mc()
    m = re.fullmatch(r"/library/sections/(\d+)(/prefs|/refresh|/analyze|/emptyTrash)?", p)
    if m and (self.command != "GET" or m.group(2) == "/refresh"):
        self.record(self.command + " " + p, q)
        if m.group(2) == "/prefs":
            SECTION_PREFS.setdefault(m.group(1), {}).update({k: v[0] for k, v in q.items() if not k.startswith("X-Plex")})
        return mc()
    if m and m.group(2) == "/prefs":
        saved = SECTION_PREFS.get(m.group(1), {})
        return mc(Setting=[{**s, "value": pref_value(s, saved)} for s in [
            {"id": "enableCinemaTrailers", "label": "Enable cinema trailers", "type": "bool", "value": True},
            {"id": "collectionMode", "label": "Collections", "type": "int", "value": 0,
             "enumValues": "0:Show|1:Hide items in collections|2:Hide collections"}]])
    if p in ("/diagnostics/logs", "/diagnostics/databases"):
        body = b"PK\x05\x06" + bytes(18)
        self.send_response(200)
        self.send_header("Content-Type", "application/zip")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
        return True
    return None


PREFS, SECTION_PREFS = {}, {}

HOURS = "|".join(f"{h}:{h % 12 or 12} {'am' if h < 12 else 'pm'}" for h in range(24))
# A sample of real server settings, with Plex's groups and the butler's.
SERVER_PREFS = [
    {"id": "FriendlyName", "label": "Friendly name", "summary": "The name of this server.", "type": "text", "value": "Mock Server", "group": "general"},
    {"id": "logDebug", "label": "Enable Plex Media Server debug logging", "type": "bool", "value": True, "group": "general", "advanced": True},
    {"id": "ScheduledLibraryUpdatesEnabled", "label": "Update my library periodically", "type": "bool", "value": True, "group": "library"},
    {"id": "FSEventLibraryUpdatesEnabled", "label": "Scan my library automatically", "type": "bool", "value": False, "group": "library"},
    {"id": "secureConnections", "label": "Secure connections", "type": "int", "value": 1, "group": "network",
     "enumValues": "0:Required|1:Preferred|2:Disabled"},
    {"id": "customConnections", "label": "Custom server access URLs", "type": "text", "value": "", "group": "network", "advanced": True},
    {"id": "TranscoderQuality", "label": "Transcoder quality", "type": "int", "value": 0, "group": "transcoder",
     "enumValues": "0:Automatic|1:Prefer higher speed encoding|2:Prefer higher quality encoding"},
    {"id": "DlnaEnabled", "label": "Enable the DLNA server", "type": "bool", "value": False, "group": "dlna"},
    {"id": "ButlerStartHour", "label": "Time at which tasks start to run", "type": "int", "value": 2, "group": "butler", "enumValues": HOURS},
    {"id": "ButlerEndHour", "label": "Time at which tasks stop running", "type": "int", "value": 5, "group": "butler", "enumValues": HOURS},
    {"id": "ButlerTaskBackupDatabase", "label": "Backup database every three days", "type": "bool", "value": True, "group": "butler"},
    {"id": "ButlerDatabaseBackupPath", "label": "Backup directory", "type": "text", "value": "/var/lib/plex/backups", "group": "butler", "advanced": True},
    {"id": "ButlerTaskOptimizeDatabase", "label": "Optimize database every week", "type": "bool", "value": True, "group": "butler"},
    {"id": "ButlerTaskCleanOldBundles", "label": "Remove old bundles every week", "type": "bool", "value": False, "group": "butler"},
    {"id": "ButlerTaskRefreshPeriodicMetadata", "label": "Refresh metadata periodically", "type": "bool", "value": True, "group": "butler"},
]


def pref_value(setting, saved):
    """A saved pref in the shape Plex reports it."""
    v = saved.get(setting["id"])
    if v is None:
        return setting["value"]
    if setting["type"] == "bool":
        return v in ("1", "true")
    return int(v) if setting["type"] == "int" else v


def mc(**kw):
    kw.setdefault("size", len(kw.get("Metadata", kw.get("Directory", kw.get("Hub", [])))))
    return {"MediaContainer": kw}


def page(items, q):
    start = int(q.get("X-Plex-Container-Start", ["0"])[0])
    size = int(q.get("X-Plex-Container-Size", ["1000"])[0])
    return mc(Metadata=items[start:start + size], offset=start, totalSize=len(items), size=len(items[start:start + size]))


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):
        pass

    def send_json(self, obj, status=200):
        body = json.dumps(obj).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def send_file(self, path, ctype):
        global DROPPED_DOWNLOAD
        size = os.path.getsize(path)
        etag = f'"mock-{size}-{os.stat(path).st_mtime_ns}"'
        rng = self.headers.get("Range")
        if self.headers.get("If-Range") not in (None, etag):
            rng = None
        if path.endswith("movie.mkv"):
            self.record("download", {"range": [rng or "full"]})
        start, end = 0, size - 1
        if rng:
            m = re.match(r"bytes=(\d*)-(\d*)", rng)
            if m:
                start = int(m.group(1) or 0)
                end = int(m.group(2)) if m.group(2) else size - 1
            if start >= size:
                self.send_response(416)
                self.send_header("Content-Range", f"bytes */{size}")
                self.end_headers()
                return
            end = min(end, size - 1)
            self.send_response(206)
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        else:
            self.send_response(200)
        self.send_header("Content-Type", ctype)
        self.send_header("Accept-Ranges", "bytes")
        if not rng or RESUME_VALIDATOR != "missing":
            self.send_header("ETag", '"changed"' if rng and RESUME_VALIDATOR == "changed" else etag)
        self.send_header("Content-Length", str(end - start + 1))
        self.end_headers()
        with open(path, "rb") as f:
            f.seek(start)
            left = end - start + 1
            try:
                while left > 0:
                    chunk = f.read(min(1 << 16, left))
                    if not chunk:
                        break
                    self.wfile.write(chunk)
                    left -= len(chunk)
                    if path.endswith("movie.mkv"):
                        self.wfile.flush()
                        if DOWNLOAD_DELAY:
                            time.sleep(DOWNLOAD_DELAY)
                        if DROP_DOWNLOAD_ONCE and not DROPPED_DOWNLOAD and left < size - 262144:
                            DROPPED_DOWNLOAD = True
                            self.close_connection = True
                            return
            except (BrokenPipeError, ConnectionResetError):
                pass

    def record(self, what, q):
        entry = {"t": round(time.time(), 1), "what": what, **{k: v[0] for k, v in q.items() if not k.startswith("X-Plex")}}
        EVENTS.append(entry)
        print(json.dumps(entry), flush=True)

    def do_PUT(self):
        self.do_GET()

    def do_POST(self):
        self.do_GET()

    def do_DELETE(self):
        self.do_GET()

    def send_events(self):
        """Live notifications: a fake library scan every 30 seconds."""
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()

        def emit(event, data):
            self.wfile.write(f"event: {event}\ndata: {json.dumps(data)}\n\n".encode())
            self.wfile.flush()

        try:
            while True:
                time.sleep(30)
                uuid = f"scan-{int(time.time())}"
                for progress in range(0, 101, 20):
                    activity = {"uuid": uuid, "type": "library.update.section", "cancellable": True, "title": "Scanning Movies",
                                "subtitle": MOVIE_TITLES[progress // 20], "progress": progress, "Context": {"librarySectionID": "1"}}
                    emit("activity", {"ActivityNotification": [{"event": "updated", "uuid": uuid, "Activity": activity}]})
                    time.sleep(1)
                emit("activity", {"ActivityNotification": [{"event": "ended", "uuid": uuid, "Activity": activity}]})
                emit("timeline", {"TimelineEntry": [{"identifier": "com.plexapp.plugins.library", "sectionID": "1", "itemID": "1",
                                                     "type": 1, "state": 5}]})
        except (BrokenPipeError, ConnectionResetError):
            pass

    def do_GET(self):
        u = urlparse(self.path)
        p, q = u.path, parse_qs(u.query)
        if p == "/:/eventsource/notifications":
            return self.send_events()
        # --- play queue edits ----------------------------------------------------
        if re.fullmatch(r"/playQueues/\d+(/items(/\d+(/move)?)?|/shuffle|/unshuffle)?", p) and self.command != "GET":
            self.record("playqueue " + self.command + " " + p, q)
            return self.send_json(mc(playQueueID=77))
        out = edit_lists(self, p, q)
        if out is None:
            out = livetv(self, p, q)
        if out is None:
            out = admin(self, p, q)
        if out is True:
            return
        if out is not None:
            return self.send_json(out)
        # --- browsing extras -------------------------------------------------------
        m = re.fullmatch(r"/library/sections/(\d+)/(firstCharacter|folder)", p)
        if m:
            sid, what = m.groups()
            items = all_movies() if sid == "1" else [big_movie(i) for i in range(LARGE)] if sid == "4" else all_shows()
            if what == "firstCharacter":
                counts = {}
                for i in sorted(items, key=lambda i: i["title"]):
                    ch = i["title"][0].upper()
                    counts[ch] = counts.get(ch, 0) + 1
                return self.send_json(mc(Directory=[{"key": c, "title": c, "size": n} for c, n in counts.items()]))
            if "parent" not in q:
                return self.send_json(mc(title1="Movies", Directory=[{"key": f"/library/sections/{sid}/folder?parent=1", "title": "Features"}]))
            return self.send_json(mc(title2="Features", Metadata=items[:6]))
        if re.fullmatch(r"/library/sections/\d+/all", p) and q.get("includeMeta") == ["1"]:
            filters = [{"filter": "genre", "filterType": "string", "key": "/library/sections/1/genre", "title": "Genre"},
                       {"filter": "year", "filterType": "integer", "key": "/library/sections/1/year", "title": "Year"},
                       {"filter": "unwatched", "filterType": "boolean", "key": "/library/sections/1/unwatched", "title": "Unwatched"},
                       {"filter": "hdr", "filterType": "boolean", "key": "/library/sections/1/hdr", "title": "HDR"}]
            return self.send_json(mc(size=0, Meta={"Type": [{"key": "/library/sections/1/all?type=1", "type": "movie", "title": "Movies",
                                                             "active": True, "Filter": filters}]}))
        if re.fullmatch(r"/library/sections/\d+/year", p):
            years = sorted({m.get("year") for m in all_movies() if m.get("year")}, reverse=True)
            return self.send_json(mc(Directory=[{"key": str(y), "title": str(y)} for y in years]))
        # --- playback reporting -------------------------------------------------
        if p == "/:/timeline":
            self.record("timeline", q)
            rk = q["ratingKey"][0]
            st = STATE.setdefault(rk, {})
            st["viewOffset"] = 0 if q["state"][0] == "stopped" and int(q["time"][0]) >= int(q["duration"][0]) * 0.9 else int(q["time"][0])
            return self.send_json(mc())
        if p in ("/:/scrobble", "/:/unscrobble"):
            self.record(p[3:], q)
            st = STATE.setdefault(q["key"][0], {})
            st["viewCount"] = 1 if p == "/:/scrobble" else 0
            st["viewOffset"] = 0
            return self.send_json(mc())
        if p == "/:/progress":
            self.record("progress", q)
            STATE.setdefault(q["key"][0], {})["viewOffset"] = int(q["time"][0])
            return self.send_json(mc())
        if p == "/actions/removeFromContinueWatching":
            self.record("removeFromContinueWatching", q)
            HIDDEN.add(q["ratingKey"][0])
            return self.send_json(mc())
        if p == "/:/rate":
            self.record("rate", q)
            STATE.setdefault(q["key"][0], {})["userRating"] = float(q["rating"][0])
            return self.send_json(mc())
        if re.fullmatch(r"/library/parts/\d+", p):
            self.record("select_streams", q)
            sel = SELECTED.setdefault(int(p.rsplit("/", 1)[1]), {})
            for param, kind in (("audioStreamID", "audio"), ("subtitleStreamID", "subtitle")):
                if param in q:
                    sel[kind] = int(q[param][0])
            return self.send_json(mc())
        m = re.fullmatch(r"/library/streams/(\d+)", p)
        if m:
            body = lyrics(int(m.group(1))).encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            return self.wfile.write(body)
        # --- media -----------------------------------------------------------------
        if re.fullmatch(r"/library/parts/\d+/\d+/file\.mkv", p):
            return self.send_file(os.path.join(MEDIA, "movie.mkv"), "video/x-matroska")
        if p == "/photo/:/transcode":
            src = q.get("url", [""])[0]
            m = re.search(r"/(thumb|art)/", src)
            n = zlib.crc32(src.encode()) % 8
            m2 = re.search(r"/metadata/(\d+)/", src)
            if m2:
                n = int(m2.group(1)) % 8
            kind = "art" if (m and m.group(1) == "art") else "poster"
            w = int(q.get("width", ["300"])[0])
            h = int(q.get("height", ["450"])[0])
            if w > h * 1.2:
                kind = "art"
            return self.send_file(os.path.join(MEDIA, "img", f"{kind}{n}.jpg"), "image/jpeg")
        # --- server ----------------------------------------------------------------
        if p == "/identity":
            return self.send_json(mc(machineIdentifier=MACHINE, version="1.41.0.9999", claimed=True))
        if p == "/":
            return self.send_json(mc(friendlyName="Mock Server", machineIdentifier=MACHINE, version="1.41.0.9999",
                                     platform="MacOSX", platformVersion="27.0", myPlexUsername="tester",
                                     myPlexSubscription=True, transcoderVideo=True))
        if p == "/library/sections":
            return self.send_json(mc(Directory=[
                {"key": "1", "title": "Movies", "type": "movie", "agent": "tv.plex.agents.movie", "scanner": "Plex Movie",
                 "Location": [{"id": 1, "path": "/media/movies"}], "scannedAt": int(time.time()) - 3600},
                {"key": "2", "title": "TV Shows", "type": "show", "agent": "tv.plex.agents.series", "scanner": "Plex TV Series",
                 "Location": [{"id": 2, "path": "/media/tv"}], "scannedAt": int(time.time()) - 7200},
                {"key": "3", "title": "Music", "type": "artist", "agent": "tv.plex.agents.music", "scanner": "Plex Music",
                 "Location": [{"id": 3, "path": "/media/music"}], "scannedAt": int(time.time()) - 7200},
            ] + ([{"key": "4", "title": "Archive", "type": "movie", "agent": "tv.plex.agents.movie", "scanner": "Plex Movie",
                   "Location": [{"id": 4, "path": "/media/archive"}], "scannedAt": int(time.time()) - 7200}] if LARGE else [])))
        if p == "/hubs":
            movies = all_movies()
            cont = [m for m in movies + [episode(0, 1, 2)] if (m.get("viewOffset") or m["type"] == "episode") and m["ratingKey"] not in HIDDEN]
            return self.send_json(mc(Hub=[
                {"hubIdentifier": "home.continue", "title": "Continue Watching", "type": "mixed", "size": len(cont), "Metadata": cont},
                {"hubIdentifier": "movie.recentlyadded.1", "title": "Recently Added Movies", "type": "movie", "more": True,
                 "key": "/library/sections/1/all?sort=addedAt:desc", "size": 12, "Metadata": movies[:12]},
                {"hubIdentifier": "tv.recentlyadded.2", "title": "Recently Added TV", "type": "show", "size": 4, "Metadata": all_shows()},
                {"hubIdentifier": "movie.toprated", "title": "Top Rated", "type": "movie", "size": 8,
                 "Metadata": sorted(movies, key=lambda m: -m["audienceRating"])[:8]},
            ]))
        if re.fullmatch(r"/hubs/metadata/\d+", p):
            movies = all_movies()
            return self.send_json(mc(Hub=[{"hubIdentifier": "similar", "title": "More Like This", "type": "movie", "Metadata": movies[3:11]},
                                          {"hubIdentifier": "director", "title": "More Directed by Ada Lindqvist", "type": "movie",
                                           "Metadata": movies[12:16]}]))
        m = re.fullmatch(r"/hubs/sections/(\d+)", p)
        if m:
            items = all_movies() if m.group(1) == "1" else all_shows()
            return self.send_json(mc(Hub=[{"hubIdentifier": "section.recent", "title": "Recently Added", "type": "movie", "Metadata": items[:10]},
                                          {"hubIdentifier": "section.random", "title": "Rediscover", "type": "movie", "Metadata": items[::-1][:10]}]))
        m = re.fullmatch(r"/library/sections/(\d+)/(all|sorts|genre|collections|filters)", p)
        if m:
            sid, what = m.groups()
            if what == "sorts":
                return self.send_json(mc(Directory=[{"key": k, "title": t, "descKey": f"{k}:desc", "defaultDirection": "asc"} for k, t in
                                                    [("titleSort", "Title"), ("addedAt", "Date Added"), ("year", "Year"), ("rating", "Rating")]]))
            if what == "genre":
                return self.send_json(mc(Directory=[{"key": str(10 + i), "title": g} for i, g in enumerate(GENRES)]))
            if what == "collections":
                return self.send_json(mc(Metadata=[]))
            items = all_movies() if sid == "1" else [artist()] if sid == "3" else [big_movie(i) for i in range(LARGE)] if sid == "4" else all_shows()
            if sid == "4":
                self.record("big library page", q)
            if sid == "3" and q.get("type") in (["9"], ["10"]):
                items = [album()] if q["type"] == ["9"] else children("3100")
            if sid == "2" and q.get("type") in (["3"], ["4"]):
                seasons = [s for sh in items for s in children(sh["ratingKey"])]
                items = seasons if q["type"] == ["3"] else [e for s in seasons for e in children(s["ratingKey"])]
            for tag, field in (("genre", "Genre"), ("actor", "Role"), ("director", "Director")):
                for want in q.get(tag, []):
                    items = [i for i in items if any(str(g["id"]) == want for g in i.get(field, []))]
            if "year" in q:
                items = [i for i in items if str(i.get("year")) in q["year"]]
            if q.get("hdr") == ["1"]:
                items = [i for i in items if int(i["ratingKey"]) % 4 == 0]
            if q.get("unwatched") == ["1"]:
                items = [i for i in items if not i.get("viewCount")]
            sort = q.get("sort", ["titleSort"])[0]
            key = sort.split(":")[0]
            keyf = {"titleSort": lambda i: i["title"], "addedAt": lambda i: i.get("addedAt", 0), "year": lambda i: i.get("year", 0),
                    "rating": lambda i: i.get("rating", 0)}.get(key, lambda i: i["title"])
            items = sorted(items, key=keyf, reverse=sort.endswith(":desc"))
            return self.send_json(page(items, q))
        m = re.fullmatch(r"/library/metadata/(\d+(?:,\d+)+)", p)
        if m:
            return self.send_json(mc(Metadata=[i for i in map(find, m.group(1).split(",")) if i]))
        m = re.fullmatch(r"/library/metadata/(\d+)(/children|/related|/allLeaves)?", p)
        if m:
            rk, sub = m.groups()
            if rk in MISSING_METADATA:
                return self.send_json(mc(), status=404)
            if sub == "/allLeaves":
                return self.send_json(mc(Metadata=leaves(rk)))
            if sub == "/children":
                return self.send_json(mc(Metadata=children(rk)))
            if sub == "/related":
                return self.send_json(mc(Hub=[]))
            item = find(rk)
            if item is None:
                return self.send_json(mc(Metadata=[]))
            item = dict(item)
            if item["type"] == "show":
                item["OnDeck"] = {"Metadata": [episode(int(rk) - 500, 1, 2)]}
            item["Related"] = {"Hub": [{"hubIdentifier": "similar", "title": "More Like This", "type": "movie", "Metadata": all_movies()[3:11]}]}
            item["Extras"] = {"size": 1, "Metadata": [dict(movie(7), type="clip", title="Official Trailer", subtype="trailer", thumb=img("art", 2))]}
            return self.send_json(mc(Metadata=[item]))
        if p == "/playQueues":
            uri = q.get("uri", [""])[0]
            rk = uri.rsplit("/", 1)[-1].split(",")[0]
            item = find(rk)
            items = []
            if item and item["type"] == "episode" and q.get("continuous") == ["1"]:
                n = int(rk)
                items = [episode((n - 1000) // 100, (n % 100) // 10, e) for e in range(n % 10, 4)]
            elif item and item["type"] in ("artist", "album"):
                items = leaves(rk)
                if q.get("shuffle") == ["1"]:
                    items = items[1:] + items[:1]
            elif item and item["type"] in ("show", "season"):
                eps = []
                for s in (children(rk) if item["type"] == "show" else [item]):
                    eps += children(s["ratingKey"])
                items = eps
            elif item:
                items = [item]
            for idx, it in enumerate(items):
                it["playQueueItemID"] = 9000 + idx
            key = q.get("key", [None])[0]
            sel = next((it["playQueueItemID"] for it in items if key and it["key"] == key), 9000)
            self.record("playqueue", q)
            return self.send_json(mc(Metadata=items, playQueueID=77, playQueueSelectedItemID=sel, playQueueTotalCount=len(items)))
        if p == "/hubs/search":
            query = q.get("query", [""])[0].lower()
            ms = [m for m in all_movies() if query in m["title"].lower()]
            ss = [s for s in all_shows() if query in s["title"].lower()]
            hubs = []
            if ms:
                hubs.append({"hubIdentifier": "movie", "title": "Movies", "type": "movie", "Metadata": ms})
            if ss:
                hubs.append({"hubIdentifier": "show", "title": "Shows", "type": "show", "Metadata": ss})
            if "mara".startswith(query[:4]) and query:
                hubs.append({"hubIdentifier": "actor", "title": "Actors", "type": "actor",
                             "Directory": [{"key": "/library/sections/1/all?actor=300", "tag": "Mara Okafor", "type": "actor", "count": 20}]})
            return self.send_json(mc(Hub=hubs))
        if p == "/status/sessions":
            playing = dict(movie(5), viewOffset=60_000, User={"id": "1", "title": "tester"},
                           Player={"title": "Living room TV", "product": "Plex for Apple TV", "platform": "tvOS", "state": "playing",
                                   "address": "192.168.1.30", "local": True},
                           Session={"id": "sess-1", "bandwidth": 8200, "location": "lan"},
                           TranscodeSession={"key": "/transcode/sessions/tr-1", "videoDecision": "transcode", "audioDecision": "copy",
                                             "sourceVideoCodec": "hevc", "videoCodec": "h264", "speed": 2.4})
            return self.send_json(mc(Metadata=[playing]))
        if p == "/status/sessions/terminate":
            self.record("terminate", q)
            return self.send_json(mc())
        if p == "/status/sessions/history/all":
            hist = [dict(movie(i % 20), historyKey=f"/status/sessions/history/{i + 1}", viewedAt=int(time.time()) - 5000 - i * 3600,
                         accountID=1) for i in range(75) if str(i + 1) not in DELETED]
            return self.send_json(page(hist, q))
        m = re.fullmatch(r"/status/sessions/history/(\d+)", p)
        if m and self.command == "DELETE":
            self.record("delete history", q)
            DELETED.add(m.group(1))
            return self.send_json(mc())
        if p == "/accounts":
            return self.send_json(mc(Account=[{"id": 1, "name": "tester"}]))
        if p == "/playlists":
            return self.send_json(mc(Metadata=[]))
        if p == "/:/prefs" and self.command == "PUT":
            self.record("PUT /:/prefs", q)
            PREFS.update({k: v[0] for k, v in q.items() if not k.startswith("X-Plex")})
            return self.send_json(mc())
        if p == "/:/prefs":
            return self.send_json(mc(Setting=[{**s, "value": pref_value(s, PREFS)} for s in SERVER_PREFS]))
        if p == "/butler":
            def task(name, title, description, interval):
                on = pref_value({"id": "ButlerTask" + name, "type": "bool", "value": True}, PREFS)
                return {"name": name, "title": title, "description": description, "interval": interval, "enabled": on}
            return self.send_json({"ButlerTasks": {"ButlerTask": [
                task("BackupDatabase", "Backup Database", "Create a backup copy of the server's database.", 3),
                task("OptimizeDatabase", "Optimize Database", "Compact and re-index.", 7),
                task("CleanOldBundles", "Clean Old Bundles", "Remove old metadata bundles.", 7),
                task("RefreshPeriodicMetadata", "Refresh Metadata Periodically", "Refresh metadata for older items.", 1),
                task("CheckForUpdates", "Check For Updates", "Look for a newer server version.", 1)]}})
        if p == "/statistics/resources":
            return self.send_json(mc(StatisticsResources=[{"timespan": 6, "at": int(time.time()), "hostCpuUtilization": 12.5,
                                                           "processCpuUtilization": 3.1, "hostMemoryUtilization": 61.0, "processMemoryUtilization": 4.2}]))
        self.record("unhandled " + self.command + " " + p, q)
        return self.send_json(mc())


def main():
    global MEDIA, DOWNLOAD_DELAY, DROP_DOWNLOAD_ONCE, RESUME_VALIDATOR, MISSING_METADATA, LARGE
    ap = argparse.ArgumentParser()
    ap.add_argument("--media", default=".")
    ap.add_argument("--port", type=int, default=32499)
    ap.add_argument("--download-delay", type=float, default=0, help="Delay each media chunk for pause/resume testing")
    ap.add_argument("--drop-download-once", action="store_true", help="Interrupt the first media transfer")
    ap.add_argument("--resume-validator", choices=["normal", "missing", "changed"], default="normal", help="Simulate unverified range responses")
    ap.add_argument("--missing-metadata", action="append", default=[], help="Return 404 for this rating key")
    ap.add_argument("--large", type=int, default=0, help="Add an Archive library and a Marathon playlist with this many entries")
    a = ap.parse_args()
    LARGE = a.large
    DOWNLOAD_DELAY = a.download_delay
    DROP_DOWNLOAD_ONCE = a.drop_download_once
    RESUME_VALIDATOR = a.resume_validator
    MISSING_METADATA = set(a.missing_metadata)
    MEDIA = a.media
    srv = ThreadingHTTPServer(("127.0.0.1", a.port), Handler)
    print(f"mock PMS on http://127.0.0.1:{srv.server_address[1]}", file=sys.stderr, flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
