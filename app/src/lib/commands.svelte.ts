// Chinotto's commands, reachable from the keyboard everywhere and, on macOS,
// from the menu bar (which shows each shortcut).
//
// On macOS the web view sees a key press before the menu does: when the key
// handler here acts it calls preventDefault and the menu stays quiet; menu
// clicks run the same functions.

import { store, isMac, type MusicView } from "./store.svelte";
import { t } from "./i18n";
import type { Track } from "./types";

/** The songs the window shows right now (for Play and Select All). */
let visibleSongs: () => Track[] = () => [];
export function setVisibleSongs(f: () => Track[]) {
  visibleSongs = f;
}

const VIEWS: MusicView[] = ["songs", "albums", "artists", "genres", "composers", "compilations"];

export const run = {
  songDetails: () => store.selected.size && (store.dialog = { kind: "info" }),
  newPlaylist: () => store.writable && void store.newPlaylist(false),
  newLibrary: () => (store.dialog = { kind: "library", id: null }),
  openFile: () => void store.pickAndOpenSongs(),
  openDevice: () => void store.openFolder(),
  addToIpod: () => store.ipod?.writable && void store.pickAndAddFiles(),
  saveAs: () => store.selected.size && void store.exportSongs(store.selectedTracks),
  eject: () => store.ipod && !store.ipod.device.manual && !store.ejecting && void store.eject(store.ipod.path),
  find: () => document.querySelector<HTMLInputElement>("input[type=search]")?.focus(),
  selectAll: () => store.selectAll(visibleSongs()),
  view: (i: number) => store.hasViews && VIEWS[i] && store.setView(VIEWS[i]),
  toggleBrowser: () => (store.showBrowser = !store.showBrowser),
  toggleArtwork: () => (store.showArtwork = !store.showArtwork),
  showPlaying: () => store.revealPlaying(),
  playPause: () => store.index && void store.togglePlay(visibleSongs()),
  stop: () => store.nowPlaying && void store.stopPlayback(),
  next: () => store.hasNext && void store.next(),
  previous: () => store.nowPlaying && void store.previous(),
  volumeUp: () => store.setVolume(Math.min(1, store.volume + 0.1)),
  volumeDown: () => store.setVolume(Math.max(0, store.volume - 0.1)),
  shuffle: () => store.setShuffle(!store.shuffle),
  repeat: () => store.cycleRepeat(),
};

/** Keyboard shortcuts for the whole window. Returns true when it acted. */
export function handleKey(e: KeyboardEvent): boolean {
  if (e.defaultPrevented || store.dialog || store.menu) return false;
  const target = e.target as HTMLElement;
  const typing = !!target.closest("input, textarea, select, [contenteditable]");
  const mod = isMac ? e.metaKey : e.ctrlKey;
  const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  let act: (() => unknown) | null = null;
  if (!mod) {
    if (k === " " && !typing && !target.closest("button")) act = run.playPause;
  } else if (e.shiftKey) {
    if (k === "o") act = run.addToIpod;
    else if (k === "s") act = run.saveAs;
  } else if (e.altKey) {
    if (k === "n") act = run.newLibrary;
  } else {
    const map: Record<string, () => unknown> = {
      i: run.songDetails,
      n: run.newPlaylist,
      o: run.openFile,
      e: run.eject,
      f: run.find,
      b: run.toggleBrowser,
      g: run.toggleArtwork,
      l: run.showPlaying,
      ".": run.stop,
      ArrowRight: run.next,
      ArrowLeft: run.previous,
      ArrowUp: run.volumeUp,
      ArrowDown: run.volumeDown,
    };
    // In text fields the arrows, ⌘A and the like keep their usual meaning.
    if (!typing || !["ArrowRight", "ArrowLeft", "ArrowUp", "ArrowDown"].includes(k)) act = map[k] ?? null;
    if (k === "a" && !typing) act = run.selectAll;
    if (/^[1-6]$/.test(k)) act = () => run.view(Number(k) - 1);
  }
  if (!act) return false;
  e.preventDefault();
  act();
  return true;
}

/** The macOS menu bar, with Chinotto's commands and their shortcuts. */
export async function installMenu() {
  if (!isMac) return;
  const { Menu, Submenu, PredefinedMenuItem, CheckMenuItem } = await import("@tauri-apps/api/menu");
  const sep = { item: "Separator" as const };
  const item = (text: string, action: () => unknown, accelerator?: string) => ({ text, accelerator, action: () => void action() });
  const shuffle = await CheckMenuItem.new({ text: t("Shuffle"), checked: store.shuffle, action: () => run.shuffle() });
  const app = await Submenu.new({
    text: "Chinotto",
    items: [
      await PredefinedMenuItem.new({ item: { About: null }, text: t("About Chinotto") }),
      sep,
      { item: "Services" },
      sep,
      { item: "Hide" },
      { item: "HideOthers" },
      { item: "ShowAll" },
      sep,
      { item: "Quit" },
    ],
  });
  const file = await Submenu.new({
    text: t("File"),
    items: [
      item(t("New Playlist"), run.newPlaylist, "CmdOrCtrl+N"),
      item(t("New Library…"), run.newLibrary, "CmdOrCtrl+Alt+N"),
      sep,
      item(t("Open File…"), run.openFile, "CmdOrCtrl+O"),
      item(t("Open Device…"), run.openDevice),
      item(t("Add Songs to iPod…"), run.addToIpod, "CmdOrCtrl+Shift+O"),
      sep,
      item(t("Song Details"), run.songDetails, "CmdOrCtrl+I"),
      item(t("Save As…"), run.saveAs, "CmdOrCtrl+Shift+S"),
      sep,
      item(t("Eject iPod"), run.eject, "CmdOrCtrl+E"),
      { item: "CloseWindow" },
    ],
  });
  const edit = await Submenu.new({
    text: t("Edit"),
    items: [
      { item: "Undo" },
      { item: "Redo" },
      sep,
      { item: "Cut" },
      { item: "Copy" },
      { item: "Paste" },
      { item: "SelectAll" },
      sep,
      item(t("Find"), run.find, "CmdOrCtrl+F"),
    ],
  });
  const view = await Submenu.new({
    text: t("View"),
    items: [
      item(t("Songs"), () => run.view(0), "CmdOrCtrl+1"),
      item(t("Albums"), () => run.view(1), "CmdOrCtrl+2"),
      item(t("Artists"), () => run.view(2), "CmdOrCtrl+3"),
      item(t("Genres"), () => run.view(3), "CmdOrCtrl+4"),
      item(t("Composers"), () => run.view(4), "CmdOrCtrl+5"),
      item(t("Compilations"), () => run.view(5), "CmdOrCtrl+6"),
      sep,
      item(t("Show or Hide Column Browser"), run.toggleBrowser, "CmdOrCtrl+B"),
      item(t("Show or Hide Artwork"), run.toggleArtwork, "CmdOrCtrl+G"),
      sep,
      item(t("Show Playing Song"), run.showPlaying, "CmdOrCtrl+L"),
      sep,
      { item: "Fullscreen" },
    ],
  });
  const controls = await Submenu.new({
    text: t("Controls"),
    items: [
      item(t("Play / Pause (Space)"), run.playPause),
      item(t("Stop"), run.stop, "CmdOrCtrl+."),
      item(t("Next"), run.next, "CmdOrCtrl+Right"),
      item(t("Previous"), run.previous, "CmdOrCtrl+Left"),
      sep,
      item(t("Volume Up"), run.volumeUp, "CmdOrCtrl+Up"),
      item(t("Volume Down"), run.volumeDown, "CmdOrCtrl+Down"),
      sep,
      shuffle,
      item(t("Repeat: Off, All, One"), run.repeat),
    ],
  });
  const windowMenu = await Submenu.new({ text: t("Window"), items: [{ item: "Minimize" }, { item: "Maximize" }, sep, { item: "BringAllToFront" }] });
  const menu = await Menu.new({ items: [app, file, edit, view, controls, windowMenu] });
  await menu.setAsAppMenu();
  await windowMenu.setAsWindowsMenuForNSApp();
  // Keep the Shuffle tick in step with the button.
  $effect.root(() => {
    $effect(() => {
      void shuffle.setChecked(store.shuffle);
    });
  });
}
