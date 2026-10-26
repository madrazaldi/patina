// Plex's numeric media type ids and stream types (from crates/plex/src/lib.rs).

/** Plex's numeric media type identifiers, used for `type=` query parameters. */
export const MediaType = {
  Movie: 1,
  Show: 2,
  Season: 3,
  Episode: 4,
  Trailer: 5,
  Person: 7,
  Artist: 8,
  Album: 9,
  Track: 10,
  Clip: 12,
  Photo: 13,
  PhotoAlbum: 14,
  Playlist: 15,
  PlaylistFolder: 16,
  Collection: 18,
  /** `MediaType::id()`: the variants are already their ids. */
  id(t: MediaType): number {
    return t;
  },
  /** Map a metadata `type` string ("movie", "episode", ...) to its id. */
  fromName(name: string): MediaType | null {
    switch (name) {
      case "movie":
        return MediaType.Movie;
      case "show":
        return MediaType.Show;
      case "season":
        return MediaType.Season;
      case "episode":
        return MediaType.Episode;
      case "trailer":
        return MediaType.Trailer;
      case "person":
        return MediaType.Person;
      case "artist":
        return MediaType.Artist;
      case "album":
        return MediaType.Album;
      case "track":
        return MediaType.Track;
      case "clip":
        return MediaType.Clip;
      case "photo":
        return MediaType.Photo;
      case "photoalbum":
        return MediaType.PhotoAlbum;
      case "playlist":
        return MediaType.Playlist;
      case "playlistFolder":
        return MediaType.PlaylistFolder;
      case "collection":
        return MediaType.Collection;
      default:
        return null;
    }
  },
} as const;

/** The numeric id of a media type (the value of a `MediaType.*` member). */
export type MediaType = 1 | 2 | 3 | 4 | 5 | 7 | 8 | 9 | 10 | 12 | 13 | 14 | 15 | 16 | 18;

/** Stream types found on `Stream.streamType`. */
export const streamType = {
  VIDEO: 1,
  AUDIO: 2,
  SUBTITLE: 3,
  LYRICS: 4,
} as const;
