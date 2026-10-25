import { invoke, convertFileSrc } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { Conversion, AddProgress, AddResult, DeviceInfo, IpodView, LibrarySummary, LibraryView, ScanProgress, Track, TrackChanges, WriteResult, SetupInfo, SetupResult, DriveInfo, MscConfig, PlayerState, CopyResult, MtpInfo, OpenMgView, PodcastsView } from "./types";

export const listDevices = () => invoke<DeviceInfo[]>("list_devices");
export const addFolder = (path: string) => invoke<DeviceInfo>("add_folder", { path });
export const openIpod = (path: string) => invoke<IpodView>("open_ipod", { path });
export const closeIpod = () => invoke<void>("close_ipod");
export const eject = (path: string) => invoke<void>("eject", { path });
export const startupFolder = () => invoke<string | null>("startup_folder");
export const openMgView = (path: string) => invoke<OpenMgView>("openmg_view", { path });
export const openMgReorder = (path: string) => invoke<number>("openmg_reorder", { path });

export const onDevicesChanged = (f: (d: DeviceInfo[]) => void): Promise<UnlistenFn> =>
  listen<DeviceInfo[]>("devices-changed", (e) => f(e.payload));

export async function pickFolder(title = "Choose the drive"): Promise<string | null> {
  const r = await open({ directory: true, multiple: false, title });
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
    title: "Add Songs",
    filters: [{ name: "Music", extensions: AUDIO }],
  });
  return Array.isArray(r) ? r : typeof r === "string" ? [r] : [];
}

export async function pickMusicFolder(): Promise<string | null> {
  const r = await open({ directory: true, multiple: false, title: "Add a Folder of Songs" });
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
/** Levels (0–1) of what's playing: 8 bands of the left channel, then 8 of the right. */
export const onPlayerSpectrum = (f: (levels: number[]) => void) => listen<number[]>("player-spectrum", (e) => f(e.payload));
export const onPlayerEnded = (f: () => void) => listen("player-ended", () => f());
export const onPlayerError = (f: (msg: string) => void) => listen<string>("player-error", (e) => f(e.payload));

// ---- Podcasts ----
export const podcastsGet = () => invoke<PodcastsView>("podcasts_get");
export const podcastsSetFolder = (folder: string) => invoke<PodcastsView>("podcasts_set_folder", { folder });
export const podcastAdd = (url: string) => invoke<PodcastsView>("podcast_add", { url });
export const podcastsRefresh = (id: string | null = null) => invoke<PodcastsView>("podcasts_refresh", { id });
export const podcastRemove = (id: string, deleteFiles: boolean) => invoke<PodcastsView>("podcast_remove", { id, deleteFiles });
export const podcastMark = (id: string, keys: string[], marks: { new?: boolean; played?: boolean }) =>
  invoke<PodcastsView>("podcast_mark", { id, keys, new: marks.new ?? null, played: marks.played ?? null });
export const podcastDeleteFile = (id: string, key: string) => invoke<PodcastsView>("podcast_delete_file", { id, key });
export const podcastCancel = (id: string, key: string) => invoke<void>("podcast_cancel", { id, key });
export const podcastDownload = (id: string, key: string) => invoke<PodcastsView>("podcast_download", { id, key });
export interface OnIpod {
  show: string;
  key: string;
  dbid: string;
  played: boolean;
  bookmarkMs: number;
}
export const podcastIpodStatus = (path: string) => invoke<OnIpod[]>("podcast_ipod_status", { path });
export const podcastSend = (path: string, id: string, keys: string[]) =>
  invoke<WriteResult<{ added: number; skipped: string[] }>>("podcast_send", { path, id, keys });
export const podcastRemovePlayed = (path: string, id: string, fromComputer: boolean) =>
  invoke<WriteResult<number>>("podcast_remove_played", { path, id, fromComputer });
/** Show a song's or episode's file in the Finder / file manager. */
export const revealSong = (dbid: string) => invoke<void>("reveal_song", { dbid });
/** A podcast's saved cover (the version busts the cache after a refresh). */
export const podcastCoverUrl = (id: string, px: number, version: number) => convertFileSrc(`pod-${id}-${px}-${version}`, "libart");
export const onPodcastProgress = (f: (p: { id: string; key: string; done: number; total: number }) => void) =>
  listen<{ id: string; key: string; done: number; total: number }>("podcast-progress", (e) => f(e.payload));

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

/** Save a cover (ArtworkDB image id) as a PNG; asks where. Returns false if canceled. */
/** Save a cover: an iPod song's (by artwork id) or a library/Scratch song's (by dbid). */
export async function saveCover(from: number | string, name: string): Promise<boolean> {
  const dest = await save({
    title: "Save Cover",
    defaultPath: `${name.replace(/[/\\:*?"<>|]/g, "-").trim() || "Cover"}.png`,
    filters: [{ name: "PNG image", extensions: ["png"] }],
  });
  if (!dest) return false;
  if (typeof from === "string") await invoke<void>("save_file_cover", { dbid: from, dest });
  else await invoke<void>("save_cover", { image: from, dest });
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
export const scanLibrary = (id: string, full = false) => invoke<void>("scan_library", { id, full });
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
export const setConversion = (path: string, conversion: Conversion) => invoke<IpodView>("set_conversion", { path, conversion });

// ---- Setting up an iPod ----
export const setupInfo = (path: string) => invoke<SetupInfo>("setup_info", { path });
export const setupIpod = (a: { path: string; model: string; name: string; firewireId: string | null; addExisting: boolean; rebuild: boolean }) =>
  invoke<SetupResult>("setup_ipod", a);

// ---- Players set up as drives (MSC) ----
export const mscInspect = (path: string) => invoke<DriveInfo>("msc_inspect", { path });
export const mscSave = (path: string, config: MscConfig) => invoke<DeviceInfo>("msc_save", { path, config });
export const mscState = (path: string) => invoke<PlayerState>("msc_state", { path });
export const mscAllow = (path: string, allow: boolean) => invoke<PlayerState>("msc_allow", { path, allow });
export const mscSetConversion = (path: string, conversion: Conversion) => invoke<PlayerState>("msc_set_conversion", { path, conversion });
export const mscAdd = (path: string, dbids: string[], files: string[]) => invoke<CopyResult>("msc_add", { path, dbids, files });
export const mscDelete = (path: string, ids: number[]) => invoke<string[]>("msc_delete", { path, ids });

// ---- MTP devices ----
export const mtpOpen = (key: string) => invoke<MtpInfo>("mtp_open", { key });
export const mtpClose = (key: string) => invoke<void>("mtp_close", { key });
export const mtpInfo = (key: string) => invoke<MtpInfo | null>("mtp_info", { key });

// ---- Audio CDs ----
export const cdList = () => invoke<import("./types").Disc[]>("cd_list");
export const cdRip = (req: import("./types").RipRequest) => invoke<import("./types").RipResult>("cd_rip", { req });
export const cdEject = (id: string) => invoke<void>("cd_eject", { id });
export const cdMusicInfo = () =>
  invoke<{ artist: string; album: string; year: number; genre: string; titles: { number: number; title: string }[] }>("cd_music_info");

// ---- Burning disc images ----
export const burnStatus = () => invoke<import("./types").BurnStatus>("burn_status");
export const burnInspect = (path: string) => invoke<import("./types").DiscImage>("burn_inspect", { path });
export const burnImage = (path: string, speed: number, verify: boolean) => invoke<string[]>("burn_image", { path, speed, verify });
export async function pickDiscImage(): Promise<string | null> {
  const r = await open({ multiple: false, directory: false, title: "Choose a disc image", filters: [{ name: "Disc images", extensions: ["iso", "cue", "bin", "img", "cdr", "toast"] }] });
  return typeof r === "string" ? r : null;
}
export const openLicenses = () => invoke<void>("open_licenses");
