// What a song row does on right-click and when dragged, shared by the song
// list and the album views.

import { store } from "./store.svelte";
import type { Track } from "./types";
import { t, tn } from "./i18n";

const plain = { shiftKey: false, metaKey: false, ctrlKey: false };

/** Play from the first selected song of `tracks`, continuing down the list. */
export function playSelected(tracks: Track[]) {
  const i = tracks.findIndex((x) => store.selected.has(x.id));
  if (i >= 0) void store.playList(tracks, i);
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
  if (src.kind === "scratch") {
    // Opened from outside the libraries: play, look, save, copy to the iPod, or take off the list.
    store.menu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        { label: t("Play"), action: () => playSelected(tracks) },
        { label: t("Get Info"), action: () => (store.dialog = { kind: "info" }) },
        { label: n > 1 ? t("Save As Zip…") : t("Save As…"), action: () => store.exportSongs(store.selectedTracks) },
        ...(store.ipod
          ? [{ label: t("Copy to {name}", { name: store.ipod.name }), disabled: !store.ipod.writable, action: () => store.copyToIpod(dbids, null) }]
          : []),
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
      { label: t("Get Info"), action: () => (store.dialog = { kind: "info" }) },
      { label: n > 1 ? t("Save As Zip…") : t("Save As…"), action: () => store.exportSongs(store.selectedTracks) },
      { label: "", separator: true },
      {
        label: t("Add to Playlist"),
        disabled: ro || !targets.length,
        submenu: targets.map((p) => ({ label: p.name, action: () => store.addToPlaylist(p.id, dbids) })),
      },
      { label: t("New Playlist from Selection"), disabled: ro, action: () => store.newPlaylist(true) },
      { label: "", separator: true },
      ...(pl ? [{ label: t("Remove from “{name}”", { name: pl.name }), disabled: ro, action: () => store.removeFromPlaylist(pl.id, [...store.selected]) }] : []),
      ...(!onIpod && store.ipod
        ? [
            {
              label: t("Copy to {name}", { name: store.ipod.name }),
              disabled: !store.ipod.writable,
              submenu: [
                { label: t("Music"), action: () => store.copyToIpod(dbids, null) },
                ...store.ipod.playlists.filter((p) => !p.master && !p.smart).map((p) => ({ label: p.name, action: () => store.copyToIpod(dbids, p.id) })),
              ],
            },
          ]
        : []),
      ...(!onIpod
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
export function startSongDrag(
  e: MouseEvent,
  t: Track,
  tracks: Track[],
  insertionAt: ((x: number, y: number) => number | null) | null = null,
) {
  // Scratch songs can be dragged too (onto the iPod).
  if (e.button !== 0 || !(store.writable || store.source.kind === "scratch")) return;
  const x0 = e.clientX, y0 = e.clientY;
  const move = (m: MouseEvent) => {
    if (!store.songDrag) {
      if (Math.hypot(m.clientX - x0, m.clientY - y0) < 6) return;
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
    const d = store.songDrag;
    store.songDrag = null;
    if (d?.target) void store.addToPlaylist(d.target, d.dbids);
    else if (d?.insertAt != null) void store.moveSelected(d.insertAt);
  };
  window.addEventListener("mousemove", move);
  window.addEventListener("mouseup", up);
}
