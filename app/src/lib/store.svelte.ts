// App-wide state (Svelte 5 runes).

import * as api from "./api";
import { TrackIndex } from "./library";
import type { DeviceInfo, IpodView, SortKey, Source, Track } from "./types";

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
    const toggle = e.metaKey || e.ctrlKey;
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
