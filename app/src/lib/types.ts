// Shapes returned by the Rust side (see src-tauri/src/commands.rs).

export interface DeviceInfo {
  path: string;
  volume: string;
  model?: string | null;
  modelNumber?: string | null;
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
}

export interface IpodView {
  path: string;
  name: string;
  device: DeviceInfo;
  serial?: string | null;
  dbVersion: number;
  generation: number;
  hasArtworkDb: boolean;
  playlists: Playlist[];
  tracks: Track[];
}

/** What the main area shows. */
export type Source =
  | { kind: "none" }
  | { kind: "device" }
  | { kind: "playlist"; id: string };

export type SortKey =
  | "title" | "time" | "artist" | "album" | "genre" | "year" | "rating" | "plays" | "added" | "kind";
