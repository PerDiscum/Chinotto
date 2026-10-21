// Shapes returned by the Rust side (see src-tauri/src/commands.rs).

export interface DeviceInfo {
  path: string;
  volume: string;
  model?: string | null;
  modelNumber?: string | null;
  /** Picture key for the model, e.g. "ipod-nano-3g"; "generic" when unknown. */
  icon: string;
  /** FireWire GUID or serial, when the device reports one. */
  identity?: string | null;
  capacity: number;
  free: number;
  manual: boolean;
  /** An iPod without a database: it needs setting up first. */
  needsSetup: boolean;
  /** "ipod", or "msc" for a player set up as a drive. */
  kind: "ipod" | "msc";
  /** MSC player: the library listing its songs. */
  library: string | null;
}

/** An MSC player's settings, kept on the player. */
export interface MscConfig {
  id: string;
  name: string;
  model: string;
  icon: string;
  musicFolder: string;
  playlistFolder: string;
  playlistFormat: "m3u8" | "m3u" | "pls";
}

export interface MscProfile {
  key: string;
  name: string;
  icon: string;
  musicFolder: string;
  playlistFolder: string;
  playlistFormat: "m3u8" | "m3u" | "pls";
}

export interface DriveInfo {
  path: string;
  volume: string;
  isDisk: boolean;
  isIpod: boolean;
  config: MscConfig | null;
  suggested: string;
  folders: string[];
  profiles: MscProfile[];
}

export interface SetupModel {
  number: string;
  name: string;
  icon: string;
  /** Its database needs the FireWire ID (iPod classic, nano 3G). */
  needsId: boolean;
}

export interface SetupInfo {
  path: string;
  volume: string;
  capacity: number;
  isDisk: boolean;
  hasDatabase: boolean;
  model: string | null;
  firewireId: string | null;
  musicFiles: number;
  models: SetupModel[];
}

export interface SetupResult {
  added: number;
  problems: string[];
  setAside: string | null;
}

export interface Playlist {
  id: string;
  name: string;
  master: boolean;
  smart: boolean;
  trackIds: number[];
}

export interface Track {
  id: number;
  /** Permanent id (64-bit, as text); survives saves, unlike `id`. */
  dbid: string;
  title?: string;
  artist?: string;
  albumArtist?: string;
  album?: string;
  genre?: string;
  composer?: string;
  comment?: string;
  grouping?: string;
  sortTitle?: string;
  sortArtist?: string;
  sortAlbum?: string;
  sortAlbumArtist?: string;
  sortComposer?: string;
  location?: string;
  kind: string;
  year: number;
  trackNr: number;
  trackCount: number;
  discNr: number;
  discCount: number;
  lengthMs: number;
  size: number;
  bitrate: number;
  sampleRate: number;
  rating: number;
  playCount: number;
  compilation: boolean;
  mediaType: number;
  added?: number;
  played?: number;
  modified?: number;
  art: number;
  /** Library songs: a cover can be shown (embedded or beside the file). */
  hasCover?: boolean;
  /** Library songs: its folder isn't there (drive unplugged). */
  unavailable?: boolean;
}

export interface IpodView {
  path: string;
  name: string;
  device: DeviceInfo;
  serial?: string | null;
  dbVersion: number;
  generation: number;
  hasArtworkDb: boolean;
  /** FireWire GUID or serial; null if the iPod doesn't report one. */
  identity?: string | null;
  /** The user allowed Chinotto to change this iPod. */
  writable: boolean;
  /** How songs are converted when copied to it. */
  conversion: Conversion;
  playlists: Playlist[];
  tracks: Track[];
}

/** What the main area shows. */
export type Source =
  | { kind: "none" }
  | { kind: "device" }
  | { kind: "playlist"; id: string }
  /** A library's songs ("all") or one of its playlists. */
  | { kind: "library"; lib: string; id: string }
  /** Songs opened from outside the libraries (never added to one). */
  | { kind: "scratch" }
  /** The page of a player that shows up as a drive (MSC). */
  | { kind: "player"; path: string };

export interface LibrarySummary {
  id: string;
  name: string;
  folders: { path: string; available: boolean }[];
  songs: number;
  lastScan?: number | null;
  scanning: boolean;
  /** An MSC player's songs (shown under Devices). */
  device?: string | null;
}

export interface LibraryView {
  id: string;
  name: string;
  playlists: Playlist[];
  tracks: Track[];
}

export interface ScanProgress {
  id: string;
  done: number;
  total: number;
  current: string;
}

export type SortKey =
  | "order" | "title" | "time" | "artist" | "album" | "genre" | "year" | "rating" | "plays" | "added" | "kind" | "format" | "bitrate" | "disc" | "track" | "path";

/** Columns the column browser can show, broadest first. */
export type BrowseField = "genre" | "format" | "year" | "composer" | "artist" | "album";

/** What every command that changes the iPod returns. */
export interface WriteResult<T = unknown> {
  view: IpodView;
  extra: T;
}

export interface AddResult {
  added: string[];
  skipped: string[];
}

export interface AddProgress {
  done: number;
  total: number;
  current: string;
}

/** Fields the Song Details window can change; only the keys present are sent. */
export interface TrackChanges {
  title?: string | null;
  artist?: string | null;
  albumArtist?: string | null;
  album?: string | null;
  genre?: string | null;
  composer?: string | null;
  comment?: string | null;
  grouping?: string | null;
  sortTitle?: string | null;
  sortArtist?: string | null;
  sortAlbum?: string | null;
  sortAlbumArtist?: string | null;
  sortComposer?: string | null;
  year?: number | null;
  trackNr?: number | null;
  trackCount?: number | null;
  discNr?: number | null;
  discCount?: number | null;
  bpm?: number | null;
  rating?: number;
  compilation?: boolean;
}

export interface Conversion {
  /** "aac256", "aac128", "mp3_320" or "mp3_192". */
  format: string;
  /** Convert Apple Lossless too (it plays on iPods but is large). */
  convertAlac: boolean;
}

/** A player's page settings (see msc.rs). */
export interface PlayerState {
  writable: boolean;
  conversion: Conversion;
  plays: string[];
  aac: boolean;
}

export interface CopyResult {
  added: number;
  already: number;
  skipped: string[];
}
