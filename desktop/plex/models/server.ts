// Server-side models: sessions, settings, activities, accounts, statistics, ... (port of crates/plex/src/models/server.rs).

import { boolean, obj, optF64, optI64, optString, value } from "../de";

export interface SessionUser {
  id: string | null;
  title: string | null;
  thumb: string | null;
}

export function normalizeSessionUser(v: unknown): SessionUser {
  const o = obj(v, "SessionUser");
  return {
    id: optString(o.id),
    title: optString(o.title),
    thumb: optString(o.thumb),
  };
}

/** `SessionUser::default()`, with fields overridden by `over`. */
export function defaultSessionUser(over: Partial<SessionUser> = {}): SessionUser {
  return { ...normalizeSessionUser({}), ...over };
}

/**
 * The client playing a session.
 */
export interface Player {
  address: string | null;
  device: string | null;
  machineIdentifier: string | null;
  model: string | null;
  platform: string | null;
  platformVersion: string | null;
  product: string | null;
  profile: string | null;
  remotePublicAddress: string | null;
  state: string | null;
  title: string | null;
  vendor: string | null;
  version: string | null;
  local: boolean;
  relayed: boolean;
  secure: boolean;
}

export function normalizePlayer(v: unknown): Player {
  const o = obj(v, "Player");
  return {
    address: optString(o.address),
    device: optString(o.device),
    machineIdentifier: optString(o.machineIdentifier),
    model: optString(o.model),
    platform: optString(o.platform),
    platformVersion: optString(o.platformVersion),
    product: optString(o.product),
    profile: optString(o.profile),
    remotePublicAddress: optString(o.remotePublicAddress),
    state: optString(o.state),
    title: optString(o.title),
    vendor: optString(o.vendor),
    version: optString(o.version),
    local: boolean(o.local),
    relayed: boolean(o.relayed),
    secure: boolean(o.secure),
  };
}

/** `Player::default()`, with fields overridden by `over`. */
export function defaultPlayer(over: Partial<Player> = {}): Player {
  return { ...normalizePlayer({}), ...over };
}

export interface SessionInfo {
  id: string | null;
  bandwidth: number | null;
  location: string | null;
}

export function normalizeSessionInfo(v: unknown): SessionInfo {
  const o = obj(v, "SessionInfo");
  return {
    id: optString(o.id),
    bandwidth: optI64(o.bandwidth),
    location: optString(o.location),
  };
}

/** `SessionInfo::default()`, with fields overridden by `over`. */
export function defaultSessionInfo(over: Partial<SessionInfo> = {}): SessionInfo {
  return { ...normalizeSessionInfo({}), ...over };
}

export interface TranscodeSession {
  key: string | null;
  throttled: boolean;
  complete: boolean;
  progress: number | null;
  speed: number | null;
  duration: number | null;
  remaining: number | null;
  context: string | null;
  sourceVideoCodec: string | null;
  sourceAudioCodec: string | null;
  videoDecision: string | null;
  audioDecision: string | null;
  subtitleDecision: string | null;
  protocol: string | null;
  container: string | null;
  videoCodec: string | null;
  audioCodec: string | null;
  audioChannels: number | null;
  width: number | null;
  height: number | null;
  transcodeHwRequested: boolean;
  transcodeHwFullPipeline: boolean;
  maxOffsetAvailable: number | null;
  minOffsetAvailable: number | null;
}

export function normalizeTranscodeSession(v: unknown): TranscodeSession {
  const o = obj(v, "TranscodeSession");
  return {
    key: optString(o.key),
    throttled: boolean(o.throttled),
    complete: boolean(o.complete),
    progress: optF64(o.progress),
    speed: optF64(o.speed),
    duration: optI64(o.duration),
    remaining: optI64(o.remaining),
    context: optString(o.context),
    sourceVideoCodec: optString(o.sourceVideoCodec),
    sourceAudioCodec: optString(o.sourceAudioCodec),
    videoDecision: optString(o.videoDecision),
    audioDecision: optString(o.audioDecision),
    subtitleDecision: optString(o.subtitleDecision),
    protocol: optString(o.protocol),
    container: optString(o.container),
    videoCodec: optString(o.videoCodec),
    audioCodec: optString(o.audioCodec),
    audioChannels: optI64(o.audioChannels),
    width: optI64(o.width),
    height: optI64(o.height),
    transcodeHwRequested: boolean(o.transcodeHwRequested),
    transcodeHwFullPipeline: boolean(o.transcodeHwFullPipeline),
    maxOffsetAvailable: optF64(o.maxOffsetAvailable),
    minOffsetAvailable: optF64(o.minOffsetAvailable),
  };
}

/** `TranscodeSession::default()`, with fields overridden by `over`. */
export function defaultTranscodeSession(over: Partial<TranscodeSession> = {}): TranscodeSession {
  return { ...normalizeTranscodeSession({}), ...over };
}

/**
 * A server or account preference.
 */
export interface Setting {
  id: string | null;
  label: string | null;
  summary: string | null;
  type: string | null;
  default: unknown;
  value: unknown;
  hidden: boolean;
  advanced: boolean;
  group: string | null;
  /** `value:Label|value:Label` pairs for enum settings. */
  enumValues: string | null;
}

export function normalizeSetting(v: unknown): Setting {
  const o = obj(v, "Setting");
  return {
    id: optString(o.id),
    label: optString(o.label),
    summary: optString(o.summary),
    type: optString(o.type),
    default: value(o.default),
    value: value(o.value),
    hidden: boolean(o.hidden),
    advanced: boolean(o.advanced),
    group: optString(o.group),
    enumValues: optString(o.enumValues),
  };
}

/** `Setting::default()`, with fields overridden by `over`. */
export function defaultSetting(over: Partial<Setting> = {}): Setting {
  return { ...normalizeSetting({}), ...over };
}

/**
 * A background server activity (scan, refresh, optimise, ...).
 */
export interface Activity {
  uuid: string | null;
  type: string | null;
  cancellable: boolean;
  userID: number | null;
  title: string | null;
  subtitle: string | null;
  progress: number | null;
  Context: unknown;
}

export function normalizeActivity(v: unknown): Activity {
  const o = obj(v, "Activity");
  return {
    uuid: optString(o.uuid),
    type: optString(o.type),
    cancellable: boolean(o.cancellable),
    userID: optI64(o.userID),
    title: optString(o.title),
    subtitle: optString(o.subtitle),
    progress: optI64(o.progress),
    Context: value(o.Context),
  };
}

/** `Activity::default()`, with fields overridden by `over`. */
export function defaultActivity(over: Partial<Activity> = {}): Activity {
  return { ...normalizeActivity({}), ...over };
}

export interface Account {
  id: number | null;
  key: string | null;
  name: string | null;
  defaultAudioLanguage: string | null;
  autoSelectAudio: boolean;
  defaultSubtitleLanguage: string | null;
  subtitleMode: number | null;
  thumb: string | null;
}

export function normalizeAccount(v: unknown): Account {
  const o = obj(v, "Account");
  return {
    id: optI64(o.id),
    key: optString(o.key),
    name: optString(o.name),
    defaultAudioLanguage: optString(o.defaultAudioLanguage),
    autoSelectAudio: boolean(o.autoSelectAudio),
    defaultSubtitleLanguage: optString(o.defaultSubtitleLanguage),
    subtitleMode: optI64(o.subtitleMode),
    thumb: optString(o.thumb),
  };
}

/** `Account::default()`, with fields overridden by `over`. */
export function defaultAccount(over: Partial<Account> = {}): Account {
  return { ...normalizeAccount({}), ...over };
}

export interface Device {
  id: number | null;
  name: string | null;
  platform: string | null;
  clientIdentifier: string | null;
  createdAt: number | null;
}

export function normalizeDevice(v: unknown): Device {
  const o = obj(v, "Device");
  return {
    id: optI64(o.id),
    name: optString(o.name),
    platform: optString(o.platform),
    clientIdentifier: optString(o.clientIdentifier),
    createdAt: optI64(o.createdAt),
  };
}

/** `Device::default()`, with fields overridden by `over`. */
export function defaultDevice(over: Partial<Device> = {}): Device {
  return { ...normalizeDevice({}), ...over };
}

/**
 * Entries from `/clients` and `/servers`.
 */
export interface ServerDevice {
  name: string | null;
  host: string | null;
  address: string | null;
  port: number | null;
  machineIdentifier: string | null;
  version: string | null;
  protocol: string | null;
  product: string | null;
  deviceClass: string | null;
  protocolVersion: string | null;
  protocolCapabilities: string | null;
}

export function normalizeServerDevice(v: unknown): ServerDevice {
  const o = obj(v, "ServerDevice");
  return {
    name: optString(o.name),
    host: optString(o.host),
    address: optString(o.address),
    port: optI64(o.port),
    machineIdentifier: optString(o.machineIdentifier),
    version: optString(o.version),
    protocol: optString(o.protocol),
    product: optString(o.product),
    deviceClass: optString(o.deviceClass),
    protocolVersion: optString(o.protocolVersion),
    protocolCapabilities: optString(o.protocolCapabilities),
  };
}

/** `ServerDevice::default()`, with fields overridden by `over`. */
export function defaultServerDevice(over: Partial<ServerDevice> = {}): ServerDevice {
  return { ...normalizeServerDevice({}), ...over };
}

export interface StatisticsMedia {
  accountID: number | null;
  deviceID: number | null;
  timespan: number | null;
  at: number | null;
  metadataType: number | null;
  count: number | null;
  duration: number | null;
}

export function normalizeStatisticsMedia(v: unknown): StatisticsMedia {
  const o = obj(v, "StatisticsMedia");
  return {
    accountID: optI64(o.accountID),
    deviceID: optI64(o.deviceID),
    timespan: optI64(o.timespan),
    at: optI64(o.at),
    metadataType: optI64(o.metadataType),
    count: optI64(o.count),
    duration: optI64(o.duration),
  };
}

/** `StatisticsMedia::default()`, with fields overridden by `over`. */
export function defaultStatisticsMedia(over: Partial<StatisticsMedia> = {}): StatisticsMedia {
  return { ...normalizeStatisticsMedia({}), ...over };
}

export interface StatisticsResources {
  timespan: number | null;
  at: number | null;
  hostCpuUtilization: number | null;
  processCpuUtilization: number | null;
  hostMemoryUtilization: number | null;
  processMemoryUtilization: number | null;
}

export function normalizeStatisticsResources(v: unknown): StatisticsResources {
  const o = obj(v, "StatisticsResources");
  return {
    timespan: optI64(o.timespan),
    at: optI64(o.at),
    hostCpuUtilization: optF64(o.hostCpuUtilization),
    processCpuUtilization: optF64(o.processCpuUtilization),
    hostMemoryUtilization: optF64(o.hostMemoryUtilization),
    processMemoryUtilization: optF64(o.processMemoryUtilization),
  };
}

/** `StatisticsResources::default()`, with fields overridden by `over`. */
export function defaultStatisticsResources(over: Partial<StatisticsResources> = {}): StatisticsResources {
  return { ...normalizeStatisticsResources({}), ...over };
}

export interface StatisticsBandwidth {
  accountID: number | null;
  deviceID: number | null;
  timespan: number | null;
  at: number | null;
  lan: boolean;
  bytes: number | null;
}

export function normalizeStatisticsBandwidth(v: unknown): StatisticsBandwidth {
  const o = obj(v, "StatisticsBandwidth");
  return {
    accountID: optI64(o.accountID),
    deviceID: optI64(o.deviceID),
    timespan: optI64(o.timespan),
    at: optI64(o.at),
    lan: boolean(o.lan),
    bytes: optI64(o.bytes),
  };
}

/** `StatisticsBandwidth::default()`, with fields overridden by `over`. */
export function defaultStatisticsBandwidth(over: Partial<StatisticsBandwidth> = {}): StatisticsBandwidth {
  return { ...normalizeStatisticsBandwidth({}), ...over };
}

export interface Release {
  key: string | null;
  version: string | null;
  added: string | null;
  fixed: string | null;
  downloadURL: string | null;
  state: string | null;
}

export function normalizeRelease(v: unknown): Release {
  const o = obj(v, "Release");
  return {
    key: optString(o.key),
    version: optString(o.version),
    added: optString(o.added),
    fixed: optString(o.fixed),
    downloadURL: optString(o.downloadURL),
    state: optString(o.state),
  };
}

/** `Release::default()`, with fields overridden by `over`. */
export function defaultRelease(over: Partial<Release> = {}): Release {
  return { ...normalizeRelease({}), ...over };
}

export interface ButlerTask {
  name: string | null;
  title: string | null;
  description: string | null;
  interval: number | null;
  enabled: boolean;
  scheduleRandomized: boolean;
}

export function normalizeButlerTask(v: unknown): ButlerTask {
  const o = obj(v, "ButlerTask");
  return {
    name: optString(o.name),
    title: optString(o.title),
    description: optString(o.description),
    interval: optI64(o.interval),
    enabled: boolean(o.enabled),
    scheduleRandomized: boolean(o.scheduleRandomized),
  };
}

/** `ButlerTask::default()`, with fields overridden by `over`. */
export function defaultButlerTask(over: Partial<ButlerTask> = {}): ButlerTask {
  return { ...normalizeButlerTask({}), ...over };
}

/**
 * `/identity` response.
 */
export interface Identity {
  machineIdentifier: string | null;
  version: string | null;
  claimed: boolean;
}

export function normalizeIdentity(v: unknown): Identity {
  const o = obj(v, "Identity");
  return {
    machineIdentifier: optString(o.machineIdentifier),
    version: optString(o.version),
    claimed: boolean(o.claimed),
  };
}

/** `Identity::default()`, with fields overridden by `over`. */
export function defaultIdentity(over: Partial<Identity> = {}): Identity {
  return { ...normalizeIdentity({}), ...over };
}


export namespace Setting {
  export function valueString(s: Setting): string {
    const v = s.value;
    if (typeof v === "string") return v;
    if (v === null || v === undefined) return "";
    return JSON.stringify(v);
  }

  /** `value:Label|value:Label` pairs as `[value, label]`. */
  export function enumOptions(s: Setting): [string, string][] {
    return (s.enumValues ?? "")
      .split("|")
      .filter((p) => p !== "")
      .map((pair): [string, string] => {
        const i = pair.indexOf(":");
        return i >= 0 ? [pair.slice(0, i), pair.slice(i + 1)] : [pair, pair];
      });
  }
}
