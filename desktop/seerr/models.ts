// Seerr's JSON, as far as Patina reads it. Every field is optional: Seerr and
// TMDB leave many out, and Overseerr, Jellyseerr and Seerr differ slightly.

export interface RawUser {
  id: number;
  email?: string;
  username?: string | null;
  plexUsername?: string | null;
  jellyfinUsername?: string | null;
  displayName?: string;
  permissions?: number;
  avatar?: string;
}

export interface RawMediaInfo {
  id?: number;
  tmdbId?: number;
  tvdbId?: number | null;
  status?: number;
  status4k?: number;
  /** Plex rating key of the copy on the server, once available. */
  ratingKey?: string | null;
  ratingKey4k?: string | null;
  requests?: RawMediaRequest[];
  seasons?: { seasonNumber: number; status?: number; status4k?: number }[];
}

export interface RawMediaRequest {
  id: number;
  status: number;
  media?: RawMediaInfo & { mediaType?: "movie" | "tv" };
  type?: "movie" | "tv";
  createdAt?: string;
  updatedAt?: string;
  requestedBy?: RawUser;
  modifiedBy?: RawUser | null;
  is4k?: boolean;
  seasons?: { seasonNumber: number; status?: number }[];
}

export interface RawResult {
  id: number;
  mediaType: "movie" | "tv" | "person" | "collection";
  title?: string;
  name?: string;
  releaseDate?: string;
  firstAirDate?: string;
  overview?: string;
  posterPath?: string | null;
  backdropPath?: string | null;
  voteAverage?: number;
  mediaInfo?: RawMediaInfo;
}

export interface RawPage<T> {
  page: number;
  totalPages: number;
  totalResults?: number;
  results: T[];
}

export interface RawRequestPage {
  pageInfo: { page: number; pages: number; results: number; pageSize?: number };
  results: RawMediaRequest[];
}

interface RawDetails {
  id: number;
  overview?: string;
  posterPath?: string | null;
  backdropPath?: string | null;
  voteAverage?: number;
  genres?: { id: number; name: string }[];
  tagline?: string;
  status?: string;
  mediaInfo?: RawMediaInfo;
}

export interface RawMovieDetails extends RawDetails {
  title?: string;
  releaseDate?: string;
  runtime?: number;
}

export interface RawTvDetails extends RawDetails {
  name?: string;
  firstAirDate?: string;
  episodeRunTime?: number[];
  numberOfSeasons?: number;
  seasons?: { seasonNumber: number; name?: string; episodeCount?: number; airDate?: string | null }[];
  externalIds?: { tvdbId?: number | null };
}

export interface RawService {
  id: number;
  name: string;
  is4k: boolean;
  isDefault: boolean;
  activeProfileId?: number;
  activeDirectory?: string;
}

export interface RawServiceDetails {
  server: RawService;
  profiles: { id: number; name: string }[];
  rootFolders?: { id: number; path: string; freeSpace?: number }[];
}
