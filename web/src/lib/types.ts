// Shapes returned by the Rust commands. Plex models keep Plex's own JSON field
// names (see crates/plex/src/models); only fields the UI reads are listed.

export interface Tag {
  id?: number;
  tag?: string;
  filter?: string;
  role?: string;
  thumb?: string;
  count?: number;
}

export interface Stream {
  id?: number;
  streamType?: number;
  selected: boolean;
  forced: boolean;
  codec?: string;
  index?: number;
  language?: string;
  languageCode?: string;
  title?: string;
  displayTitle?: string;
  extendedDisplayTitle?: string;
  key?: string;
  channels?: number;
  colorTrc?: string;
  DOVIPresent: boolean;
  hearingImpaired: boolean;
  format?: string;
  // Online subtitle search results.
  providerTitle?: string;
  score?: number;
  perfectMatch: boolean;
  downloaded: boolean;
}

export interface Part {
  id?: number;
  key?: string;
  file?: string;
  size?: number;
  Stream: Stream[];
}

export interface Media {
  id?: number;
  bitrate?: number;
  width?: number;
  height?: number;
  audioChannels?: number;
  audioCodec?: string;
  videoCodec?: string;
  videoResolution?: string;
  container?: string;
  title?: string;
  editionTitle?: string;
  Part: Part[];
}

export interface UltraBlurColors {
  topLeft?: string;
  topRight?: string;
  bottomRight?: string;
  bottomLeft?: string;
}

export interface Nested {
  Metadata: Metadata[];
  Hub: Hub[];
}

export interface SessionUser {
  id?: string;
  title?: string;
  thumb?: string;
}

export interface Player {
  address?: string;
  product?: string;
  state?: string;
  title?: string;
}

export interface SessionInfo {
  id?: string;
  bandwidth?: number;
  location?: string;
}

export interface TranscodeSession {
  key?: string;
  throttled: boolean;
  progress?: number;
  speed?: number;
  context?: string;
  sourceVideoCodec?: string;
  videoDecision?: string;
  audioDecision?: string;
  protocol?: string;
  videoCodec?: string;
  audioCodec?: string;
  transcodeHwRequested: boolean;
}

export interface Metadata {
  ratingKey?: string;
  key?: string;
  guid?: string;
  type?: string;
  title?: string;
  originalTitle?: string;
  studio?: string;
  contentRating?: string;
  summary?: string;
  tagline?: string;
  year?: number;
  index?: number;
  parentIndex?: number;
  rating?: number;
  audienceRating?: number;
  userRating?: number;
  ratingImage?: string;
  audienceRatingImage?: string;
  viewCount?: number;
  viewOffset?: number;
  lastViewedAt?: number;
  addedAt?: number;
  originallyAvailableAt?: string;
  duration?: number;
  thumb?: string;
  art?: string;
  composite?: string;
  titleSort?: string;
  editionTitle?: string;
  parentRatingKey?: string;
  parentTitle?: string;
  parentThumb?: string;
  parentArt?: string;
  grandparentRatingKey?: string;
  grandparentTitle?: string;
  grandparentThumb?: string;
  grandparentArt?: string;
  leafCount?: number;
  viewedLeafCount?: number;
  childCount?: number;
  librarySectionID?: number;
  playlistType?: string;
  smart: boolean;
  playlistItemID?: number;
  playQueueItemID?: number;
  /** Collections: -1 library default, 0 hide, 1 hide items, 2 show items. */
  collectionMode?: number;
  /** Collections: "0" release date, "1" title, "2" custom (a string on most servers). */
  collectionSort?: string | number;
  historyKey?: string;
  viewedAt?: number;
  accountID?: number;
  sessionKey?: string;
  User?: SessionUser;
  Player?: Player;
  Session?: SessionInfo;
  TranscodeSession?: TranscodeSession;
  Media: Media[];
  Genre: Tag[];
  Director: Tag[];
  Writer: Tag[];
  Country: Tag[];
  Collection: Tag[];
  Label: Tag[];
  Mood: Tag[];
  Style: Tag[];
  Role: Tag[];
  Image: { type?: string; url?: string }[];
  UltraBlurColors?: UltraBlurColors;
  Extras?: Nested;
  Related?: Nested;
  OnDeck?: Nested;
}

export interface Directory {
  key?: string;
  title?: string;
  type?: string;
  thumb?: string;
  count?: number;
  /** Item count of a first-character entry. */
  size?: number;
  tag?: string;
  filter?: string;
  descKey?: string;
  agent?: string;
  scanner?: string;
  language?: string;
  refreshing: boolean;
  scannedAt?: number;
  hidden: boolean;
  Location: { id?: number; path?: string }[];
}

export interface Hub {
  hubKey?: string;
  key?: string;
  title?: string;
  type?: string;
  hubIdentifier?: string;
  more: boolean;
  Metadata: Metadata[];
  Directory: Directory[];
}

export interface MediaContainer {
  size?: number;
  totalSize?: number;
  offset?: number;
  Metadata: Metadata[];
  Directory: Directory[];
}

export interface HomeUser {
  uuid?: string;
  title?: string;
  username?: string;
  thumb?: string;
  protected: boolean;
}

export interface Activity {
  uuid?: string;
  cancellable: boolean;
  title?: string;
  subtitle?: string;
  progress?: number;
}

export interface Account {
  id?: number;
  name?: string;
  defaultAudioLanguage?: string;
  defaultSubtitleLanguage?: string;
}

export interface Device {
  id?: number;
  name?: string;
  platform?: string;
  createdAt?: number;
}

export interface ServerDevice {
  name?: string;
  address?: string;
  machineIdentifier?: string;
  product?: string;
}

export interface Release {
  version?: string;
  added?: string;
  fixed?: string;
  downloadUrl?: string;
}

export interface ButlerTask {
  name?: string;
  title?: string;
  description?: string;
  /** Days between runs. */
  interval?: number;
  enabled: boolean;
}

// App-level shapes (crates/app/src/state.rs, commands.rs).

export interface UserView {
  uuid?: string;
  name: string;
  email?: string;
  thumb?: string;
  plexPass: boolean;
}

export interface ServerView {
  id: string;
  name: string;
  owned: boolean;
  sourceTitle?: string;
  platform?: string;
  version?: string;
  presence: boolean;
}

export type Stage =
  | { kind: "booting" }
  | { kind: "login" }
  | { kind: "pickServer"; servers: ServerView[]; user: UserView }
  | { kind: "connecting"; name: string }
  | { kind: "ready" }
  | { kind: "failed"; message: string };

export interface SessionView {
  accountId: string;
  serverName: string;
  serverId: string;
  machineIdentifier?: string;
  baseUrl: string;
  owned: boolean;
  hasTv: boolean;
  user?: UserView;
  servers: ServerView[];
}

export interface NowPlaying {
  item: Metadata;
  positionMs: number;
  durationMs: number;
  paused: boolean;
  transcoding: boolean;
  queueLen: number;
  queuePos: number;
  /** Queue entry id of the item (see QueueView). */
  uid: number;
  /** In the closing credits / last seconds with something queued after: show "Up next". */
  ending: boolean;
  /** Playback stops when this item ends. */
  stopAfter: boolean;
  live: boolean;
  /** A downloaded file played offline. */
  local: boolean;
}

export interface QueueItem {
  uid: number;
  item: Metadata;
}

/** The running play queue in play order. Absent when nothing plays (or for offline files). */
export interface QueueView {
  items: QueueItem[];
  current?: number;
  shuffled: boolean;
  stopAfter: boolean;
}

/** A background task on the server (scan, analysis, ...), live from notifications. */
export interface ServerActivity {
  uuid: string;
  /** e.g. "library.update.section", "library.refresh.items", "media.generate.intros" */
  kind: string;
  title: string;
  subtitle?: string;
  progress: number;
  cancellable: boolean;
  sectionId?: string;
}

export interface Snapshot {
  connected: boolean;
  stage: Stage;
  session?: SessionView;
  sections: Directory[];
  nowPlaying?: NowPlaying;
  queue?: QueueView;
  activities: ServerActivity[];
  revision: number;
}

export interface Choice {
  id: string;
  label: string;
  color?: string;
}

export interface Info {
  version: string;
  configPath: string;
  mpvConfigDir: string;
  mpvConfExists: boolean;
  qualities: Choice[];
  accents: Choice[];
  debugRoute?: string;
  debugPlay?: string;
}

// Stored as TOML, so these keep snake_case.
export interface PlayerSettings {
  mpv_path: string;
  extra_args: string;
  quality: string;
  auto_skip_intro: boolean;
  auto_skip_credits: boolean;
  skip_key: string;
  sync_streams: boolean;
  watched_threshold: number;
  continuous: boolean;
  media_controls: boolean;
  discord: boolean;
  discord_app_id: string;
  /** Empty means the default folder. */
  downloads_dir: string;
}

export interface SavedView {
  id: string;
  name: string;
  scope: string;
  library: string;
  mediaType: number;
  sort: string;
  desc: boolean;
  unwatched: boolean;
  toggles: string[];
  picked: { filter: string; title: string; value: string; label: string }[];
}
export interface HomeLayout { order: string[]; hidden: string[]; shown: string[]; compact_continue: boolean; }
export interface UiSettings {
  saved_views: SavedView[];
  home_layouts: Record<string, HomeLayout>;
  accent: string;
  poster_size: number;
  hide_spoilers: boolean;
}

export interface Settings {
  player: PlayerSettings;
  ui: UiSettings;
}

export interface PinView {
  id: number;
  code: string;
  url: string;
  attempt: number;
}

export interface PlaylistView {
  playlist: Metadata;
  items: Metadata[];
  /** Entries the server reports; larger than `items.length` only if loading was cut short. */
  total: number;
}

export interface HistoryPage {
  items: Metadata[];
  hasMore: boolean;
  accounts: Record<string, string>;
}

export interface Tasks {
  activities: Activity[];
  transcodes: TranscodeSession[];
}

export interface Overview {
  version?: string;
  platform?: string;
  owner?: string;
  plexPass: boolean;
  machineIdentifier?: string;
  transcoder: boolean;
  connectedVia: string;
  hostCpu: number;
  plexCpu: number;
  hostMemory: number;
  plexMemory: number;
  release?: Release;
  canInstall: boolean;
}

export interface Pref {
  id: string;
  label: string;
  summary?: string;
  kind: string;
  value: string;
  options: [string, string][];
  advanced: boolean;
  /** Plex's group id; empty for library settings. */
  group: string;
}

export interface PrefGroup {
  id: string;
  label: string;
  prefs: Pref[];
}

/** A butler task with the setting that switches its scheduled runs. */
export interface ScheduledTask extends ButlerTask {
  pref?: Pref;
}

export interface Maintenance {
  /** Butler settings that aren't one task's switch, like the maintenance window. */
  schedule: Pref[];
  tasks: ScheduledTask[];
}

export interface Friend {
  id?: number;
  title?: string;
  username?: string;
  email?: string;
  thumb?: string;
  restricted: boolean;
  home: boolean;
  status?: string;
}

export interface UsersDevices {
  accounts: Account[];
  devices: Device[];
  clients: ServerDevice[];
  friends: Friend[];
}

export interface PartChange {
  partId: number;
  audio?: number;
  subtitle?: number;
}

export interface TrackView {
  total: number;
  seasons: { index: number; title: string; count: number }[];
  scope: { ratingKey: string; label: string }[];
  reference?: string;
  audio: { id: number; label: string; external: boolean }[];
  subtitles: { id: number; label: string; external: boolean }[];
  audioName?: string;
  subtitleName?: string;
  nothingChosen: boolean;
  plan: {
    changes: PartChange[];
    episodes: number;
    noAudio: string[];
    noSubtitle: string[];
  };
}

export interface Lyrics {
  synced: boolean;
  lines: { timeMs?: number; text: string }[];
}

export interface FilterView {
  /** Query key, e.g. "genre", "year", "unwatched", "hdr". */
  filter: string;
  title: string;
  /** "string", "integer", "tag" or "boolean". Boolean filters take "1". */
  kind: string;
}

export interface FolderView {
  title?: string;
  /** Sub-folders; pass `key` back to sectionFolders. */
  directories: Directory[];
  items: Metadata[];
}

/** A poster/background choice: `key` is what setArtwork wants; `thumb` previews it. */
export interface Artwork {
  key?: string;
  ratingKey?: string;
  thumb?: string;
  provider?: string;
  selected?: boolean | number;
}

export interface MatchResult {
  guid?: string;
  name?: string;
  score?: number;
  year?: number;
  thumb?: string;
  summary?: string;
  matched: boolean;
}

export type DownloadState = "queued" | "downloading" | "paused" | "done" | "failed";

export interface Download {
  account: string;
  /** "<server id>:<ratingKey>" */
  id: string;
  serverId: string;
  serverName: string;
  item: Metadata;
  file?: string;
  /** Local artwork path; show it with `downloadImg(id)` from lib/images. */
  poster?: string;
  size: number;
  received: number;
  state: DownloadState;
  error?: string;
  addedAt: number;
}

export interface LiveProgram {
  title: string;
  subtitle?: string;
  guid?: string;
  summary?: string;
  thumb?: string;
  /** Unix seconds. */
  beginsAt: number;
  endsAt: number;
}

export interface LiveChannel {
  dvr: string;
  id: string;
  number?: string;
  title: string;
  thumb?: string;
  /** From now on, in order; the first may be on air. */
  programs: LiveProgram[];
}

export interface Recordings {
  upcoming: { id: string; title: string; subtitle?: string; beginsAt?: number; channel?: string; status?: string }[];
  rules: { id: string; title: string; kind?: string }[];
}

export interface LibraryChoices {
  /** [identifier, title] */
  agents: [string, string][];
  scanners: string[];
}

export interface LibraryForm {
  name: string;
  /** "movie", "show", "artist" or "photo". */
  kind: string;
  agent: string;
  scanner: string;
  language: string;
  locations: string[];
}

export interface ServerStats {
  /** Bytes per time bucket, oldest first; `at` in unix seconds. */
  bandwidth: { at: number; lan: number; wan: number }[];
  accounts: { name: string; bytes: number }[];
  devices: { name: string; bytes: number }[];
  plays: number;
  playSeconds: number;
}
