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
  playlists: Playlist[];
  tracks: Track[];
}

/** What the main area shows. */
export type Source =
  | { kind: "none" }
  | { kind: "device" }
  | { kind: "playlist"; id: string }
  /** A library's songs ("all") or one of its playlists. */
  | { kind: "library"; lib: string; id: string };

export interface LibrarySummary {
  id: string;
  name: string;
  folders: { path: string; available: boolean }[];
  songs: number;
  lastScan?: number | null;
  scanning: boolean;
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
  | "order" | "title" | "time" | "artist" | "album" | "genre" | "year" | "rating" | "plays" | "added" | "kind" | "format" | "bitrate";

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

/** Fields the Get Info window can change; only the keys present are sent. */
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
