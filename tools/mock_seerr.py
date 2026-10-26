#!/usr/bin/env python3
"""A tiny fake Seerr (Overseerr/Jellyseerr) server for developing and testing Patina offline.

It serves the parts of the Seerr API (/api/v1) that Patina uses: status, sign-in
with an API key or a Plex token, search, discover lists, movie and show details,
requests (create, list, approve, decline, retry, delete) and the Radarr/Sonarr
service lists. Every request that changes something is logged to stdout as JSON.

    python3 tools/mock_seerr.py --port 5055 [--media /path/to/dir]

Sign in with API key `mock-key` (an admin), or with any Plex token (a user who
may request, without auto-approval). With `--media`, posters come from
`img/poster{0..7}.jpg` there, as with tools/mock_pms.py.
"""

import argparse
import json
import os
import re
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

API_KEY = "mock-key"
MEDIA = None
LOCK = threading.Lock()
# connect.sid value -> user id.
SESSIONS = {}

ADMIN = 2
REQUEST = 32
USERS = {
    1: {"id": 1, "email": "admin@example.com", "displayName": "Mock Admin", "permissions": ADMIN, "userType": 1},
    2: {"id": 2, "email": "alice@example.com", "displayName": "Alice", "plexUsername": "alice", "permissions": REQUEST,
        "userType": 1},
}
OVERVIEW = "A mock-server title used to exercise Patina's Seerr pages."

# tmdb id -> movie. `status` is Seerr's MediaStatus (None when unknown to Seerr); `ratingKey` is the
# copy on tools/mock_pms.py ("The Verdigris Line" is its movie 100).
MOVIES = {
    9001: {"title": "The Verdigris Line", "releaseDate": "2014-05-01", "poster": 0, "status": 5, "ratingKey": "100"},
    9002: {"title": "Lighthouse Keepers", "releaseDate": "2021-09-17", "poster": 1, "status": None},
    9003: {"title": "Northern Static", "releaseDate": "2023-02-03", "poster": 2, "status": None},
    9004: {"title": "Paper Satellites", "releaseDate": "2024-11-08", "poster": 3, "status": None},
    9005: {"title": "Long Way Down River", "releaseDate": "2019-04-12", "poster": 4, "status": None},
    9006: {"title": "Kid's Own (Director's Cut)!", "releaseDate": "2012-06-30", "poster": 5, "status": None},
}
# tmdb id -> show; `seasons` maps season number -> episode count, `available` lists seasons on the server.
SHOWS = {
    8001: {"name": "Station Eleven Below", "firstAirDate": "2019-01-10", "poster": 6, "seasons": {1: 3, 2: 3, 3: 8},
           "available": [1, 2], "ratingKey": "500", "tvdbId": 7001},
    8002: {"name": "Ferry Season", "firstAirDate": "2022-03-04", "poster": 7, "seasons": {1: 10, 2: 10, 3: 6},
           "available": [], "tvdbId": 7002},
    8003: {"name": "Glass Weather", "firstAirDate": "2025-08-21", "poster": 2, "seasons": {1: 8}, "available": [],
           "tvdbId": 7003},
}
REQUESTS = []
NEXT_ID = [1]
PORT = [0]


def now():
    return time.strftime("%Y-%m-%dT%H:%M:%S.000Z", time.gmtime())


def poster(n):
    if MEDIA and os.path.exists(os.path.join(MEDIA, "img", f"poster{n}.jpg")):
        return f"http://127.0.0.1:{PORT[0]}/img/poster{n}.jpg"
    return f"/mock{n}.jpg"


def requests_for(kind, tmdb):
    return [r for r in REQUESTS if r["type"] == kind and r["media"]["tmdbId"] == tmdb]


def media_status(kind, tmdb, is4k=False):
    """MediaStatus of a title: available on the server, else from its open requests."""
    if not is4k:
        if kind == "movie" and MOVIES[tmdb]["status"]:
            return MOVIES[tmdb]["status"]
        if kind == "tv":
            s = SHOWS[tmdb]
            if s["available"]:
                return 5 if len(s["available"]) == len(s["seasons"]) else 4
    open_ = [r for r in requests_for(kind, tmdb) if r["is4k"] == is4k and r["status"] in (1, 2)]
    if any(r["status"] == 2 for r in open_):
        return 3
    if open_:
        return 2
    return None


def media_info(kind, tmdb, with_requests=True):
    status, status4k = media_status(kind, tmdb), media_status(kind, tmdb, True)
    reqs = requests_for(kind, tmdb)
    if status is None and status4k is None and not reqs:
        return None
    info = {"id": tmdb, "tmdbId": tmdb, "mediaType": kind, "status": status or 1, "status4k": status4k or 1}
    entry = MOVIES.get(tmdb) if kind == "movie" else SHOWS.get(tmdb)
    if entry.get("ratingKey") and status in (4, 5):
        info["ratingKey"] = entry["ratingKey"]
    if kind == "tv":
        s = SHOWS[tmdb]
        info["tvdbId"] = s["tvdbId"]
        info["seasons"] = [{"seasonNumber": n, "status": 5 if n in s["available"] else 1, "status4k": 1} for n in s["seasons"]]
    if with_requests:
        info["requests"] = [dict(r, media={"id": tmdb, "tmdbId": tmdb}) for r in reqs]
    return info


def movie_result(tmdb):
    m = MOVIES[tmdb]
    out = {"id": tmdb, "mediaType": "movie", "title": m["title"], "releaseDate": m["releaseDate"], "overview": OVERVIEW,
           "posterPath": poster(m["poster"]), "backdropPath": poster(m["poster"]), "voteAverage": 7.2}
    info = media_info("movie", tmdb, False)
    if info:
        out["mediaInfo"] = info
    return out


def tv_result(tmdb):
    s = SHOWS[tmdb]
    out = {"id": tmdb, "mediaType": "tv", "name": s["name"], "firstAirDate": s["firstAirDate"], "overview": OVERVIEW,
           "posterPath": poster(s["poster"]), "backdropPath": poster(s["poster"]), "voteAverage": 8.1}
    info = media_info("tv", tmdb, False)
    if info:
        out["mediaInfo"] = info
    return out


def all_results():
    return [movie_result(t) for t in MOVIES] + [tv_result(t) for t in SHOWS] + [
        {"id": 1, "mediaType": "person", "name": "Ada Lindqvist", "knownFor": []}]


def page_of(results, q):
    page = max(1, int(q.get("page", ["1"])[0] or 1))
    size = 20
    return {"page": page, "totalPages": max(1, -(-len(results) // size)), "totalResults": len(results),
            "results": results[(page - 1) * size:page * size]}


SERVICES = {
    "radarr": [{"id": 0, "name": "Radarr", "is4k": False, "isDefault": True, "activeProfileId": 1, "activeDirectory": "/movies"},
               {"id": 1, "name": "Radarr 4K", "is4k": True, "isDefault": True, "activeProfileId": 2, "activeDirectory": "/movies-4k"}],
    "sonarr": [{"id": 0, "name": "Sonarr", "is4k": False, "isDefault": True, "activeProfileId": 1, "activeDirectory": "/tv"}],
}


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):
        pass

    def send_json(self, obj, status=200, headers=None):
        body = b"" if obj is None else json.dumps(obj).encode()
        self.send_response(status)
        if obj is not None:
            self.send_header("Content-Type", "application/json")
        for k, v in (headers or {}).items():
            self.send_header(k, v)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def record(self, what, **fields):
        print(json.dumps({"t": round(time.time(), 1), "what": what, **fields}), flush=True)

    def body(self):
        n = int(self.headers.get("Content-Length") or 0)
        return json.loads(self.rfile.read(n) or b"{}") if n else {}

    def user(self):
        if self.headers.get("X-Api-Key") == API_KEY:
            return USERS[1]
        m = re.search(r"connect\.sid=([^;]+)", self.headers.get("Cookie") or "")
        if m and m.group(1) in SESSIONS:
            return USERS[SESSIONS[m.group(1)]]
        return None

    def do_GET(self):
        self.route("GET")

    def do_POST(self):
        self.route("POST")

    def do_DELETE(self):
        self.route("DELETE")

    def route(self, method):
        u = urlparse(self.path)
        p, q = u.path, parse_qs(u.query)
        if p.startswith("/img/") and MEDIA and method == "GET":
            path = os.path.join(MEDIA, "img", os.path.basename(p))
            if os.path.exists(path):
                with open(path, "rb") as f:
                    data = f.read()
                self.send_response(200)
                self.send_header("Content-Type", "image/jpeg")
                self.send_header("Content-Length", str(len(data)))
                self.end_headers()
                self.wfile.write(data)
                return
            return self.send_json({"message": "Not found"}, 404)
        if not p.startswith("/api/v1/"):
            return self.send_json({"message": "Not found"}, 404)
        p = p[len("/api/v1"):]
        # Seerr's query validation rejects these characters unless percent-encoded.
        if re.search(r"[ !'()*]", u.query):
            return self.send_json({"message": "request/query/query must be url encoded"}, 400)
        if p == "/status":
            return self.send_json({"version": "2.7.3", "commitTag": "mock"})
        if p == "/auth/plex" and method == "POST":
            token = self.body().get("authToken")
            if not token or token == "bad":
                return self.send_json({"message": "Unable to authenticate."}, 403)
            sid = f"mock-{len(SESSIONS) + 1}"
            SESSIONS[sid] = 2
            self.record("plex-sign-in")
            return self.send_json(USERS[2], headers={"Set-Cookie": f"connect.sid={sid}; Path=/; HttpOnly"})
        user = self.user()
        if not user:
            return self.send_json({"message": "You do not have permission to access this endpoint."}, 403)
        manage = bool(user["permissions"] & ADMIN)
        with LOCK:
            return self.authed(method, p, q, user, manage)

    def authed(self, method, p, q, user, manage):
        if p == "/auth/me":
            return self.send_json(user)
        if p == "/search":
            text = q.get("query", [""])[0].lower()
            return self.send_json(page_of([r for r in all_results() if text in (r.get("title") or r.get("name") or "").lower()], q))
        if p == "/discover/trending":
            return self.send_json(page_of(all_results(), q))
        if p in ("/discover/movies", "/discover/movies/upcoming"):
            return self.send_json(page_of([movie_result(t) for t in MOVIES], q))
        if p in ("/discover/tv", "/discover/tv/upcoming"):
            return self.send_json(page_of([tv_result(t) for t in SHOWS], q))
        m = re.fullmatch(r"/(movie|tv)/(\d+)", p)
        if m:
            kind, tmdb = m.group(1), int(m.group(2))
            if tmdb not in (MOVIES if kind == "movie" else SHOWS):
                return self.send_json({"message": "Unable to retrieve details."}, 404)
            if kind == "movie":
                out = dict(movie_result(tmdb), runtime=112, tagline="Every signal leaves a trace.",
                           genres=[{"id": 18, "name": "Drama"}])
            else:
                s = SHOWS[tmdb]
                out = dict(tv_result(tmdb), episodeRunTime=[45], genres=[{"id": 9648, "name": "Mystery"}],
                           externalIds={"tvdbId": s["tvdbId"]}, numberOfSeasons=len(s["seasons"]),
                           seasons=[{"seasonNumber": 0, "name": "Specials", "episodeCount": 2}] + [
                               {"seasonNumber": n, "name": f"Season {n}", "episodeCount": c, "airDate": f"{2018 + n}-01-10"}
                               for n, c in s["seasons"].items()])
            info = media_info(kind, tmdb)
            if info:
                out["mediaInfo"] = info
            return self.send_json(out)
        if p == "/request" and method == "GET":
            return self.list_requests(q, user, manage)
        if p == "/request" and method == "POST":
            return self.create_request(self.body(), user, manage)
        m = re.fullmatch(r"/request/(\d+)(?:/(approve|decline|retry))?", p)
        if m:
            req = next((r for r in REQUESTS if r["id"] == int(m.group(1))), None)
            if not req:
                return self.send_json({"message": "Request not found."}, 404)
            action = m.group(2)
            if method == "DELETE" and not action:
                if not manage and (req["requestedBy"]["id"] != user["id"] or req["status"] != 1):
                    return self.send_json({"message": "You do not have permission to delete this request."}, 401)
                REQUESTS.remove(req)
                self.record("request-delete", id=req["id"])
                return self.send_json(None, 204)
            if method == "POST" and action:
                if not manage:
                    return self.send_json({"message": "You do not have permission to access this endpoint."}, 403)
                if action in ("approve", "decline") and req["status"] != 1:
                    return self.send_json({"message": "Only pending requests can be approved or declined."}, 409)
                if action == "retry" and req["status"] != 4:
                    return self.send_json({"message": "Only failed requests can be retried."}, 409)
                req["status"] = {"approve": 2, "decline": 3, "retry": 2}[action]
                req["modifiedBy"], req["updatedAt"] = user, now()
                self.record(f"request-{action}", id=req["id"])
                return self.send_json(req)
        m = re.fullmatch(r"/service/(radarr|sonarr)(?:/(\d+))?", p)
        if m and method == "GET":
            services = SERVICES[m.group(1)]
            if m.group(2) is None:
                return self.send_json(services)
            server = next((s for s in services if s["id"] == int(m.group(2))), None)
            if not server:
                return self.send_json({"message": "Not found"}, 404)
            root = server["activeDirectory"]
            return self.send_json({"server": server, "profiles": [{"id": 1, "name": "HD-1080p"}, {"id": 2, "name": "Ultra-HD"}],
                                   "rootFolders": [{"id": 1, "path": root, "freeSpace": 4 * 10 ** 12},
                                                   {"id": 2, "path": f"{root}-archive", "freeSpace": 10 ** 12}]})
        return self.send_json({"message": "Not found"}, 404)

    def list_requests(self, q, user, manage):
        take = int(q.get("take", ["20"])[0])
        skip = int(q.get("skip", ["0"])[0])
        want = q.get("filter", ["all"])[0]
        by = q.get("requestedBy", [None])[0]
        out = []
        for r in sorted(REQUESTS, key=lambda r: r["id"], reverse=True):
            if not manage and r["requestedBy"]["id"] != user["id"]:
                continue
            if by and r["requestedBy"]["id"] != int(by):
                continue
            status = media_status(r["type"], r["media"]["tmdbId"], r["is4k"])
            if want == "pending" and r["status"] != 1:
                continue
            if want == "approved" and r["status"] != 2:
                continue
            if want == "failed" and r["status"] != 4:
                continue
            if want == "available" and status not in (4, 5):
                continue
            if want in ("processing", "unavailable") and (r["status"] != 2 or status in (4, 5)):
                continue
            out.append(dict(r, media=dict(media_info(r["type"], r["media"]["tmdbId"], False) or {}, tmdbId=r["media"]["tmdbId"])))
        page = out[skip:skip + take]
        return self.send_json({"pageInfo": {"page": skip // max(take, 1) + 1, "pages": max(1, -(-len(out) // max(take, 1))),
                                            "results": len(out), "pageSize": take}, "results": page})

    def create_request(self, b, user, manage):
        kind, tmdb = b.get("mediaType"), b.get("mediaId")
        table = MOVIES if kind == "movie" else SHOWS if kind == "tv" else None
        if table is None or tmdb not in table:
            return self.send_json({"message": "Media not found."}, 404)
        is4k = bool(b.get("is4k"))
        seasons = []
        if kind == "tv":
            seasons = b.get("seasons")
            if seasons == "all":
                seasons = list(SHOWS[tmdb]["seasons"])
            if not seasons:
                return self.send_json({"message": "No seasons requested."}, 400)
            taken = set(SHOWS[tmdb]["available"] if not is4k else [])
            for r in requests_for(kind, tmdb):
                if r["is4k"] == is4k and r["status"] not in (3, 5):
                    taken.update(s["seasonNumber"] for s in r["seasons"])
            # Like Seerr: seasons already requested or available are dropped; none left is a 202.
            seasons = [n for n in seasons if n not in taken]
            if not seasons:
                return self.send_json({"message": "No seasons available to request"}, 202)
        elif media_status(kind, tmdb, is4k) in (2, 3, 4, 5):
            return self.send_json({"message": "Request for this media already exists."}, 409)
        req = {"id": NEXT_ID[0], "status": 2 if manage else 1, "type": kind, "is4k": is4k, "createdAt": now(),
               "updatedAt": now(), "requestedBy": user, "modifiedBy": user if manage else None,
               "media": {"tmdbId": tmdb}, "seasons": [{"id": n, "seasonNumber": n, "status": 1} for n in seasons],
               "serverId": b.get("serverId"), "profileId": b.get("profileId"), "rootFolder": b.get("rootFolder")}
        NEXT_ID[0] += 1
        REQUESTS.append(req)
        self.record("request", mediaType=kind, mediaId=tmdb, seasons=seasons, is4k=is4k, by=user["id"],
                    serverId=b.get("serverId"), profileId=b.get("profileId"), rootFolder=b.get("rootFolder"),
                    tvdbId=b.get("tvdbId"))
        return self.send_json(dict(req, media=media_info(kind, tmdb, False)), 201)


def main():
    global API_KEY, MEDIA
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=5055)
    ap.add_argument("--media", default=None, help="Folder with img/poster{0..7}.jpg (as for mock_pms.py)")
    ap.add_argument("--api-key", default=API_KEY, help="API key of the admin user")
    ap.add_argument("--pending", action="store_true", help="Start with a pending request by Alice for Northern Static")
    a = ap.parse_args()
    API_KEY, MEDIA = a.api_key, a.media
    if a.pending:
        REQUESTS.append({"id": NEXT_ID[0], "status": 1, "type": "movie", "is4k": False, "createdAt": now(), "updatedAt": now(),
                         "requestedBy": USERS[2], "modifiedBy": None, "media": {"tmdbId": 9003}, "seasons": []})
        NEXT_ID[0] += 1
    srv = ThreadingHTTPServer(("127.0.0.1", a.port), Handler)
    PORT[0] = srv.server_address[1]
    print(f"mock Seerr on http://127.0.0.1:{PORT[0]}", file=sys.stderr, flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
