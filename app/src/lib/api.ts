import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { AddProgress, AddResult, DeviceInfo, IpodView, LibrarySummary, LibraryView, ScanProgress, Track, TrackChanges, WriteResult } from "./types";

export const listDevices = () => invoke<DeviceInfo[]>("list_devices");
export const addFolder = (path: string) => invoke<DeviceInfo>("add_folder", { path });
export const openIpod = (path: string) => invoke<IpodView>("open_ipod", { path });
export const closeIpod = () => invoke<void>("close_ipod");
export const eject = (path: string) => invoke<void>("eject", { path });
export const startupFolder = () => invoke<string | null>("startup_folder");

export const onDevicesChanged = (f: (d: DeviceInfo[]) => void): Promise<UnlistenFn> =>
  listen<DeviceInfo[]>("devices-changed", (e) => f(e.payload));

export async function pickFolder(): Promise<string | null> {
  const r = await open({ directory: true, multiple: false, title: "Choose the drive" });
  return typeof r === "string" ? r : null;
}

/** URL of a cover thumbnail at least `px` wide (served by the `art:` scheme). */
export const artUrl = (generation: number, imageId: number, px: number) =>
  convertFileSrc(`${generation}-${imageId}-${px}`, "art");

// ---- Changing the iPod (each returns the re-read iPod) ----

export const setAllowChanges = (path: string, allow: boolean) =>
  invoke<IpodView>("set_allow_changes", { path, allow });
export const editTracks = (path: string, ids: string[], changes: TrackChanges) =>
  invoke<WriteResult>("edit_tracks", { path, ids, changes });
export const deleteTracks = (path: string, ids: string[]) => invoke<WriteResult>("delete_tracks", { path, ids });
export const addFiles = (path: string, files: string[], playlist: string | null) =>
  invoke<WriteResult<AddResult>>("add_files", { path, files, playlist });
export const createPlaylist = (path: string, name: string, ids: string[]) =>
  invoke<WriteResult<{ id: string }>>("create_playlist", { path, name, ids });
export const renamePlaylist = (path: string, id: string, name: string) =>
  invoke<WriteResult>("rename_playlist", { path, id, name });
export const deletePlaylist = (path: string, id: string) => invoke<WriteResult>("delete_playlist", { path, id });
export const setPlaylistTracks = (path: string, id: string, ids: string[]) =>
  invoke<WriteResult>("set_playlist_tracks", { path, id, ids });

export const onAddProgress = (f: (p: AddProgress) => void): Promise<UnlistenFn> =>
  listen<AddProgress>("add-progress", (e) => f(e.payload));

const AUDIO = ["mp3", "m4a", "aac", "wav", "aif", "aiff"];
export async function pickMusicFiles(): Promise<string[]> {
  const r = await open({
    multiple: true,
    directory: false,
    title: "Add to iPod",
    filters: [{ name: "Music the iPod plays", extensions: AUDIO }],
  });
  return Array.isArray(r) ? r : typeof r === "string" ? [r] : [];
}

export async function pickMusicFolder(): Promise<string | null> {
  const r = await open({ directory: true, multiple: false, title: "Add a folder to iPod" });
  return typeof r === "string" ? r : null;
}

// ---- Playback ----

export interface PlayerStatus {
  state: "playing" | "paused" | "stopped";
  positionMs: number;
  durationMs: number | null;
}
export const playTrack = (dbid: string, startMs = 0) => invoke<void>("play_track", { dbid, startMs });
export const playerPlay = () => invoke<void>("player_play");
export const playerPause = () => invoke<void>("player_pause");
export const playerStop = () => invoke<void>("player_stop");
export const playerSeek = (ms: number) => invoke<void>("player_seek", { ms: Math.max(0, Math.round(ms)) });
export const playerVolume = (volume: number) => invoke<void>("player_volume", { volume });
export const onPlayerStatus = (f: (s: PlayerStatus) => void) => listen<PlayerStatus>("player-status", (e) => f(e.payload));
export const onPlayerEnded = (f: () => void) => listen("player-ended", () => f());
export const onPlayerError = (f: (msg: string) => void) => listen<string>("player-error", (e) => f(e.payload));

// ---- Artwork ----
export const setArtwork = (path: string, ids: string[], image: string) => invoke<WriteResult>("set_artwork", { path, ids, image });
export const removeArtwork = (path: string, ids: string[]) => invoke<WriteResult>("remove_artwork", { path, ids });
/** Data URL of a small preview, with "#<width>x<height>" of the original appended. */
export const imagePreview = (image: string) => invoke<string>("image_preview", { image });
export async function pickImage(): Promise<string | null> {
  const r = await open({
    multiple: false,
    directory: false,
    title: "Choose a cover",
    filters: [{ name: "Images", extensions: ["jpg", "jpeg", "png", "gif", "bmp", "webp", "tif", "tiff"] }],
  });
  return typeof r === "string" ? r : null;
}

/** Save a cover (ArtworkDB image id) as a PNG; asks where. Returns false if cancelled. */
export async function saveCover(imageId: number, name: string): Promise<boolean> {
  const dest = await save({
    title: "Save Cover",
    defaultPath: `${name.replace(/[/\\:*?"<>|]/g, "-").trim() || "Cover"}.png`,
    filters: [{ name: "PNG image", extensions: ["png"] }],
  });
  if (!dest) return false;
  await invoke<void>("save_cover", { image: imageId, dest });
  return true;
}
/** The clipboard's image, kept in a temporary file; returns its path. */
export const pasteImage = () => invoke<string>("paste_image");

// ---- Device pictures ----
export const deviceIcons = () => invoke<Record<string, string>>("device_icons");
export const setDeviceIcon = (identity: string, icon: string | null) => invoke<void>("set_device_icon", { identity, icon });

// ---- Libraries ----
export const listLibraries = () => invoke<LibrarySummary[]>("list_libraries");
export const createLibrary = (name: string, folders: string[]) => invoke<string>("create_library", { name, folders });
export const updateLibrary = (id: string, name: string | null, folders: string[] | null) =>
  invoke<void>("update_library", { id, name, folders });
export const deleteLibrary = (id: string) => invoke<void>("delete_library", { id });
export const scanLibrary = (id: string) => invoke<void>("scan_library", { id });
export const removeUnavailable = (id: string) => invoke<number>("remove_unavailable", { id });
export const libraryView = (id: string) => invoke<LibraryView>("library_view", { id });
export const onLibrariesChanged = (f: (l: LibrarySummary[]) => void) => listen<LibrarySummary[]>("libraries-changed", (e) => f(e.payload));
export const onLibraryUpdated = (f: (id: string) => void) => listen<string>("library-updated", (e) => f(e.payload));
export const onLibraryProgress = (f: (p: ScanProgress) => void) => listen<ScanProgress>("library-progress", (e) => f(e.payload));
/** Cover of a library song, about `px` wide. */
export const libArtUrl = (lib: string, id: number, px: number, version = 0) =>
  convertFileSrc(`${lib}-${id}-${px}-${version}`, "libart");
export async function pickMusicFolders(): Promise<string[]> {
  const r = await open({ directory: true, multiple: true, title: "Choose music folders" });
  return Array.isArray(r) ? r : typeof r === "string" ? [r] : [];
}
export const libCreatePlaylist = (lib: string, name: string, ids: number[]) => invoke<string>("lib_create_playlist", { lib, name, ids });
export const libRenamePlaylist = (lib: string, id: string, name: string) => invoke<void>("lib_rename_playlist", { lib, id, name });
export const libDeletePlaylist = (lib: string, id: string) => invoke<void>("lib_delete_playlist", { lib, id });
export const libSetPlaylistTracks = (lib: string, id: string, ids: number[]) => invoke<void>("lib_set_playlist_tracks", { lib, id, ids });
export const libRemoveTracks = (lib: string, ids: number[]) => invoke<number>("lib_remove_tracks", { lib, ids });
/** Returns the files that couldn't be changed. `cover`: image path, "" to remove, null to leave. */
export const libEditTracks = (lib: string, ids: number[], changes: TrackChanges, cover: string | null) =>
  invoke<string[]>("lib_edit_tracks", { lib, ids, changes, cover });

// ---- Scratch list (songs opened from outside the libraries) ----
export const scratchView = () => invoke<{ tracks: Track[]; play: number | null }>("scratch_view");
export const scratchAdd = (paths: string[]) => invoke<void>("scratch_add", { paths });
export const scratchRemove = (ids: number[]) => invoke<void>("scratch_remove", { ids });
export const onScratchChanged = (f: (play: number | null) => void) =>
  listen<{ play: number | null }>("scratch-changed", (e) => f(e.payload.play));
export async function pickSongs(): Promise<string[]> {
  const r = await open({
    multiple: true,
    directory: false,
    title: "Open",
    filters: [{ name: "Music", extensions: ["mp3", "m4a", "m4b", "aac", "flac", "ogg", "oga", "opus", "wav", "aif", "aiff"] }],
  });
  return Array.isArray(r) ? r : typeof r === "string" ? [r] : [];
}

// ---- Saving songs to the computer ----
export interface SongRef {
  dbid: string;
  title?: string;
  artist?: string;
  album?: string;
  trackNr?: number;
}
export const exportName = (song: SongRef) => invoke<string>("export_name", { song });
export const exportSongs = (songs: SongRef[], dest: string) => invoke<string>("export_songs", { songs, dest });
export async function pickSaveFile(defaultPath: string, title: string, ext: string): Promise<string | null> {
  return (await save({ title, defaultPath, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] })) ?? null;
}
