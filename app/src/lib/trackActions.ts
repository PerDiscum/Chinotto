// What a song row does on right-click and when dragged, shared by the song
// list and the album views.

import { store, isMac } from "./store.svelte";
import type { Track } from "./types";
import { t, tn } from "./i18n";

const plain = { shiftKey: false, metaKey: false, ctrlKey: false };

/** Play from the first selected song of `tracks`, continuing down the list. */
export function playSelected(tracks: Track[]) {
  const i = tracks.findIndex((x) => store.selected.has(x.id));
  if (i >= 0) void store.playList(tracks, i);
}

/** "Copy to <iPod>" (its Music or one of its playlists), when an iPod is open. */
function ipodCopy(dbids: string[]) {
  const ipod = store.ipod;
  if (!ipod) return [];
  return [
    {
      label: t("Copy to {name}", { name: ipod.name }),
      disabled: !ipod.writable,
      submenu: [
        { label: t("Music"), action: () => store.copyToIpod(dbids, null) },
        ...ipod.playlists.filter((p) => !p.master && !p.smart).map((p) => ({ label: p.name, action: () => store.copyToIpod(dbids, p.id) })),
      ],
    },
  ];
}

/** "Copy to <player>" for each player set up as a drive or open over MTP (not the one shown). */
function playerCopies(dbids: string[], shownLib: string | null) {
  return store.devices
    .filter((d) => (d.kind === "msc" || d.kind === "openmg" || (d.kind === "mtp" && !!d.library)) && d.library !== shownLib)
    .map((d) => ({ label: t("Copy to {name}", { name: d.volume }), action: () => store.copyToPlayer(d.path, dbids) }));
}

/** Right-click on a song of `tracks`. */
export function songMenu(e: MouseEvent, track: Track, tracks: Track[]) {
  e.preventDefault();
  // Right-clicking outside the selection selects that song.
  if (!store.selected.has(track.id)) store.clickTrack(track.id, tracks, plain);
  const n = store.selected.size;
  const ro = !store.writable;
  const pl = store.editablePlaylist;
  const dbids = store.selectedTracks.map((x) => x.dbid);
  const src = store.source;
  const onIpod = src.kind !== "library" && src.kind !== "scratch";
  // Songs on a player set up as a drive.
  const player = src.kind === "library" ? store.playerOf(src.lib) : undefined;
  const canDelete = player?.kind === "msc" || player?.kind === "mtp";
  const canEditPlaylists = !player || (player.kind === "msc" && store.playlistsWritable);
  if (src.kind === "scratch") {
    // Opened from outside the libraries: play, look, save, copy to the iPod, or take off the list.
    store.menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: t("Play"), action: () => playSelected(tracks) },
        { label: t("Song Details"), action: () => (store.dialog = { kind: "info" }) },
        { label: n > 1 ? t("Save As Zip…") : t("Save As…"), action: () => store.exportSongs(store.selectedTracks) },
        ...ipodCopy(dbids),
        ...playerCopies(dbids, null),
        { label: "", separator: true },
        { label: t("Remove from Scratch"), action: () => store.removeFromScratch([...store.selected]) },
      ],
    };
    return;
  }
  // Playlists to add to, as addToPlaylist ids.
  const targets = onIpod
    ? (store.ipod?.playlists ?? []).filter((p) => !p.master && !p.smart && p.id !== pl?.id).map((p) => ({ id: p.id, name: p.name }))
    : (store.libView?.view.playlists ?? []).filter((p) => !p.master && p.id !== pl?.id).map((p) => ({ id: `lib:${src.kind === "library" ? src.lib : ""}:${p.id}`, name: p.name }));
  store.menu = {
    x: e.clientX,
    y: e.clientY,
    items: [
      { label: t("Play"), action: () => playSelected(tracks) },
      { label: t("Song Details"), action: () => (store.dialog = { kind: "info" }) },
      { label: n > 1 ? t("Save As Zip…") : t("Save As…"), action: () => store.exportSongs(store.selectedTracks) },
      { label: "", separator: true },
      ...(canEditPlaylists
        ? [
            {
              label: t("Add to Playlist"),
              disabled: !store.playlistsWritable || !targets.length,
              submenu: targets.map((p) => ({ label: p.name, action: () => store.addToPlaylist(p.id, dbids) })),
            },
            { label: t("New Playlist from Selection"), disabled: !store.playlistsWritable, action: () => store.newPlaylist(true) },
            { label: "", separator: true },
          ]
        : []),
      ...(pl ? [{ label: t("Remove from “{name}”", { name: pl.name }), disabled: ro, action: () => store.removeFromPlaylist(pl.id, [...store.selected]) }] : []),
      ...(!onIpod ? ipodCopy(dbids) : []),
      ...playerCopies(dbids, src.kind === "library" ? src.lib : null),
      ...(!onIpod && player && canDelete
        ? [
            {
              label: tn(n, "Delete Song from {name}…", "Delete {n} Songs from {name}…", { name: player.volume }),
              danger: true,
              action: () => (store.dialog = { kind: "delete-player-songs", path: player.path, ids: [...store.selected] }),
            },
          ]
        : []),
      ...(!onIpod && !player
        ? [{ label: tn(n, "Remove Song from Library…", "Remove {n} Songs from Library…"), action: () => (store.dialog = { kind: "remove-from-library", ids: [...store.selected] }) }]
        : []),
      ...(onIpod
        ? [
            {
              label: tn(n, "Delete Song from iPod…", "Delete {n} Songs from iPod…"),
              danger: true,
              disabled: ro,
              action: () => (store.dialog = { kind: "delete-songs", ids: dbids }),
            },
          ]
        : []),
    ],
  };
}

/**
 * Press on a song and move: drag the selection onto a playlist in the source
 * list, or (with `insertionAt`) to a new place in the shown playlist.
 */
/** Scrolling the list during a drag is allowed only by the drag itself (see allowDragScroll). */
let scrollAllowedUntil = 0;
export function allowDragScroll() {
  scrollAllowedUntil = performance.now() + 100;
}

/**
 * Keep the list the drag started in where it is while dragging: nothing may
 * scroll it away (the web view's own edge scrolling included), except a
 * deliberate reorder scroll. Returns the function that lets go.
 */
function holdScroll(from: EventTarget | null): () => void {
  let el = from instanceof HTMLElement ? from : null;
  while (el && !(el.scrollHeight > el.clientHeight && /(auto|scroll)/.test(getComputedStyle(el).overflowY))) el = el.parentElement;
  if (!el) return () => {};
  const box = el;
  let top = box.scrollTop;
  const onScroll = () => {
    if (performance.now() < scrollAllowedUntil) top = box.scrollTop;
    else if (box.scrollTop !== top) box.scrollTop = top;
  };
  box.addEventListener("scroll", onScroll);
  return () => box.removeEventListener("scroll", onScroll);
}

export function startSongDrag(
  e: MouseEvent,
  t: Track,
  tracks: Track[],
  insertionAt: ((x: number, y: number) => number | null) | null = null,
) {
  // Any song can be dragged (to copy it somewhere); where it's dropped decides
  // what happens, and says so when it can't.
  if (e.button !== 0) return;
  const x0 = e.clientX, y0 = e.clientY;
  let release = () => {};
  const from = e.currentTarget;
  const move = (m: MouseEvent) => {
    if (!store.songDrag) {
      if (Math.hypot(m.clientX - x0, m.clientY - y0) < 6) return;
      release = holdScroll(from);
      if (!store.selected.has(t.id)) store.clickTrack(t.id, tracks, plain);
      store.songDrag = { dbids: store.selectedTracks.map((x) => x.dbid), x: m.clientX, y: m.clientY, target: null };
    }
    const over = document.elementFromPoint(m.clientX, m.clientY)?.closest<HTMLElement>("[data-drop-playlist]");
    const insertAt = insertionAt ? insertionAt(m.clientX, m.clientY) : null;
    store.songDrag = { ...store.songDrag, x: m.clientX, y: m.clientY, target: over?.dataset.dropPlaylist ?? null, insertAt };
  };
  const up = () => {
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", up);
    release();
    const d = store.songDrag;
    store.songDrag = null;
    if (d?.target) void store.addToPlaylist(d.target, d.dbids);
    else if (d?.insertAt != null) void store.moveSelected(d.insertAt);
  };
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", up);
}

/**
 * Press on an album's cover and move: its songs become the selection and are
 * dragged like songs (onto the iPod or a playlist). A plain click still opens it.
 */
export function startAlbumDrag(e: MouseEvent, tracks: Track[]) {
  if (e.button !== 0 || (isMac && e.ctrlKey) || !tracks.length) return;
  const x0 = e.clientX, y0 = e.clientY;
  let release = () => {};
  const from = e.currentTarget;
  const move = (m: MouseEvent) => {
    if (!store.songDrag) {
      if (Math.hypot(m.clientX - x0, m.clientY - y0) < 6) return;
      release = holdScroll(from);
      store.selected = new Set(tracks.map((x) => x.id));
      store.anchor = tracks[0].id;
      store.songDrag = { dbids: tracks.map((x) => x.dbid), x: m.clientX, y: m.clientY, target: null };
    }
    const over = document.elementFromPoint(m.clientX, m.clientY)?.closest<HTMLElement>("[data-drop-playlist]");
    store.songDrag = { ...store.songDrag, x: m.clientX, y: m.clientY, target: over?.dataset.dropPlaylist ?? null };
  };
  const up = () => {
    window.removeEventListener("mousemove", move);
    window.removeEventListener("mouseup", up);
    release();
    const d = store.songDrag;
    store.songDrag = null;
    if (!d) return;
    // The click that ends a drag doesn't open or close the album.
    const swallow = (c: MouseEvent) => c.stopPropagation();
    window.addEventListener("click", swallow, { capture: true, once: true });
    setTimeout(() => window.removeEventListener("click", swallow, { capture: true }), 0);
    if (d.target) void store.addToPlaylist(d.target, d.dbids);
  };
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", up);
}
