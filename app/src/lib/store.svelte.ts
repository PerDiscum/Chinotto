// App-wide state (Svelte 5 runes).

import * as api from "./api";
import { TrackIndex } from "./library";
import { count } from "./format";
import { t, tn } from "./i18n";
import type { AddProgress, BrowseField, Conversion, DeviceInfo, IpodView, LibrarySummary, LibraryView, ScanProgress, SortKey, Source, Track, TrackChanges, WriteResult } from "./types";

export interface MenuItem {
  label: string;
  action?: () => void;
  disabled?: boolean;
  danger?: boolean;
  separator?: boolean;
  submenu?: MenuItem[];
}

export type Dialog =
  | { kind: "info" }
  | { kind: "delete-songs"; ids: string[] }
  | { kind: "delete-playlist"; id: string; name: string }
  | { kind: "report"; title: string; lines: string[] }
  | { kind: "library"; id: string | null }
  | { kind: "delete-library"; id: string; name: string }
  | { kind: "remove-from-library"; ids: number[] }
  /** Set up an iPod with no database, or (rebuild) replace a lost or broken one. */
  | { kind: "setup"; path: string; rebuild: boolean }
  /** Set up (or change the settings of) a player that shows up as a drive. */
  | { kind: "msc"; path: string }
  | { kind: "delete-player-songs"; path: string; ids: number[] };

function loadVolume(): number {
  try {
    const v = Number(localStorage.getItem("chinotto.volume"));
    return v > 0 && v <= 1 ? v : 0.8;
  } catch {
    return 0.8;
  }
}

export type Repeat = "off" | "all" | "one";
export type MusicView = "songs" | "albums" | "artists" | "genres" | "composers" | "compilations";
const VIEWS = ["songs", "albums", "artists", "genres", "composers", "compilations"] as const;

function loadPref<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  try {
    const v = localStorage.getItem(key) as T | null;
    return v && allowed.includes(v) ? v : fallback;
  } catch {
    return fallback;
  }
}
function savePref(key: string, v: string) {
  try {
    localStorage.setItem(key, v);
  } catch {
    /* not essential */
  }
}

function loadList(key: string, fallback: string[]): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(key) ?? "null");
    return Array.isArray(v) && v.every((x) => typeof x === "string") ? v : fallback;
  } catch {
    return fallback;
  }
}

/** A library song's number, from its dbid "lib:<library>:<song>". */
const libSongId = (dbid: string) => Number(dbid.split(":")[2]);

/** Fisher–Yates shuffle of a copy. */
function shuffled<T>(a: T[]): T[] {
  const r = a.slice();
  for (let i = r.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [r[i], r[j]] = [r[j], r[i]];
  }
  return r;
}

/** On macOS Ctrl-click is a right-click; Cmd toggles selection. Elsewhere Ctrl toggles. */
export const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);

class Store {
  devices = $state<DeviceInfo[]>([]);
  ipod = $state.raw<IpodView | null>(null);
  /** Index of the open iPod's songs. */
  ipodIndex = $state.raw<TrackIndex | null>(null);

  // ---- Libraries ----
  libraries = $state<LibrarySummary[]>([]);
  /** Libraries loaded so far (songs and playlists), with their indexes. */
  libViews = $state.raw<Record<string, { view: LibraryView; index: TrackIndex }>>({});
  scanProgress = $state<Record<string, ScanProgress>>({});
  /** Libraries collapsed in the sidebar. */
  collapsed = $state<string[]>(loadList("chinotto.collapsed", []));

  /** The shown library's songs, when a library is shown. */
  get libView(): { view: LibraryView; index: TrackIndex } | null {
    const s = this.source;
    return s.kind === "library" ? (this.libViews[s.lib] ?? null) : null;
  }

  /** Index of the songs being shown (a library's or the iPod's). */
  get index(): TrackIndex | null {
    if (this.source.kind === "scratch") return this.scratch?.index ?? null;
    return this.source.kind === "library" ? (this.libView?.index ?? null) : this.ipodIndex;
  }

  /** Songs opened from outside the libraries; null when there are none. */
  scratch = $state.raw<{ tracks: Track[]; index: TrackIndex } | null>(null);

  private async loadScratch(play: number | null) {
    try {
      const v = await api.scratchView();
      const p = play ?? v.play;
      this.scratch = v.tracks.length ? { tracks: v.tracks, index: new TrackIndex(v.tracks) } : null;
      if (!this.scratch) {
        if (this.source.kind === "scratch") this.select(this.ipod ? { kind: "device" } : { kind: "none" });
        return;
      }
      if (p) {
        // Just opened: show the Scratch list and play the song.
        if (this.source.kind !== "scratch") this.select({ kind: "scratch" });
        const tracks = this.scratch.tracks;
        const i = tracks.findIndex((t) => t.id === p);
        if (i >= 0) {
          this.selected = new Set([p]);
          await this.playList(tracks, i);
        }
      }
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Open songs (files or folders) in the Scratch list and play the first. */
  async openSongs(paths: string[]) {
    if (paths.length) await api.scratchAdd(paths).catch((e) => (this.error = String(e)));
  }

  async pickAndOpenSongs() {
    await this.openSongs(await api.pickSongs());
  }

  async removeFromScratch(ids: number[]) {
    await api.scratchRemove(ids).catch((e) => (this.error = String(e)));
    this.selected = new Set();
  }
  /** Path of the iPod being read, while it's being read. */
  opening = $state<string | null>(null);
  ejecting = $state<string | null>(null);
  error = $state<string | null>(null);

  source = $state<Source>({ kind: "none" });
  search = $state("");
  showBrowser = $state(true);
  showArtwork = $state(true);
  /** The look: "modern" (the Chinotto colours) or "retro" (glossy blue, drawn art). */
  theme = $state(loadPref("chinotto.theme", ["modern", "retro"] as const, "modern"));
  setTheme(theme: "modern" | "retro") {
    this.theme = theme;
    savePref("chinotto.theme", theme);
    document.documentElement.dataset.theme = theme;
  }
  /** Column browser: which columns show, and what's picked in each. */
  browserFields = $state<BrowseField[]>(loadList("chinotto.browserFields", ["genre", "artist", "album"]) as BrowseField[]);
  browserSel = $state<Partial<Record<BrowseField, string | null>>>({});
  /** Song list columns that are hidden. */
  hiddenColumns = $state<string[]>(loadList("chinotto.hiddenColumns", []));
  /** Song list columns, in the order shown (they can be dragged around). */
  columnOrder = $state<string[]>(loadList("chinotto.columnOrder", []));
  /** How a library's or iPod's Music is shown. */
  musicView = $state<MusicView>(loadPref("chinotto.view", VIEWS, "songs"));
  /** Album whose songs are open in the Albums view (album key). */
  openAlbum = $state<string | null>(null);
  /** Name picked in the Artists, Genres and Composers views. */
  groupPick = $state<Partial<Record<MusicView, string>>>({});

  /** Sorting per source (playlist id), like iTunes: playlists start in their own order. */
  private sorts = $state<Record<string, { key: SortKey; dir: 1 | -1 }>>({});
  selected = $state.raw<Set<number>>(new Set());
  /** Last clicked track (anchor for shift-click ranges). */
  anchor = $state<number | null>(null);

  /** What's being written to the iPod right now (shown in the status display). */
  busy = $state<string | null>(null);
  progress = $state<AddProgress | null>(null);
  dialog = $state<Dialog | null>(null);
  menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  /** Playlist whose name is being edited in the source list. */
  renaming = $state<string | null>(null);
  /** Songs being dragged inside the window (Chinotto's own drag, not the OS's). */
  songDrag = $state<{ dbids: string[]; x: number; y: number; target: string | null; insertAt?: number | null } | null>(null);

  // ---- Playback ----
  player = $state<api.PlayerStatus>({ state: "stopped", positionMs: 0, durationMs: null });
  /** dbid of the song loaded in the player. */
  nowPlaying = $state<string | null>(null);
  /** Songs to play, in order (dbids), and where we are in it. */
  queue = $state.raw<string[]>([]);
  queuePos = $state(0);
  volume = $state(loadVolume());
  /** Pictures chosen per device (identity → picture key). */
  deviceIcons = $state<Record<string, string>>({});
  shuffle = $state(loadPref("chinotto.shuffle", ["on", "off"] as const, "off") === "on");
  repeat = $state<Repeat>(loadPref("chinotto.repeat", ["off", "all", "one"] as const, "off"));
  /** The queue in the list's own order (the queue itself is shuffled when shuffle is on). */
  private listOrder: string[] = [];

  get playingTrack(): Track | null {
    const d = this.nowPlaying;
    if (!d) return null;
    if (d.startsWith("scratch:")) return this.scratch?.index.byId.get(Number(d.slice(8))) ?? null;
    if (d.startsWith("lib:")) {
      const [, lib, id] = d.split(":");
      return this.libViews[lib]?.index.byId.get(Number(id)) ?? null;
    }
    return this.ipod?.tracks.find((t) => t.dbid === d) ?? null;
  }

  /** What's shown can be changed: a library always; the iPod when allowed. */
  get writable() {
    if (this.source.kind === "library") {
      // A player's songs can be changed when it allows changes.
      const player = this.playerOf(this.source.lib);
      if (player) return !!this.libView && !!this.playerStates[player.path]?.writable;
      return !!this.libView && !this.isDeviceLibrary(this.source.lib);
    }
    return !!this.ipod?.writable;
  }

  /** The library lists a player's songs (it's shown under Devices). */
  isDeviceLibrary(lib: string): boolean {
    return !!this.libraries.find((l) => l.id === lib)?.device;
  }

  /** Libraries of the computer's own music (not players'). */
  get ownLibraries(): LibrarySummary[] {
    return this.libraries.filter((l) => !l.device);
  }

  /** Whether playlist files can be changed for the current source. */
  get playlistsWritable(): boolean {
    const s = this.source;
    if (!this.writable) return false;
    return s.kind !== "library" || this.libraryPlaylistsWritable(s.lib);
  }

  /** MSC playlists are editable only when the player is writable and uses M3U. */
  libraryPlaylistsWritable(lib: string): boolean {
    const player = this.playerOf(lib);
    if (!player) return !this.isDeviceLibrary(lib);
    const format = this.libraries.find((l) => l.id === lib)?.playlistFormat?.toLowerCase();
    return player.kind === "msc" && !!this.playerStates[player.path]?.writable && (format === "m3u" || format === "m3u8");
  }

  /** The regular (non-library, non-smart) playlist being shown, if any. */
  get editablePlaylist() {
    const s = this.source;
    if (s.kind === "library") {
      if (!this.libraryPlaylistsWritable(s.lib)) return null;
      const p = this.libView?.view.playlists.find((x) => x.id === s.id);
      return p && !p.master ? p : null;
    }
    if (s.kind !== "playlist" || !this.ipod) return null;
    const p = this.ipod.playlists.find((x) => x.id === s.id);
    return p && !p.master && !p.smart ? p : null;
  }

  dbidsOf(ids: number[]): string[] {
    return this.index ? this.index.get(ids).map((t) => t.dbid) : [];
  }

  get masterPlaylist() {
    return this.ipod?.playlists.find((p) => p.master) ?? null;
  }

  /** Tracks of the current source before search and browser filters. */
  get sourceTracks(): Track[] {
    const s = this.source;
    if (s.kind === "scratch") return this.scratch?.tracks ?? [];
    if (s.kind === "library") {
      const lv = this.libView;
      const pl = lv?.view.playlists.find((p) => p.id === s.id);
      return lv && pl ? lv.index.get(pl.trackIds) : [];
    }
    if (!this.ipod || !this.index || s.kind !== "playlist") return [];
    const pl = this.ipod.playlists.find((p) => p.id === s.id);
    return pl ? this.index.get(pl.trackIds) : [];
  }

  get selectedTracks(): Track[] {
    return this.index ? this.index.get([...this.selected]) : [];
  }

  async init() {
    await api.onDevicesChanged((d) => this.devicesChanged(d));
    await api.onPlayerStatus((p) => {
      this.player = p;
      if (p.state === "stopped" && !this.advancing) this.nowPlaying = null;
    });
    await api.onPlayerEnded(() => void this.next(true));
    await api.onPlayerError((msg) => {
      this.error = msg;
      this.nowPlaying = null;
    });
    void api.playerVolume(this.volume);
    await api.onAddProgress((p) => {
      // A late event after the work finished mustn't bring the status back.
      if (!this.busy) return;
      if (this.burning) {
        const n = { p: Math.floor(p.done / 10) };
        this.progress = p;
        this.busy = p.current === "verify" ? t("Checking the disc… {p}%", n) : p.current === "prepare" ? t("Preparing the image… {p}%", n) : t("Burning… {p}%", n);
        return;
      }
      if (p.current === "openmg-database") {
        this.progress = null;
        this.busy = t("Saving the Walkman's song list… (this can take a few minutes; don't unplug it)");
        return;
      }
      this.progress = p;
      const n = { done: Math.min(p.done + 1, p.total), total: p.total };
      if (p.total) this.busy = (this.ripping ? t("Ripping {done} of {total}", n) : t("Adding {done} of {total}", n)) + (p.current ? `: ${p.current}` : "");
    });
    this.deviceIcons = await api.deviceIcons().catch(() => ({}));
    await api.onLibrariesChanged((l) => (this.libraries = l));
    await api.onLibraryUpdated((id) => void this.reloadLibrary(id));
    await api.onLibraryProgress((p) => (this.scanProgress = { ...this.scanProgress, [p.id]: p }));
    await api.onScratchChanged((play) => void this.loadScratch(play));
    await this.loadScratch(null);
    this.libraries = await api.listLibraries().catch(() => []);
    // Expanded libraries show their playlists in the sidebar.
    for (const l of this.libraries) if (!l.device && !this.collapsed.includes(l.id)) void this.loadLibrary(l.id);
    // Audio CDs: macOS mounts them, so looking is cheap; on Linux, reading a
    // disc's contents spins the drive, so it's done when Rip CD… is chosen.
    if (isMac) {
      void this.refreshDiscs();
      setInterval(() => void this.refreshDiscs(), 4000);
    }
    const start = await api.startupFolder();
    this.devicesChanged(await api.listDevices());
    if (start && !this.ipod) await this.open(start);
  }

  private devicesChanged(list: DeviceInfo[]) {
    const before = new Set(this.devices.map((d) => d.path));
    // An open MTP device keeps its details; one unplugged is forgotten.
    this.mtpInfos = Object.fromEntries(Object.entries(this.mtpInfos).filter(([k]) => list.some((d) => d.path === k)));
    list = list.map((d) => {
      const i = this.mtpInfos[d.path];
      return i ? { ...d, library: i.library, volume: i.name, capacity: i.capacity, free: i.free } : d;
    });
    this.devices = list;
    // Players set up as drives: their songs (and playlists, for the sidebar).
    for (const d of list) if ((d.kind === "msc" || d.kind === "openmg") && d.library && !this.libViews[d.library]) void this.loadPlayer(d.library);
    if (!list.some((d) => d.kind === "mtp")) this.mtpInfos = {};
    for (const d of list) if ((d.kind === "msc" || d.kind === "openmg") && !this.playerStates[d.path]) void this.loadPlayerState(d.path);
    // The player being shown was unplugged.
    const s = this.source;
    if (s.kind === "library" && this.isDeviceLibrary(s.lib) && !list.some((d) => d.library === s.lib)) this.select({ kind: "none" });
    if (s.kind === "player" && !list.some((d) => d.path === s.path)) this.select({ kind: "none" });
    if (s.kind === "openmg" && !list.some((d) => d.path === s.path)) this.select({ kind: "none" });
    // The open iPod was unplugged.
    if (this.ipod && !list.some((d) => d.path === this.ipod!.path)) this.forget();
    // Open a newly plugged-in iPod when nothing else is open.
    if (!this.ipod && !this.opening) {
      const ready = list.filter((d) => !d.needsSetup && d.kind === "ipod");
      const fresh = ready.find((d) => !before.has(d.path) && !d.manual) ?? (before.size === 0 ? ready[0] : undefined);
      if (fresh) void this.open(fresh.path, true);
    }
  }

  private forget() {
    if (this.nowPlaying) void this.stopPlayback();
    this.ipod = null;
    this.ipodIndex = null;
    if (this.source.kind !== "library") {
      this.source = { kind: "none" };
      this.selected = new Set();
      this.resetBrowser();
    }
  }

  /** iPod whose database couldn't be read (it can be rebuilt). */
  openFailed = $state<string | null>(null);

  async open(path: string, auto = false) {
    if (this.opening) return;
    if (this.devices.find((d) => d.path === path)?.needsSetup) {
      if (!auto) this.dialog = { kind: "setup", path, rebuild: false };
      return;
    }
    this.opening = path;
    this.error = null;
    try {
      const view = await api.openIpod(path);
      this.openFailed = null;
      this.ipodIndex = new TrackIndex(view.tracks);
      this.ipod = view;
      // Opened by hand, or nothing else shown: show the iPod's music.
      if (this.source.kind !== "library" || !auto) {
        this.selected = new Set();
        this.resetBrowser();
        const master = view.playlists.find((p) => p.master);
        this.source = master ? { kind: "playlist", id: master.id } : { kind: "device" };
      }
    } catch (e) {
      this.error = String(e);
      this.openFailed = path;
    } finally {
      this.opening = null;
    }
  }

  /** Pick a drive to set up as an iPod (one that isn't found by itself). */
  async pickDriveToSetUp() {
    const path = await api.pickFolder();
    if (path) this.dialog = { kind: "setup", path, rebuild: false };
  }

  /** Set up the iPod at `path` (see SetupDialog); then open it. */
  async setUpIpod(a: { path: string; model: string; name: string; firewireId: string | null; addExisting: boolean; rebuild: boolean }) {
    if (this.busy) return;
    if (this.ipod?.path === a.path) this.forget();
    this.dialog = null;
    this.busy = a.rebuild ? t("Rebuilding the database…") : t("Setting up the iPod…");
    this.error = null;
    try {
      const r = await api.setupIpod(a);
      this.openFailed = null;
      this.busy = null;
      this.progress = null;
      this.devicesChanged(await api.listDevices());
      if (!this.devices.some((d) => d.path === a.path)) {
        // A drive picked by hand: keep it in the list.
        const d = await api.addFolder(a.path);
        this.devices = [...this.devices, d];
      }
      await this.open(a.path);
      const lines = [...r.problems];
      if (r.setAside) lines.unshift(t("The old database was kept in {path}.", { path: r.setAside }));
      if (lines.length) this.dialog = { kind: "report", title: tn(r.added, "Done: {n} song is on the iPod", "Done: {n} songs are on the iPod"), lines };
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = null;
      this.progress = null;
    }
  }

  /** Open MTP devices' details, by key ("mtp:<bus>-<dev>"). */
  mtpInfos = $state<Record<string, import("./types").MtpInfo>>({});

  /** Open an MTP device (reading its songs takes a moment) and show its page. */
  async openMtp(key: string) {
    if (this.opening) return;
    if (this.mtpInfos[key]) return this.select({ kind: "player", path: key });
    this.opening = key;
    this.error = null;
    try {
      const info = await api.mtpOpen(key);
      // Only one MTP device is open at a time.
      this.mtpInfos = { [key]: info };
      this.devices = this.devices.map((d) => (d.path === key ? { ...d, library: info.library, volume: info.name, capacity: info.capacity, free: info.free } : d.kind === "mtp" ? { ...d, library: null } : d));
      this.libraries = await api.listLibraries();
      await this.loadLibrary(info.library);
      await this.loadPlayerState(key);
      this.select({ kind: "player", path: key });
    } catch (e) {
      this.error = String(e);
    } finally {
      this.opening = null;
    }
  }

  /** After copying to or deleting from an open MTP device: its songs and free space again. */
  private async refreshMtp(key: string) {
    const info = await api.mtpInfo(key).catch(() => null);
    if (!info) return;
    this.mtpInfos = { ...this.mtpInfos, [key]: info };
    this.devices = this.devices.map((d) => (d.path === key ? { ...d, capacity: info.capacity, free: info.free } : d));
    await this.reloadLibrary(info.library);
  }

  /** Let go of an MTP device so other programs can use it. */
  async closeMtp(key: string) {
    const lib = this.mtpInfos[key]?.library;
    await api.mtpClose(key);
    this.mtpInfos = {};
    this.devices = this.devices.map((d) => (d.path === key ? { ...d, library: null } : d));
    const s = this.source;
    if ((s.kind === "player" && s.path === key) || (s.kind === "library" && s.lib === lib)) this.select({ kind: "none" });
    this.libraries = await api.listLibraries();
  }

  /** Players' page settings (Allow changes, conversion), by path. */
  playerStates = $state<Record<string, import("./types").PlayerState>>({});

  async loadPlayerState(path: string) {
    try {
      this.playerStates = { ...this.playerStates, [path]: await api.mscState(path) };
    } catch (e) {
      this.error = String(e);
    }
  }

  async setPlayerAllow(path: string, allow: boolean) {
    try {
      this.playerStates = { ...this.playerStates, [path]: await api.mscAllow(path, allow) };
    } catch (e) {
      this.error = String(e);
    }
  }

  async setPlayerConversion(path: string, c: Conversion) {
    try {
      this.playerStates = { ...this.playerStates, [path]: await api.mscSetConversion(path, c) };
    } catch (e) {
      this.error = String(e);
    }
  }

  /** The player (MSC) whose songs a library lists. */
  playerOf(lib: string): DeviceInfo | undefined {
    return this.devices.find((d) => d.library === lib);
  }

  /** Copy songs (dbids) and/or files onto a player. */
  async copyToPlayer(path: string, dbids: string[], files: string[] = []) {
    if (this.busy || (!dbids.length && !files.length)) return;
    const d = this.devices.find((x) => x.path === path);
    const n = dbids.length + files.length;
    this.busy = tn(n, "Copying {n} song to {name}…", "Copying {n} songs to {name}…", { name: d?.volume ?? "" });
    this.error = null;
    try {
      const r = await api.mscAdd(path, dbids, files);
      if (d?.kind === "mtp") await this.refreshMtp(path);
      if (d?.kind === "openmg") this.openMgRevision++;
      const lines = [...r.skipped];
      if (r.already) lines.unshift(tn(r.already, "{n} song was already on the player.", "{n} songs were already on the player."));
      if (r.skipped.length) this.dialog = { kind: "report", title: tn(r.added, "{n} song copied", "{n} songs copied"), lines };
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = null;
      this.progress = null;
    }
  }

  /** Bumped after songs are added to an OpenMG Walkman (its page reads its list again). */
  openMgRevision = $state(0);

  /** Put an OpenMG Walkman's albums in track order (numbers from the libraries). */
  async reorderOpenMg(path: string) {
    if (this.busy) return;
    this.busy = t("Putting albums in track order…");
    this.error = null;
    try {
      const n = await api.openMgReorder(path);
      this.openMgRevision++;
      this.dialog = { kind: "report", title: t("Albums are in track order"), lines: [tn(n, "Track number found for {n} song in your libraries.", "Track numbers found for {n} songs in your libraries.")] };
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = null;
    }
  }

  async pickAndCopyToPlayer(path: string) {
    const files = await api.pickMusicFiles();
    if (files.length) await this.copyToPlayer(path, [], files);
  }

  async deleteFromPlayer(path: string, ids: number[]) {
    this.dialog = null;
    try {
      const problems = await api.mscDelete(path, ids);
      if (path.startsWith("mtp:")) await this.refreshMtp(path);
      this.selected = new Set();
      if (problems.length) this.dialog = { kind: "report", title: tn(problems.length, "{n} file couldn't be deleted", "{n} files couldn't be deleted"), lines: problems };
    } catch (e) {
      this.error = String(e);
    }
  }

  /** A player's library, once its first scan has made it. */
  private async loadPlayer(lib: string) {
    this.libraries = await api.listLibraries();
    if (this.libraries.some((l) => l.id === lib)) await this.loadLibrary(lib);
  }

  /** Save a player's settings (Add Drive); then show its songs. */
  async saveMsc(path: string, config: import("./types").MscConfig) {
    try {
      const d = await api.mscSave(path, config);
      this.dialog = null;
      this.devices = [...this.devices.filter((x) => x.path !== d.path), d];
      if (d.library) {
        await this.loadPlayer(d.library);
        if (this.libViews[d.library]) this.select({ kind: "library", lib: d.library, id: "all" });
      }
    } catch (e) {
      this.error = String(e);
    }
  }

  async openFolder() {
    const path = await api.pickFolder();
    if (!path) return;
    try {
      // Not an iPod: a player that shows up as a drive.
      const info = await api.mscInspect(path);
      if (!info.isIpod) {
        this.dialog = { kind: "msc", path };
        return;
      }
      const d = await api.addFolder(path);
      if (!this.devices.some((x) => x.path === d.path)) this.devices = [...this.devices, d];
      await this.open(d.path);
    } catch (e) {
      this.error = String(e);
    }
  }

  async eject(path: string) {
    this.ejecting = path;
    this.error = null;
    try {
      if (this.ipod?.path === path) this.forget();
      await api.eject(path);
      this.devices = this.devices.filter((d) => d.path !== path);
    } catch (e) {
      this.error = t("Couldn't eject: {error}", { error: String(e) });
    } finally {
      this.ejecting = null;
    }
  }

  // ---- Changing the iPod ----

  /** Show a freshly re-read iPod, keeping the selection (as it is now, not
   *  when the change started) and the current playlist. A selection in a
   *  library or the Scratch list isn't the iPod's and is left alone. */
  private applyView(view: IpodView) {
    const onIpod = this.source.kind === "playlist" || this.source.kind === "device";
    const keep = onIpod ? this.selectedTracks.map((t) => t.dbid) : [];
    const anchor = onIpod && this.anchor !== null ? (this.index?.byId.get(this.anchor)?.dbid ?? null) : null;
    const index = new TrackIndex(view.tracks);
    const byDbid = new Map(view.tracks.map((t) => [t.dbid, t.id]));
    this.ipodIndex = index;
    this.ipod = view;
    if (onIpod) {
      this.selected = new Set(keep.map((d) => byDbid.get(d)).filter((x): x is number => x !== undefined));
      this.anchor = anchor !== null ? (byDbid.get(anchor) ?? null) : null;
    }
    const s = this.source;
    if (s.kind === "playlist" && !view.playlists.some((p) => p.id === s.id)) {
      const master = view.playlists.find((p) => p.master);
      this.source = master ? { kind: "playlist", id: master.id } : { kind: "device" };
    }
  }

  /** Run one change against the open iPod. Returns `{ extra }` (the command's extra result), or null on failure. */
  private async write<T>(label: string, run: (path: string) => Promise<WriteResult<T>>): Promise<{ extra: T } | null> {
    if (!this.ipod || this.busy) return null;
    this.busy = label;
    this.error = null;
    try {
      const r = await run(this.ipod.path);
      this.applyView(r.view);
      return { extra: r.extra };
    } catch (e) {
      this.error = String(e);
      return null;
    } finally {
      this.busy = null;
      this.progress = null;
    }
  }

  async setConversion(c: Conversion) {
    if (!this.ipod) return;
    try {
      this.applyView(await api.setConversion(this.ipod.path, c));
    } catch (e) {
      this.error = String(e);
    }
  }

  async allowChanges(allow: boolean) {
    if (!this.ipod) return;
    try {
      this.applyView(await api.setAllowChanges(this.ipod.path, allow));
    } catch (e) {
      this.error = String(e);
    }
  }

  async saveInfo(ids: string[], changes: TrackChanges, artwork: { set: string } | "remove" | null = null) {
    const s = this.source;
    if (s.kind === "library") {
      if (!Object.keys(changes).length && !artwork) return true;
      this.busy = ids.length > 1 ? tn(ids.length, "Saving…", "Saving {n} songs…") : t("Saving…");
      try {
        const cover = artwork === "remove" ? "" : artwork ? artwork.set : null;
        const problems = await api.libEditTracks(s.lib, ids.map(libSongId), changes, cover);
        if (problems.length) this.dialog = { kind: "report", title: tn(problems.length, "{n} file couldn't be changed", "{n} files couldn't be changed"), lines: problems };
        return !problems.length;
      } catch (e) {
        this.error = String(e);
        return false;
      } finally {
        this.busy = null;
      }
    }
    if (Object.keys(changes).length && (await this.write(t("Saving…"), (p) => api.editTracks(p, ids, changes))) === null) return false;
    if (artwork === "remove") return (await this.write(t("Removing artwork…"), (p) => api.removeArtwork(p, ids))) !== null;
    if (artwork) return (await this.write(t("Adding artwork…"), (p) => api.setArtwork(p, ids, artwork.set))) !== null;
    return true;
  }

  /** An image file dropped on the window while Song Details is open. */
  infoImageDrop = $state<string | null>(null);

  // ---- Libraries ----

  /** URL of a song's cover about `px` wide, or null when it has none. */
  coverUrl(t: Track | null | undefined, px: number): string | null {
    if (!t) return null;
    if (t.dbid.startsWith("scratch:")) return t.hasCover ? api.libArtUrl("scratch", t.id, px, t.modified ?? 0) : null;
    if (t.dbid.startsWith("lib:")) {
      const [, lib] = t.dbid.split(":");
      // The file's date is in the URL, so a changed cover isn't served from the cache.
      return t.hasCover ? api.libArtUrl(lib, t.id, px, t.modified ?? 0) : null;
    }
    return this.ipod && t.art ? api.artUrl(this.ipod.generation, t.art, px) : null;
  }

  /** Show a library's songs (loading them the first time). */
  async showLibrary(lib: string, id = "all") {
    if (!this.libViews[lib] && !(await this.loadLibrary(lib))) return;
    this.select({ kind: "library", lib, id });
  }

  async loadLibrary(lib: string): Promise<boolean> {
    try {
      const view = await api.libraryView(lib);
      this.libViews = { ...this.libViews, [lib]: { view, index: new TrackIndex(view.tracks) } };
      return true;
    } catch (e) {
      this.error = String(e);
      return false;
    }
  }

  /** A scan finished: re-read the library if it's loaded, keeping the selection. */
  private async reloadLibrary(lib: string) {
    const { [lib]: _, ...rest } = this.scanProgress;
    this.scanProgress = rest;
    if (!this.libViews[lib]) {
      // A player's first scan: show its songs and playlists.
      if (this.devices.some((d) => d.library === lib)) await this.loadPlayer(lib);
      return;
    }
    const showing = this.source.kind === "library" && this.source.lib === lib;
    const keep = showing ? this.selectedTracks.map((t) => t.dbid) : [];
    if (!(await this.loadLibrary(lib)) || !showing) return;
    const byDbid = new Map(this.libViews[lib].view.tracks.map((t) => [t.dbid, t.id]));
    this.selected = new Set(keep.map((d) => byDbid.get(d)).filter((x): x is number => x !== undefined));
  }

  /** New empty playlist in a library (from its menu). */
  async newLibraryPlaylist(lib: string) {
    await this.showLibrary(lib);
    if (this.collapsed.includes(lib)) this.toggleCollapsed(lib);
    await this.newPlaylist(false);
  }

  toggleCollapsed(lib: string) {
    if (this.collapsed.includes(lib) && !this.libViews[lib]) void this.loadLibrary(lib);
    this.collapsed = this.collapsed.includes(lib) ? this.collapsed.filter((x) => x !== lib) : [...this.collapsed, lib];
    savePref("chinotto.collapsed", JSON.stringify(this.collapsed));
  }

  async createLibrary(name: string, folders: string[]) {
    try {
      const id = await api.createLibrary(name, folders);
      this.dialog = null;
      this.libraries = await api.listLibraries();
      await this.showLibrary(id);
    } catch (e) {
      this.error = String(e);
    }
  }

  async updateLibrary(id: string, name: string, folders: string[]) {
    try {
      await api.updateLibrary(id, name, folders);
      this.dialog = null;
      this.libraries = await api.listLibraries();
      const lv = this.libViews[id];
      if (lv) this.libViews = { ...this.libViews, [id]: { ...lv, view: { ...lv.view, name } } };
    } catch (e) {
      this.error = String(e);
    }
  }

  async deleteLibrary(id: string) {
    this.dialog = null;
    try {
      if (this.nowPlaying?.startsWith(`lib:${id}:`)) await this.stopPlayback();
      await api.deleteLibrary(id);
      const { [id]: _, ...rest } = this.libViews;
      this.libViews = rest;
      if (this.source.kind === "library" && this.source.lib === id) this.select(this.ipod ? { kind: "device" } : { kind: "none" });
      this.libraries = await api.listLibraries();
    } catch (e) {
      this.error = String(e);
    }
  }

  // ---- Audio CDs ----

  /** Audio CDs in the drives (macOS: kept up to date; Linux: when asked). */
  discs = $state<import("./types").Disc[]>([]);
  /** A rip is running (the progress says "Ripping"). */
  ripping = $state(false);

  async refreshDiscs() {
    const d = await api.cdList().catch(() => []);
    if (JSON.stringify(d) !== JSON.stringify(this.discs)) this.discs = d;
    const s = this.source;
    if (s.kind === "disc" && !d.some((x) => x.id === s.id)) this.select({ kind: "none" });
    for (const id of Object.keys(this.discDrafts)) if (!d.some((x) => x.id === id)) delete this.discDrafts[id];
    for (const x of d) if (!this.discDrafts[x.id]) this.discDrafts[x.id] = this.newDraft(x.name);
  }

  // ---- Burning disc images ----

  burning = $state(false);
  burnStatus = $state<import("./types").BurnStatus | null>(null);
  burnImage = $state<import("./types").DiscImage | null>(null);
  /** 0: the slowest the drive and disc allow. */
  burnSpeed = $state(0);
  burnVerify = $state(true);

  async refreshBurn() {
    if (this.burning) return;
    const s = await api.burnStatus().catch(() => null);
    if (JSON.stringify(s) !== JSON.stringify(this.burnStatus)) this.burnStatus = s;
  }

  async chooseBurnImage() {
    const path = await api.pickDiscImage();
    if (!path) return;
    try {
      this.burnImage = await api.burnInspect(path);
      this.error = null;
    } catch (e) {
      this.error = String(e);
    }
  }

  async burn() {
    const img = this.burnImage;
    if (!img || this.busy) return;
    this.burning = true;
    this.busy = t("Burning…");
    this.error = null;
    try {
      const lines = await api.burnImage(img.path, this.burnSpeed, this.burnVerify);
      this.dialog = { kind: "report", title: t("The disc is ready"), lines };
    } catch (e) {
      this.error = String(e);
    } finally {
      this.burning = false;
      this.busy = null;
      this.progress = null;
      void this.refreshBurn();
    }
  }

  /** What's been typed for each disc in a drive. */
  discDrafts = $state<Record<string, import("./types").DiscDraft>>({});

  private newDraft(name: string): import("./types").DiscDraft {
    let format = "alac";
    try {
      format = localStorage.getItem("chinotto.ripFormat") ?? "alac";
    } catch {
      /* not kept */
    }
    // A disc's volume name is often its title (CD-Text); "Audio CD" isn't.
    return { artist: "", album: name === "Audio CD" ? "" : name, year: "", genre: "", format, titles: {}, skip: {}, cover: null, preview: null };
  }

  /** Show the Rip page: of `id`, else the first disc (looking for one first on Linux). */
  async showDisc(id: string | null = null) {
    if (!this.discs.length) await this.refreshDiscs();
    const d = this.discs.find((x) => x.id === id) ?? this.discs[0];
    if (d) this.select({ kind: "disc", id: d.id });
    else this.error = t("No audio CD found. Put one in the drive, then try again. (On Linux, ripping needs cdparanoia.)");
  }

  async ejectDisc(id: string) {
    try {
      await api.cdEject(id);
      await this.refreshDiscs();
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Ask for a folder, then rip into it. Libraries whose folders hold it scan it. */
  async rip(req: Omit<import("./types").RipRequest, "folder">) {
    if (this.busy) return;
    const folder = await api.pickFolder(t("Rip the CD into"));
    if (!folder) return;
    this.dialog = null;
    this.ripping = true;
    this.busy = t("Ripping…");
    this.error = null;
    try {
      const r = await api.cdRip({ ...req, folder });
      const inside = (dir: string) => r.folder === dir || r.folder.startsWith(dir.endsWith("/") ? dir : dir + "/");
      for (const l of this.libraries) if (!l.device && l.folders.some((f) => inside(f.path))) void this.scanLibrary(l.id);
      this.dialog = {
        kind: "report",
        title: tn(r.ripped, "Ripped {n} song", "Ripped {n} songs"),
        lines: [t("Saved in {path}", { path: r.folder }), ...r.problems],
      };
    } catch (e) {
      this.error = String(e);
    } finally {
      this.ripping = false;
      this.busy = null;
      this.progress = null;
    }
  }

  /** Scan a library's folders; `full` reads every song's tags again. */
  async scanLibrary(id: string, full = false) {
    try {
      await api.scanLibrary(id, full);
    } catch (e) {
      this.error = String(e);
    }
  }

  async removeUnavailable(id: string) {
    try {
      const n = await api.removeUnavailable(id);
      this.dialog = { kind: "report", title: n ? tn(n, "Removed {n} unavailable song", "Removed {n} unavailable songs") : t("No unavailable songs"), lines: [] };
    } catch (e) {
      this.error = String(e);
    }
  }

  /** The picture to show for a device: the user's choice, else its model's. */
  iconFor(d: { icon: string; identity?: string | null }): string {
    return (d.identity && this.deviceIcons[d.identity]) || d.icon;
  }

  /** Choose a device's picture (null: automatic). */
  async setDeviceIcon(identity: string, icon: string | null) {
    try {
      await api.setDeviceIcon(identity, icon);
      const next = { ...this.deviceIcons };
      if (icon) next[identity] = icon;
      else delete next[identity];
      this.deviceIcons = next;
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Save a song's cover to a PNG file. */
  async saveCover(t: Track) {
    if (!t.art && !t.hasCover) return;
    try {
      const fromFile = t.dbid.startsWith("lib:") || t.dbid.startsWith("scratch:");
      await api.saveCover(fromFile ? t.dbid : t.art, t.album || t.title || "Cover");
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Paste the clipboard's image as the cover of these songs. */
  async pasteCover(ids: string[]) {
    if (!ids.length || !this.writable) return;
    let image: string;
    try {
      image = await api.pasteImage();
    } catch (e) {
      this.error = String(e);
      return;
    }
    if (this.source.kind === "library") await this.saveInfo(ids, {}, { set: image });
    else await this.write(t("Adding artwork…"), (p) => api.setArtwork(p, ids, image));
  }

  /** Delete key / menu: in a playlist, take songs out of it; elsewhere, delete from the iPod (asks first). */
  removeSelected() {
    if (this.source.kind === "scratch" && this.selected.size) return void this.removeFromScratch([...this.selected]);
    // Songs on a player: delete them from it (asks first).
    const s = this.source;
    const player = s.kind === "library" ? this.playerOf(s.lib) : undefined;
    if (player && this.selected.size) return void (this.dialog = { kind: "delete-player-songs", path: player.path, ids: [...this.selected] });
    if (!this.writable || !this.selected.size) return;
    const pl = this.editablePlaylist;
    if (pl) void this.removeFromPlaylist(pl.id, [...this.selected]);
    else if (this.source.kind === "library") this.dialog = { kind: "remove-from-library", ids: [...this.selected] };
    else this.dialog = { kind: "delete-songs", ids: this.selectedTracks.map((t) => t.dbid) };
  }

  async deleteSongs(ids: string[]) {
    this.dialog = null;
    const n = ids.length;
    const ok = await this.write(tn(n, "Deleting {n} song…", "Deleting {n} songs…"), (p) => api.deleteTracks(p, ids));
    if (ok !== null) this.selected = new Set();
  }

  private playlistDbids(id: string): string[] {
    const pl = this.ipod?.playlists.find((p) => p.id === id);
    return pl ? (this.ipodIndex?.get(pl.trackIds).map((t) => t.dbid) ?? []) : [];
  }

  /** A library playlist, from its sidebar id "lib:<library>:<playlist>". */
  private libPlaylist(scoped: string) {
    const [, lib, pid] = scoped.split(":");
    const pl = this.libViews[lib]?.view.playlists.find((p) => p.id === pid);
    return pl ? { lib, pl } : null;
  }

  /** Run a change to a library; the library-updated event then shows it. */
  private async libChange<T>(run: () => Promise<T>): Promise<T | null> {
    this.error = null;
    try {
      return await run();
    } catch (e) {
      this.error = String(e);
      return null;
    }
  }

  async removeFromPlaylist(id: string, trackIds: number[]) {
    const s = this.source;
    if (s.kind === "library") {
      const pl = this.libView?.view.playlists.find((p) => p.id === id);
      if (!pl) return;
      const drop = new Set(trackIds);
      const ok = await this.libChange(() => api.libSetPlaylistTracks(s.lib, id, pl.trackIds.filter((t) => !drop.has(t))));
      if (ok !== null) this.selected = new Set();
      return;
    }
    const drop = new Set(this.dbidsOf(trackIds));
    const rest = this.playlistDbids(id).filter((d) => !drop.has(d));
    const ok = await this.write(t("Saving…"), (p) => api.setPlaylistTracks(p, id, rest));
    if (ok !== null) this.selected = new Set();
  }

  /** Add songs to a playlist: an iPod playlist id, or "lib:<library>:<playlist>". */
  async addToPlaylist(id: string, dbids: string[]) {
    // Dropped on a player: copy the songs onto it.
    if (id.startsWith("player:")) return this.copyToPlayer(id.slice(7), dbids);
    if (id.startsWith("lib:")) {
      const lp = this.libPlaylist(id);
      if (!lp) return;
      // Only the same library's songs (copying between libraries isn't a thing).
      const add = dbids.filter((d) => d.startsWith(`lib:${lp.lib}:`)).map(libSongId);
      if (!add.length) {
        this.error = t("A library's playlists can only hold songs from that library.");
        return;
      }
      const have = new Set(lp.pl.trackIds);
      const fresh = add.filter((x) => !have.has(x));
      if (fresh.length) await this.libChange(() => api.libSetPlaylistTracks(lp.lib, lp.pl.id, [...lp.pl.trackIds, ...fresh]));
      return;
    }
    // Library or Scratch songs onto the iPod ("ipod:music", or one of its playlists): copy them over.
    const fromComputer = (d: string) => d.startsWith("lib:") || d.startsWith("scratch:");
    if (dbids.some(fromComputer)) {
      await this.copyToIpod(dbids.filter(fromComputer), id === "ipod:music" ? null : id);
      return;
    }
    if (id === "ipod:music") return;
    const have = this.playlistDbids(id);
    const present = new Set(have);
    const add = dbids.filter((d) => !d.startsWith("lib:") && !present.has(d));
    if (!add.length) return;
    await this.write(t("Saving…"), (p) => api.setPlaylistTracks(p, id, [...have, ...add]));
  }

  async newPlaylist(fromSelection: boolean) {
    if (!this.playlistsWritable) return;
    const s = this.source;
    if (s.kind === "library") {
      const ids = fromSelection ? this.selectedTracks.map((t) => t.id) : [];
      const pid = await this.libChange(() => api.libCreatePlaylist(s.lib, t("untitled playlist"), ids));
      if (pid) {
        await this.loadLibrary(s.lib);
        this.select({ kind: "library", lib: s.lib, id: pid });
        this.renaming = `lib:${s.lib}:${pid}`;
      }
      return;
    }
    const ids = fromSelection ? this.selectedTracks.map((t) => t.dbid) : [];
    const r = await this.write(t("Saving…"), (p) => api.createPlaylist(p, t("untitled playlist"), ids));
    if (r) {
      this.select({ kind: "playlist", id: r.extra.id });
      this.renaming = r.extra.id;
    }
  }

  async renamePlaylist(id: string, name: string) {
    this.renaming = null;
    const clean = name.trim();
    if (id.startsWith("lib:")) {
      const t = this.libPlaylist(id);
      if (t && clean && clean !== t.pl.name) await this.libChange(() => api.libRenamePlaylist(t.lib, t.pl.id, clean));
      return;
    }
    const pl = this.ipod?.playlists.find((p) => p.id === id);
    if (!pl || !clean || clean === pl.name) return;
    await this.write(t("Saving…"), (p) => api.renamePlaylist(p, id, clean));
  }

  async deletePlaylist(id: string) {
    this.dialog = null;
    if (id.startsWith("lib:")) {
      const t = this.libPlaylist(id);
      if (!t) return;
      if (this.source.kind === "library" && this.source.lib === t.lib && this.source.id === t.pl.id) this.select({ kind: "library", lib: t.lib, id: "all" });
      await this.libChange(() => api.libDeletePlaylist(t.lib, t.pl.id));
      return;
    }
    await this.write(t("Deleting playlist…"), (p) => api.deletePlaylist(p, id));
  }

  /** Take songs out of a library (files stay where they are). */
  async removeFromLibrary(ids: number[]) {
    this.dialog = null;
    const s = this.source;
    if (s.kind !== "library") return;
    const ok = await this.libChange(() => api.libRemoveTracks(s.lib, ids));
    if (ok !== null) this.selected = new Set();
  }

  /** Copy files or folders onto the iPod (into the current playlist too, if one is shown). */
  async addFiles(paths: string[]) {
    if (!paths.length || !this.writable) return;
    const pl = this.editablePlaylist?.id ?? null;
    const res = await this.write(t("Adding songs…"), (p) => api.addFiles(p, paths, pl));
    if (!res) return;
    const r = res.extra;
    if (r.skipped.length) {
      this.dialog = {
        kind: "report",
        title: tn(r.added.length, "Added {n} song; {skipped} not added", "Added {n} songs; {skipped} not added", { skipped: r.skipped.length }),
        lines: r.skipped,
      };
    }
    // Show what was added: clear filters that could hide it.
    this.search = "";
    this.resetBrowser();
    const byDbid = new Map(this.ipod!.tracks.map((t) => [t.dbid, t.id]));
    this.selected = new Set(r.added.map((d) => byDbid.get(d)).filter((x): x is number => x !== undefined));
  }

  /**
   * Copy library songs onto the open iPod (into one of its playlists too, if
   * given). Songs the iPod already has (same name, artist, album and length)
   * aren't copied again; formats the iPod can't play are reported.
   */
  async copyToIpod(dbids: string[], playlist: string | null) {
    if (!this.ipod) return;
    if (!this.ipod.writable) {
      this.error = t("Changes to this iPod are switched off. Turn on “Allow changes” on its page first.");
      return;
    }
    const songs = dbids.map((d) => this.trackByDbid(d)).filter((t): t is Track => !!t && !!t.location && !t.unavailable);
    const same = (a: Track, b: Track) =>
      (a.title ?? "") === (b.title ?? "") && (a.artist ?? "") === (b.artist ?? "") && (a.album ?? "") === (b.album ?? "") && Math.abs(a.lengthMs - b.lengthMs) < 2000;
    const onIpod = new Map<string, Track[]>();
    for (const t of this.ipod.tracks) {
      const k = (t.title ?? "").toLowerCase();
      onIpod.set(k, [...(onIpod.get(k) ?? []), t]);
    }
    const already: Track[] = [];
    const files: string[] = [];
    for (const t of songs) {
      const match = onIpod.get((t.title ?? "").toLowerCase())?.find((x) => same(x, t));
      if (match) already.push(match);
      else files.push(t.location!);
    }
    let added: string[] = [];
    const skipped: string[] = [];
    if (files.length) {
      const res = await this.write(tn(files.length, "Copying {n} song to {name}…", "Copying {n} songs to {name}…", { name: this.ipod.name }), (p) => api.addFiles(p, files, playlist));
      if (!res) return;
      added = res.extra.added;
      skipped.push(...res.extra.skipped);
    }
    // Songs already there still go into the playlist.
    if (playlist && already.length) await this.addToPlaylist(playlist, already.map((t) => t.dbid));
    const lines = [...skipped];
    if (already.length) lines.push(tn(already.length, "{n} song already on the iPod, not copied again.", "{n} songs already on the iPod, not copied again."));
    if (skipped.length || (already.length && !added.length)) {
      this.dialog = { kind: "report", title: tn(added.length, "Copied {n} song to {name}", "Copied {n} songs to {name}", { name: this.ipod?.name ?? t("the iPod") }), lines };
    }
  }

  /** Save songs to the computer: one as its own file, several as a zip. */
  async exportSongs(tracks: Track[]) {
    const songs = tracks
      .filter((x) => !x.unavailable)
      .map((x) => ({ dbid: x.dbid, title: x.title, artist: x.artist, album: x.album, trackNr: x.trackNr }));
    if (!songs.length) return;
    try {
      let dest: string | null;
      if (songs.length === 1) {
        const name = await api.exportName(songs[0]);
        dest = await api.pickSaveFile(name, t("Save Song"), name.split(".").pop() ?? "mp3");
      } else {
        dest = await api.pickSaveFile(t("{n} songs", { n: songs.length }) + ".zip", t("Save Songs as a Zip"), "zip");
      }
      if (!dest) return;
      this.busy = tn(songs.length, "Saving {n} song…", "Saving {n} songs…");
      await api.exportSongs(songs, dest);
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = null;
    }
  }

  /** A song by dbid, from the iPod or any loaded library. */
  trackByDbid(d: string): Track | null {
    if (d.startsWith("scratch:")) return this.scratch?.index.byId.get(Number(d.slice(8))) ?? null;
    if (d.startsWith("lib:")) {
      const [, lib, id] = d.split(":");
      return this.libViews[lib]?.index.byId.get(Number(id)) ?? null;
    }
    return this.ipod?.tracks.find((t) => t.dbid === d) ?? null;
  }

  async pickAndAddFiles() {
    const files = await api.pickMusicFiles();
    if (files.length) await this.addFiles(files);
  }

  async pickAndAddFolder() {
    const dir = await api.pickMusicFolder();
    if (dir) await this.addFiles([dir]);
  }

  // ---- Playback ----

  /** True while switching songs, so the brief "stopped" in between isn't taken as the end. */
  private advancing = false;

  /** Play `list` (in its current order) starting at `startIndex`. */
  async playList(list: Track[], startIndex: number) {
    if (!list.length) return;
    this.listOrder = list.map((t) => t.dbid);
    const start = Math.max(0, Math.min(startIndex, list.length - 1));
    if (this.shuffle) {
      // The chosen song first, then the rest in random order.
      const first = this.listOrder[start];
      this.queue = [first, ...shuffled(this.listOrder.filter((_, i) => i !== start))];
      await this.playAt(0);
    } else {
      this.queue = this.listOrder;
      await this.playAt(start);
    }
  }

  setShuffle(on: boolean) {
    this.shuffle = on;
    savePref("chinotto.shuffle", on ? "on" : "off");
    if (!this.queue.length) return;
    // Rearrange what's still to come; the playing song stays put.
    const current = this.queue[this.queuePos];
    if (on) {
      this.queue = [current, ...shuffled(this.listOrder.filter((d) => d !== current))];
      this.queuePos = 0;
    } else {
      this.queue = this.listOrder;
      this.queuePos = Math.max(0, this.listOrder.indexOf(current));
    }
  }

  /** Off → all → one → off, as iTunes' repeat button. */
  cycleRepeat() {
    this.repeat = this.repeat === "off" ? "all" : this.repeat === "all" ? "one" : "off";
    savePref("chinotto.repeat", this.repeat);
  }

  /** Whether Next has somewhere to go. */
  get hasNext(): boolean {
    return !!this.nowPlaying && (this.queuePos + 1 < this.queue.length || this.repeat === "all");
  }

  private async playAt(pos: number, skipped = 0): Promise<void> {
    const dbid = this.queue[pos];
    if (!dbid) return this.stopPlayback();
    this.queuePos = pos;
    this.nowPlaying = dbid;
    this.advancing = true;
    try {
      await api.playTrack(dbid);
      if (skipped) this.error = tn(skipped, "Skipped {n} song whose file couldn't be played.", "Skipped {n} songs whose file couldn't be played.");
    } catch (e) {
      // Like iTunes: skip a song that can't be played (e.g. its file is missing) and go on,
      // but give up after a few in a row.
      if (skipped < 4 && pos + 1 < this.queue.length) return this.playAt(pos + 1, skipped + 1);
      this.error = String(e);
      this.nowPlaying = null;
    } finally {
      // The player confirms with a status event shortly.
      setTimeout(() => (this.advancing = false), 300);
    }
  }

  /** Play/pause button and Space bar. */
  async togglePlay(visible: Track[]) {
    if (this.nowPlaying && this.player.state === "playing") return api.playerPause();
    if (this.nowPlaying && this.player.state === "paused") return api.playerPlay();
    // Nothing loaded: play the selection's first song (or the list from the top).
    const sel = visible.findIndex((t) => this.selected.has(t.id));
    await this.playList(visible, sel >= 0 ? sel : 0);
  }

  /** `auto`: the song finished by itself (repeat one replays it then). */
  async next(auto = false) {
    if (!this.queue.length) return;
    if (auto && this.repeat === "one") await this.playAt(this.queuePos);
    else if (this.queuePos + 1 < this.queue.length) await this.playAt(this.queuePos + 1);
    else if (this.repeat === "all") {
      // From the top again; a new random order each time round when shuffling.
      if (this.shuffle) this.queue = shuffled(this.listOrder);
      await this.playAt(0);
    } else if (auto) await this.stopPlayback();
  }

  /** Like iTunes: back to the start of the song if more than 3 s in, else the previous song. */
  async previous() {
    if (!this.nowPlaying) return;
    if (this.player.positionMs > 3000) return api.playerSeek(0);
    if (this.queuePos === 0) {
      if (this.repeat === "all" && this.queue.length > 1) return this.playAt(this.queue.length - 1);
      return api.playerSeek(0);
    }
    await this.playAt(this.queuePos - 1);
  }

  async seek(ms: number) {
    this.player = { ...this.player, positionMs: ms };
    await api.playerSeek(ms);
  }

  setVolume(v: number) {
    this.volume = v;
    void api.playerVolume(v);
    try {
      localStorage.setItem("chinotto.volume", String(v));
    } catch {
      /* not essential */
    }
  }

  async stopPlayback() {
    this.nowPlaying = null;
    this.queue = [];
    this.listOrder = [];
    await api.playerStop();
  }

  /** Music (a library's or the iPod's whole collection) can be shown in several views. */
  get hasViews(): boolean {
    const s = this.source;
    if (s.kind === "library") return s.id === "all";
    return s.kind === "playlist" && s.id === this.masterPlaylist?.id;
  }

  /** The view in use: other lists (playlists) are always songs. */
  get view(): MusicView {
    return this.hasViews ? this.musicView : "songs";
  }

  setView(v: MusicView) {
    this.musicView = v;
    this.openAlbum = null;
    savePref("chinotto.view", v);
  }

  select(src: Source) {
    this.openAlbum = null;
    this.groupPick = {};
    this.source = src;
    this.selected = new Set();
    this.anchor = null;
    this.resetBrowser();
  }

  /** Searching from the device page searches the iPod's music. */
  searchStarted() {
    const master = this.masterPlaylist;
    if (this.search && this.source.kind === "device" && master) {
      this.source = { kind: "playlist", id: master.id };
      this.resetBrowser();
    }
  }

  resetBrowser() {
    this.browserSel = {};
  }

  /** Pick in a browser column; the columns to its right go back to "All". */
  pick(field: BrowseField, v: string | null) {
    const i = this.browserFields.indexOf(field);
    const next: Partial<Record<BrowseField, string | null>> = {};
    for (const f of this.browserFields.slice(0, i)) next[f] = this.browserSel[f] ?? null;
    next[field] = v;
    this.browserSel = next;
  }

  /** True when the browser is narrowing the list. */
  get browserFiltering(): boolean {
    return this.showBrowser && Object.values(this.browserSel).some((v) => v !== null && v !== undefined);
  }

  toggleBrowserField(f: BrowseField) {
    const on = this.browserFields.includes(f);
    if (on && this.browserFields.length === 1) return; // keep at least one
    const all: BrowseField[] = ["genre", "format", "year", "composer", "artist", "album"];
    this.browserFields = all.filter((x) => (x === f ? !on : this.browserFields.includes(x)));
    this.resetBrowser();
    savePref("chinotto.browserFields", JSON.stringify(this.browserFields));
  }

  toggleColumn(key: string) {
    this.hiddenColumns = this.hiddenColumns.includes(key) ? this.hiddenColumns.filter((k) => k !== key) : [...this.hiddenColumns, key];
    savePref("chinotto.hiddenColumns", JSON.stringify(this.hiddenColumns));
    // Hiding the sorted column sorts by artist instead.
    if (this.hiddenColumns.includes(this.sortKey)) this.setSort("artist", 1);
  }

  /** Put column `key` before column `before` (null: at the end). */
  moveColumn(key: string, before: string | null, all: string[]) {
    const order = this.columnsInOrder(all).filter((k) => k !== key);
    const i = before === null ? order.length : order.indexOf(before);
    order.splice(i < 0 ? order.length : i, 0, key);
    this.columnOrder = order;
    savePref("chinotto.columnOrder", JSON.stringify(order));
  }

  /** `all` (the default order) arranged as the user left it; new columns keep their default place. */
  columnsInOrder(all: string[]): string[] {
    const saved = this.columnOrder.filter((k) => all.includes(k));
    const out = [...saved];
    all.forEach((k, i) => {
      if (out.includes(k)) return;
      // After the column that comes before it by default.
      const prev = all.slice(0, i).reverse().find((p) => out.includes(p));
      out.splice(prev ? out.indexOf(prev) + 1 : 0, 0, k);
    });
    return out;
  }

  private get sortSlot(): string {
    const s = this.source;
    return s.kind === "playlist" ? s.id : s.kind === "library" ? `lib:${s.lib}:${s.id}` : "";
  }

  /** Whether the shown list is a playlist with an order of its own (not the whole library). */
  get hasOwnOrder(): boolean {
    const s = this.source;
    if (s.kind === "library") return s.id !== "all";
    return s.kind === "playlist" && !!this.ipod?.playlists.some((p) => p.id === s.id && !p.master);
  }

  get sortKey(): SortKey {
    return this.sorts[this.sortSlot]?.key ?? (this.hasOwnOrder ? "order" : "artist");
  }

  get sortDir(): 1 | -1 {
    return this.sorts[this.sortSlot]?.dir ?? 1;
  }

  private setSort(key: SortKey, dir: 1 | -1) {
    this.sorts = { ...this.sorts, [this.sortSlot]: { key, dir } };
  }

  sortBy(key: SortKey) {
    if (this.sortKey === key) this.setSort(key, this.sortDir === 1 ? -1 : 1);
    else this.setSort(key, 1);
  }

  /** Songs can be dragged into a new order: an editable playlist, in its own order, unfiltered. */
  get canReorder(): boolean {
    return !!this.editablePlaylist && this.playlistsWritable && this.sortKey === "order" && this.sortDir === 1 && !this.search && !this.browserFiltering;
  }

  /** Move the selected songs of the shown playlist to before position `at` (in the playlist). */
  async moveSelected(at: number) {
    const pl = this.editablePlaylist;
    if (!pl || !this.index) return;
    const tracks = this.index.get(pl.trackIds);
    const moving = tracks.filter((t) => this.selected.has(t.id));
    if (!moving.length) return;
    const before = tracks.slice(0, at).filter((t) => !this.selected.has(t.id));
    const after = tracks.slice(at).filter((t) => !this.selected.has(t.id));
    const order = [...before, ...moving, ...after].map((t) => t.dbid);
    if (order.every((d, i) => d === tracks[i].dbid)) return; // no change
    const s = this.source;
    if (s.kind === "library") await this.libChange(() => api.libSetPlaylistTracks(s.lib, pl.id, order.map(libSongId)));
    else await this.write(t("Saving…"), (p) => api.setPlaylistTracks(p, pl.id, order));
  }

  /** Click handling for the song list: plain, Shift (range), Cmd/Ctrl (toggle). */
  clickTrack(id: number, visible: Track[], e: { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean }) {
    const toggle = isMac ? e.metaKey : e.ctrlKey || e.metaKey;
    if (e.shiftKey && this.anchor !== null) {
      const a = visible.findIndex((t) => t.id === this.anchor);
      const b = visible.findIndex((t) => t.id === id);
      if (a >= 0 && b >= 0) {
        const [lo, hi] = a < b ? [a, b] : [b, a];
        const next = toggle ? new Set(this.selected) : new Set<number>();
        for (let i = lo; i <= hi; i++) next.add(visible[i].id);
        this.selected = next;
        return;
      }
    }
    if (toggle) {
      const next = new Set(this.selected);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      this.selected = next;
    } else {
      this.selected = new Set([id]);
    }
    this.anchor = id;
  }

  selectAll(visible: Track[]) {
    this.selected = new Set(visible.map((t) => t.id));
  }

  /** Song the list should scroll to (set by Show Playing Song). */
  reveal = $state<number | null>(null);

  /** Show the playing song in its list, selected (⌘L). */
  revealPlaying() {
    const d = this.nowPlaying;
    const track = this.playingTrack;
    if (!d || !track) return;
    let src: Source;
    if (d.startsWith("scratch:")) src = { kind: "scratch" };
    else if (d.startsWith("lib:")) src = { kind: "library", lib: d.split(":")[1], id: "all" };
    else if (this.masterPlaylist) src = { kind: "playlist", id: this.masterPlaylist.id };
    else return;
    this.select(src);
    this.search = "";
    if (this.hasViews && this.musicView !== "songs") this.setView("songs");
    this.selected = new Set([track.id]);
    this.anchor = track.id;
    this.reveal = track.id;
  }

  /** Arrow-key movement in the song list; returns the new row index. */
  moveSelection(visible: Track[], delta: number, extend: boolean): number | null {
    if (!visible.length) return null;
    const cur = this.anchor !== null ? visible.findIndex((t) => t.id === this.anchor) : -1;
    const next = Math.max(0, Math.min(visible.length - 1, cur < 0 ? 0 : cur + delta));
    const id = visible[next].id;
    if (extend) {
      const s = new Set(this.selected);
      s.add(id);
      this.selected = s;
    } else {
      this.selected = new Set([id]);
    }
    this.anchor = id;
    return next;
  }
}

export const store = new Store();
