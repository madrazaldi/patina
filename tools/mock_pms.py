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

GENRES = ["Drama", "Science Fiction", "Thriller", "Comedy", "Animation", "Mystery"]
MOVIE_TITLES = [
    "The Verdigris Line", "Copper Sky", "Afterglow", "Quiet Harbor", "Northbound",
    "Paper Moons", "The Long Exposure", "Salt & Static", "Glass Orchard", "Lanterns",
    "Midnight Transit", "A Field of Echoes", "Tidewater", "The Cartographer", "Low Orbit",
    "Signal Fires", "Hollow Crown", "Bright Minutes", "The Understudy", "Ember Season",
]
SHOW_TITLES = ["Station Eleven Below", "The Weatherman", "Parallel Lines", "Harbor Lights"]


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


STATE = {}
# Rating keys removed from Continue Watching.
HIDDEN = set()
# part id -> {"audio": stream id, "subtitle": stream id or 0}, set through PUT /library/parts/<id>.
SELECTED = {}


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
    return None


def leaves(rk):
    item = find(rk)
    if item is None:
        return []
    if item["type"] == "show":
        return [e for s in children(rk) for e in children(s["ratingKey"])]
    if item["type"] == "season":
        return children(rk)
    return [item]


def children(rk):
    n = int(rk)
    if 500 <= n < 600:
        return [season(n - 500, s) for s in (1, 2)]
    if 600 <= n < 700:
        return [episode((n - 600) // 10, n % 10, e) for e in (1, 2, 3)]
    return []


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
        size = os.path.getsize(path)
        rng = self.headers.get("Range")
        start, end = 0, size - 1
        if rng:
            m = re.match(r"bytes=(\d*)-(\d*)", rng)
            if m:
                start = int(m.group(1) or 0)
                end = int(m.group(2)) if m.group(2) else size - 1
            self.send_response(206)
            self.send_header("Content-Range", f"bytes {start}-{end}/{size}")
        else:
            self.send_response(200)
        self.send_header("Content-Type", ctype)
        self.send_header("Accept-Ranges", "bytes")
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
        # --- browsing extras -------------------------------------------------------
        m = re.fullmatch(r"/library/sections/(\d+)/(firstCharacter|folder)", p)
        if m:
            sid, what = m.groups()
            items = all_movies() if sid == "1" else all_shows()
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
            ]))
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
            items = all_movies() if sid == "1" else all_shows()
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
            return self.send_json(mc(Metadata=[]))
        if p == "/status/sessions/history/all":
            hist = [dict(movie(3), historyKey="/status/sessions/history/1", viewedAt=int(time.time()) - 5000, accountID=1)]
            return self.send_json(mc(Metadata=hist))
        if p == "/accounts":
            return self.send_json(mc(Account=[{"id": 1, "name": "tester"}]))
        if p == "/playlists":
            return self.send_json(mc(Metadata=[]))
        if p == "/:/prefs":
            return self.send_json(mc(Setting=[
                {"id": "FriendlyName", "label": "Friendly name", "summary": "The name of this server.", "type": "text", "value": "Mock Server"},
                {"id": "ScheduledLibraryUpdatesEnabled", "label": "Update my library periodically", "type": "bool", "value": True},
                {"id": "TranscoderQuality", "label": "Transcoder quality", "type": "int", "value": 0,
                 "enumValues": "0:Automatic|1:Prefer higher speed encoding|2:Prefer higher quality encoding"},
            ]))
        if p == "/butler":
            return self.send_json({"ButlerTasks": {"ButlerTask": [
                {"name": "BackupDatabase", "title": "Backup Database", "description": "Create a backup copy of the server's database.", "enabled": True},
                {"name": "OptimizeDatabase", "title": "Optimize Database", "description": "Compact and re-index.", "enabled": True}]}})
        if p == "/statistics/resources":
            return self.send_json(mc(StatisticsResources=[{"timespan": 6, "at": int(time.time()), "hostCpuUtilization": 12.5,
                                                           "processCpuUtilization": 3.1, "hostMemoryUtilization": 61.0, "processMemoryUtilization": 4.2}]))
        self.record("unhandled " + self.command + " " + p, q)
        return self.send_json(mc())


def main():
    global MEDIA
    ap = argparse.ArgumentParser()
    ap.add_argument("--media", default=".")
    ap.add_argument("--port", type=int, default=32499)
    a = ap.parse_args()
    MEDIA = a.media
    srv = ThreadingHTTPServer(("127.0.0.1", a.port), Handler)
    print(f"mock PMS on http://127.0.0.1:{a.port}", file=sys.stderr, flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
