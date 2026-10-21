// App-wide state (Svelte 5 runes).

import * as api from "./api";
import { TrackIndex } from "./library";
import type { AddProgress, DeviceInfo, IpodView, SortKey, Source, Track, TrackChanges, WriteResult } from "./types";

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
  | { kind: "report"; title: string; lines: string[] };

function loadVolume(): number {
  try {
    const v = Number(localStorage.getItem("chinotto.volume"));
    return v > 0 && v <= 1 ? v : 0.8;
  } catch {
    return 0.8;
  }
}

export type Repeat = "off" | "all" | "one";

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
  index = $state.raw<TrackIndex | null>(null);
  /** Path of the iPod being read, while it's being read. */
  opening = $state<string | null>(null);
  ejecting = $state<string | null>(null);
  error = $state<string | null>(null);

  source = $state<Source>({ kind: "none" });
  search = $state("");
  showBrowser = $state(true);
  showArtwork = $state(true);
  genre = $state<string | null>(null);
  artist = $state<string | null>(null);
  album = $state<string | null>(null);
  sortKey = $state<SortKey>("artist");
  sortDir = $state<1 | -1>(1);
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
  songDrag = $state<{ dbids: string[]; x: number; y: number; target: string | null } | null>(null);

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
    if (!this.nowPlaying || !this.ipod) return null;
    return this.ipod.tracks.find((t) => t.dbid === this.nowPlaying) ?? null;
  }

  get writable() {
    return !!this.ipod?.writable;
  }

  /** The regular (non-library, non-smart) playlist being shown, if any. */
  get editablePlaylist() {
    const s = this.source;
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
      this.progress = p;
      if (p.total) this.busy = `Adding ${Math.min(p.done + 1, p.total)} of ${p.total}${p.current ? `: ${p.current}` : ""}`;
    });
    this.deviceIcons = await api.deviceIcons().catch(() => ({}));
    const start = await api.startupFolder();
    this.devicesChanged(await api.listDevices());
    if (start && !this.ipod) await this.open(start);
  }

  private devicesChanged(list: DeviceInfo[]) {
    const before = new Set(this.devices.map((d) => d.path));
    this.devices = list;
    // The open iPod was unplugged.
    if (this.ipod && !list.some((d) => d.path === this.ipod!.path)) this.forget();
    // Open a newly plugged-in iPod when nothing else is open.
    if (!this.ipod && !this.opening) {
      const fresh = list.find((d) => !before.has(d.path) && !d.manual) ?? (before.size === 0 ? list[0] : undefined);
      if (fresh) void this.open(fresh.path);
    }
  }

  private forget() {
    if (this.nowPlaying) void this.stopPlayback();
    this.ipod = null;
    this.index = null;
    this.source = { kind: "none" };
    this.selected = new Set();
    this.resetBrowser();
  }

  async open(path: string) {
    if (this.opening) return;
    this.opening = path;
    this.error = null;
    try {
      const view = await api.openIpod(path);
      this.index = new TrackIndex(view.tracks);
      this.ipod = view;
      this.selected = new Set();
      this.resetBrowser();
      const master = view.playlists.find((p) => p.master);
      this.source = master ? { kind: "playlist", id: master.id } : { kind: "device" };
    } catch (e) {
      this.error = String(e);
    } finally {
      this.opening = null;
    }
  }

  async openFolder() {
    const path = await api.pickFolder();
    if (!path) return;
    try {
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
      this.error = `Couldn't eject: ${e}`;
    } finally {
      this.ejecting = null;
    }
  }

  // ---- Changing the iPod ----

  /** Show a freshly re-read iPod, keeping the selection and current playlist. */
  private applyView(view: IpodView, keep: string[] = [], anchor: string | null = null) {
    const index = new TrackIndex(view.tracks);
    const byDbid = new Map(view.tracks.map((t) => [t.dbid, t.id]));
    this.index = index;
    this.ipod = view;
    this.selected = new Set(keep.map((d) => byDbid.get(d)).filter((x): x is number => x !== undefined));
    this.anchor = anchor !== null ? (byDbid.get(anchor) ?? null) : null;
    const s = this.source;
    if (s.kind === "playlist" && !view.playlists.some((p) => p.id === s.id)) {
      const master = view.playlists.find((p) => p.master);
      this.source = master ? { kind: "playlist", id: master.id } : { kind: "device" };
    }
  }

  /** Run one change against the open iPod. Returns `{ extra }` (the command's extra result), or null on failure. */
  private async write<T>(label: string, run: (path: string) => Promise<WriteResult<T>>): Promise<{ extra: T } | null> {
    if (!this.ipod || this.busy) return null;
    const keep = this.selectedTracks.map((t) => t.dbid);
    const anchor = this.anchor !== null ? (this.index?.byId.get(this.anchor)?.dbid ?? null) : null;
    this.busy = label;
    this.error = null;
    try {
      const r = await run(this.ipod.path);
      this.applyView(r.view, keep, anchor);
      return { extra: r.extra };
    } catch (e) {
      this.error = String(e);
      return null;
    } finally {
      this.busy = null;
      this.progress = null;
    }
  }

  async allowChanges(allow: boolean) {
    if (!this.ipod) return;
    try {
      this.applyView(await api.setAllowChanges(this.ipod.path, allow), this.selectedTracks.map((t) => t.dbid));
    } catch (e) {
      this.error = String(e);
    }
  }

  async saveInfo(ids: string[], changes: TrackChanges, artwork: { set: string } | "remove" | null = null) {
    if (Object.keys(changes).length && (await this.write("Saving…", (p) => api.editTracks(p, ids, changes))) === null) return false;
    if (artwork === "remove") return (await this.write("Removing artwork…", (p) => api.removeArtwork(p, ids))) !== null;
    if (artwork) return (await this.write("Adding artwork…", (p) => api.setArtwork(p, ids, artwork.set))) !== null;
    return true;
  }

  /** An image file dropped on the window while Get Info is open. */
  infoImageDrop = $state<string | null>(null);

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
    if (!t.art) return;
    try {
      await api.saveCover(t.art, t.album || t.title || "Cover");
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
    await this.write("Adding artwork…", (p) => api.setArtwork(p, ids, image));
  }

  /** Delete key / menu: in a playlist, take songs out of it; elsewhere, delete from the iPod (asks first). */
  removeSelected() {
    if (!this.writable || !this.selected.size) return;
    const pl = this.editablePlaylist;
    if (pl) void this.removeFromPlaylist(pl.id, [...this.selected]);
    else this.dialog = { kind: "delete-songs", ids: this.selectedTracks.map((t) => t.dbid) };
  }

  async deleteSongs(ids: string[]) {
    this.dialog = null;
    const n = ids.length;
    const ok = await this.write(`Deleting ${n} ${n === 1 ? "song" : "songs"}…`, (p) => api.deleteTracks(p, ids));
    if (ok !== null) this.selected = new Set();
  }

  private playlistDbids(id: string): string[] {
    const pl = this.ipod?.playlists.find((p) => p.id === id);
    return pl ? this.dbidsOf(pl.trackIds) : [];
  }

  async removeFromPlaylist(id: string, trackIds: number[]) {
    const drop = new Set(this.dbidsOf(trackIds));
    const rest = this.playlistDbids(id).filter((d) => !drop.has(d));
    const ok = await this.write("Saving…", (p) => api.setPlaylistTracks(p, id, rest));
    if (ok !== null) this.selected = new Set();
  }

  async addToPlaylist(id: string, dbids: string[]) {
    const have = this.playlistDbids(id);
    const present = new Set(have);
    const add = dbids.filter((d) => !present.has(d));
    if (!add.length) return;
    await this.write("Saving…", (p) => api.setPlaylistTracks(p, id, [...have, ...add]));
  }

  async newPlaylist(fromSelection: boolean) {
    const ids = fromSelection ? this.selectedTracks.map((t) => t.dbid) : [];
    const r = await this.write("Saving…", (p) => api.createPlaylist(p, "untitled playlist", ids));
    if (r) {
      this.select({ kind: "playlist", id: r.extra.id });
      this.renaming = r.extra.id;
    }
  }

  async renamePlaylist(id: string, name: string) {
    this.renaming = null;
    const pl = this.ipod?.playlists.find((p) => p.id === id);
    const clean = name.trim();
    if (!pl || !clean || clean === pl.name) return;
    await this.write("Saving…", (p) => api.renamePlaylist(p, id, clean));
  }

  async deletePlaylist(id: string) {
    this.dialog = null;
    await this.write("Deleting playlist…", (p) => api.deletePlaylist(p, id));
  }

  /** Copy files or folders onto the iPod (into the current playlist too, if one is shown). */
  async addFiles(paths: string[]) {
    if (!paths.length || !this.writable) return;
    const pl = this.editablePlaylist?.id ?? null;
    const res = await this.write("Adding songs…", (p) => api.addFiles(p, paths, pl));
    if (!res) return;
    const r = res.extra;
    if (r.skipped.length) {
      this.dialog = {
        kind: "report",
        title: `Added ${r.added.length} ${r.added.length === 1 ? "song" : "songs"}; ${r.skipped.length} not added`,
        lines: r.skipped,
      };
    }
    // Show what was added: clear filters that could hide it.
    this.search = "";
    this.resetBrowser();
    const byDbid = new Map(this.ipod!.tracks.map((t) => [t.dbid, t.id]));
    this.selected = new Set(r.added.map((d) => byDbid.get(d)).filter((x): x is number => x !== undefined));
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
      if (skipped) this.error = `Skipped ${skipped} ${skipped === 1 ? "song" : "songs"} whose file couldn't be played.`;
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

  select(src: Source) {
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
    this.genre = this.artist = this.album = null;
  }

  pickGenre(g: string | null) {
    this.genre = g;
    this.artist = this.album = null;
  }
  pickArtist(a: string | null) {
    this.artist = a;
    this.album = null;
  }
  pickAlbum(a: string | null) {
    this.album = a;
  }

  sortBy(key: SortKey) {
    if (this.sortKey === key) this.sortDir = this.sortDir === 1 ? -1 : 1;
    else {
      this.sortKey = key;
      this.sortDir = 1;
    }
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
