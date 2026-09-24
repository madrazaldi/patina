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
  tag?: string;
  descKey?: string;
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
}

export interface Snapshot {
  stage: Stage;
  session?: SessionView;
  sections: Directory[];
  nowPlaying?: NowPlaying;
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
}

export interface UiSettings {
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
}

export interface UsersDevices {
  accounts: Account[];
  devices: Device[];
  clients: ServerDevice[];
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
