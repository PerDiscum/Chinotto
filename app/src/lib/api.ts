import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { AddProgress, AddResult, DeviceInfo, IpodView, TrackChanges, WriteResult } from "./types";

export const listDevices = () => invoke<DeviceInfo[]>("list_devices");
export const addFolder = (path: string) => invoke<DeviceInfo>("add_folder", { path });
export const openIpod = (path: string) => invoke<IpodView>("open_ipod", { path });
export const closeIpod = () => invoke<void>("close_ipod");
export const eject = (path: string) => invoke<void>("eject", { path });
export const startupFolder = () => invoke<string | null>("startup_folder");

export const onDevicesChanged = (f: (d: DeviceInfo[]) => void): Promise<UnlistenFn> =>
  listen<DeviceInfo[]>("devices-changed", (e) => f(e.payload));

export async function pickFolder(): Promise<string | null> {
  const r = await open({ directory: true, multiple: false, title: "Choose the top folder of an iPod" });
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
