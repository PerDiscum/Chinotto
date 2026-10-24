<script lang="ts">
  import { t, tn } from "./lib/i18n";
  import { onMount } from "svelte";
  import { store } from "./lib/store.svelte";
  import { albums, browse, groups } from "./lib/library";
  import ViewTabs from "./lib/ViewTabs.svelte";
  import AlbumGrid from "./lib/AlbumGrid.svelte";
  import GroupView from "./lib/GroupView.svelte";
  import { count, longDuration, bytes } from "./lib/format";
  import Toolbar from "./lib/Toolbar.svelte";
  import SourceList from "./lib/SourceList.svelte";
  import ColumnBrowser from "./lib/ColumnBrowser.svelte";
  import SongList from "./lib/SongList.svelte";
  import DevicePage from "./lib/DevicePage.svelte";
  import PlayerPage from "./lib/PlayerPage.svelte";
  import OpenMgPage from "./lib/OpenMgPage.svelte";
  import DiscPage from "./lib/DiscPage.svelte";
  import MtpPage from "./lib/MtpPage.svelte";
  import ArtworkPane from "./lib/ArtworkPane.svelte";
  import Welcome from "./lib/Welcome.svelte";
  import Icon from "./lib/Icon.svelte";
  import Dialogs from "./lib/Dialogs.svelte";
  import ContextMenu from "./lib/ContextMenu.svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { handleKey, installMenu, setVisibleSongs } from "./lib/commands.svelte";

  /** Files being dragged in from Finder / the file manager. */
  let fileDrag = $state(false);

  onMount(() => {
    void store.init();
    setVisibleSongs(() => shown);
    void installMenu();
    const off = getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      // Only real files from Finder / the file manager (they carry paths);
      // drags inside the window are Chinotto's own song drags.
      if (p.type === "enter") fileDrag = p.paths.length > 0 && !store.songDrag && store.dialog?.kind !== "info";
      else if (p.type === "over") {
        /* keep the current state */
      }
      else if (p.type === "leave") fileDrag = false;
      else if (p.type === "drop") {
        fileDrag = false;
        if (store.dialog?.kind === "info") {
          // Dropped on Song Details: an image becomes the cover.
          const img = p.paths.find((x) => /\.(jpe?g|png|gif|bmp|webp|tiff?)$/i.test(x));
          if (img) store.infoImageDrop = img;
        } else if (store.ipod?.writable && (store.source.kind === "playlist" || store.source.kind === "device") && p.paths.length) {
          void store.addFiles(p.paths);
        } else if (p.paths.length) {
          // Anywhere else: just play them, in the Scratch list.
          void store.openSongs(p.paths);
        }
      }
    });
    return () => void off.then((f) => f());
  });

  /** A click on an empty part of the window clears the song selection. */
  function mousedown(e: MouseEvent) {
    if (store.dialog || store.menu || !store.selected.size) return;
    const el = e.target as HTMLElement;
    // Not on a scroll bar.
    if (e.offsetX > el.clientWidth || e.offsetY > el.clientHeight) return;
    const keep = "button, input, select, textarea, a, label, [role=row], [role=option], [role=menu], [role=dialog], [role=separator], .toolbar, .art, .catcher, .backdrop, .band";
    if (!el.closest(keep)) store.selected = new Set();
  }

  // Source → search → column browser → sort.
  const searched = $derived(store.index ? store.index.search(store.sourceTracks, store.search) : []);
  const browsed = $derived(
    store.showBrowser && store.view === "songs" ? browse(searched, store.browserFields, store.browserSel) : null,
  );
  const albumList = $derived(store.view === "albums" || store.view === "compilations" ? albums(searched) : []);
  const filtered = $derived(browsed ? browsed.tracks : searched);
  const visible = $derived(store.index ? store.index.sort(filtered, store.sortKey, store.sortDir) : []);
  /** The songs the current view actually shows (for ⌘A and Space): in the
   *  album views the open album or every shown album, in Artists/Genres/
   *  Composers the picked name's songs. */
  const shown = $derived.by(() => {
    const v = store.view;
    if (v === "songs") return visible;
    const flat = (list: ReturnType<typeof albums>) => list.flatMap((a) => a.tracks);
    if (v === "albums" || v === "compilations") {
      const list = v === "compilations" ? albumList.filter((a) => a.compilation) : albumList;
      const open = list.find((a) => a.key === store.openAlbum);
      return open ? open.tracks : flat(list);
    }
    const list = groups(searched, v === "artists" ? "artist" : v === "genres" ? "genre" : "composer");
    const picked = list.find((g) => g.name === store.groupPick[v]) ?? list[0];
    return picked ? flat(albums(picked.tracks)) : [];
  });
  const totals = $derived.by(() => {
    let ms = 0, size = 0;
    for (const t of visible) { ms += t.lengthMs; size += t.size; }
    return { ms, size };
  });
</script>

<svelte:window
  onkeydown={(e) => handleKey(e)}
  onmousedown={mousedown}
  oncontextmenu={(e) => {
    // The web view's own menu (Back, Reload…) isn't Chinotto's: only text fields keep theirs.
    if (!(e.target as HTMLElement).closest("input, textarea, [contenteditable]")) e.preventDefault();
  }}
/>

<div class="app">
  <Toolbar {visible} visibleMs={totals.ms} />

  <aside class="side">
    <SourceList />
    {#if store.showArtwork && (store.ipod || store.source.kind === "library" || store.source.kind === "scratch")}<ArtworkPane />{/if}
    <div class="side-tools">
      <button
        title={t("New library or open an iPod folder")}
        aria-label={t("New library or open an iPod folder")}
        onclick={(e) => {
          const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
          store.menu = {
            x: r.left,
            y: r.top - 112,
            items: [
              { label: t("New Library…"), action: () => (store.dialog = { kind: "library", id: null }) },
              { label: t("Open File…"), action: () => store.pickAndOpenSongs() },
              { label: t("Open iPod Folder…"), action: () => store.openFolder() },
              { label: t("Set Up iPod…"), action: () => store.pickDriveToSetUp() },
            ],
          };
        }}><Icon name="folder" /></button
      >
      {#if store.ipod && store.source.kind !== "library"}
        <button
          title={store.writable ? t("Add songs to the iPod… (also: drop files or folders on the window)") : t("Allow changes on the iPod's page to add songs")}
          aria-label={t("Add songs to the iPod")}
          disabled={!store.writable || !!store.busy}
          onclick={(e) => {
            const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
            store.menu = {
              x: r.left,
              y: r.top - 64,
              items: [
                { label: t("Add Files…"), action: () => store.pickAndAddFiles() },
                { label: t("Add Folder…"), action: () => store.pickAndAddFolder() },
              ],
            };
          }}><Icon name="add" /></button
        >
        <button
          title={store.writable ? t("New playlist") : t("Allow changes on the iPod's page to make playlists")}
          aria-label={t("New playlist")}
          disabled={!store.writable || !!store.busy}
          onclick={() => store.newPlaylist(false)}><Icon name="newlist" /></button
        >
      {:else if store.source.kind === "library" && !store.isDeviceLibrary(store.source.lib)}
        <button title={t("New playlist in this library")} aria-label={t("New playlist")} disabled={!!store.busy} onclick={() => store.newPlaylist(false)}
          ><Icon name="newlist" /></button
        >
      {/if}
        <button
          title={store.shuffle ? t("Shuffle is on") : t("Shuffle is off")}
          aria-label={t("Shuffle")}
          aria-pressed={store.shuffle}
          onclick={() => store.setShuffle(!store.shuffle)}><Icon name="shuffle" /></button
        >
        <button
          title={store.repeat === "off" ? t("Repeat is off") : store.repeat === "all" ? t("Repeat all") : t("Repeat one")}
          aria-label={store.repeat === "off" ? t("Repeat is off") : store.repeat === "all" ? t("Repeat all") : t("Repeat one")}
          aria-pressed={store.repeat !== "off"}
          onclick={() => store.cycleRepeat()}><Icon name={store.repeat === "one" ? "repeat-one" : "repeat"} /></button
        >
      <span class="spacer"></span>
      <button title={store.showBrowser ? t("Hide column browser") : t("Show column browser")} aria-pressed={store.showBrowser} onclick={() => (store.showBrowser = !store.showBrowser)}><Icon name="columns" /></button>
      <button title={store.showArtwork ? t("Hide artwork") : t("Show artwork")} aria-pressed={store.showArtwork} onclick={() => (store.showArtwork = !store.showArtwork)}><Icon name="artwork" /></button>
    </div>
  </aside>

  <main class="main">
    {#if store.error}
      <div class="error" role="alert">
        <span>{store.error}</span>
        <button onclick={() => (store.error = null)}>{t("Dismiss")}</button>
      </div>
    {/if}

    {#if store.source.kind === "disc"}
      <DiscPage id={store.source.id} />
    {:else if store.source.kind === "openmg"}
      <OpenMgPage path={store.source.path} />
    {:else if store.source.kind === "player" && store.source.path.startsWith("mtp:")}
      <MtpPage path={store.source.path} />
    {:else if store.source.kind === "player"}
      <PlayerPage path={store.source.path} />
    {:else if store.source.kind !== "library" && store.source.kind !== "scratch" && !store.ipod}
      <Welcome />
    {:else if store.source.kind === "device"}
      <DevicePage />
    {:else}
      {#if store.hasViews}<ViewTabs />{/if}
      {#if store.view === "albums"}
        <AlbumGrid albums={albumList} empty={store.search ? t("No albums match “{q}”.", { q: store.search }) : t("No albums")} />
      {:else if store.view === "compilations"}
        <AlbumGrid albums={albumList.filter((a) => a.compilation)} empty={t("No compilations (albums with the Compilation tag).")} />
      {:else if store.view === "artists" || store.view === "genres" || store.view === "composers"}
        <GroupView tracks={searched} by={store.view === "artists" ? "artist" : store.view === "genres" ? "genre" : "composer"} view={store.view} />
      {:else}
        {#if browsed}<ColumnBrowser columns={browsed.columns} />{/if}
        <SongList tracks={visible} />
      {/if}
    {/if}
  </main>

  {#if fileDrag}
    <div class="dropzone" aria-hidden="true">
      <p>
        {#if !(store.ipod?.writable && (store.source.kind === "playlist" || store.source.kind === "device"))}{t("Drop to play (in the Scratch list, not added to a library)")}
        {:else if store.writable}{store.editablePlaylist
            ? t("Drop to add to {name} and “{playlist}”", { name: store.ipod?.name ?? "", playlist: store.editablePlaylist.name })
            : t("Drop to add to {name}", { name: store.ipod?.name ?? "" })}
        {:else}{t("Turn on “Allow changes” on the iPod's page to add songs")}{/if}
      </p>
    </div>
  {/if}

  {#if store.songDrag}
    <div class="songdrag" style:left="{store.songDrag.x + 12}px" style:top="{store.songDrag.y + 8}px" aria-hidden="true">
      {tn(store.songDrag.dbids.length, "{n} song", "{n} songs")}
    </div>
  {/if}

  <Dialogs />
  <ContextMenu />

  <footer class="status">
    {#if (store.ipod && store.source.kind === "playlist") || store.source.kind === "library" || store.source.kind === "scratch"}
      {tn(visible.length, "{n} song", "{n} songs")}, {longDuration(totals.ms)}, {bytes(totals.size)}{#if store.selected.size > 1}; {t("{n} selected", { n: store.selected.size })}{/if}
    {/if}
  </footer>
</div>

<style>
  .app {
    display: grid;
    grid-template-areas: "toolbar toolbar" "side main" "side status";
    grid-template-columns: 220px 1fr;
    grid-template-rows: auto 1fr 22px;
    height: 100vh;
  }
  .side {
    grid-area: side;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--side);
    color: var(--side-ink);
    border-right: 1px solid var(--side-edge);
  }
  .side-tools {
    display: flex;
    gap: 2px;
    padding: 4px 6px;
    border-top: 1px solid var(--side-edge);
  }
  .side-tools button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 22px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--side-icon);
  }
  .side-tools .spacer {
    flex: 1;
  }
  .side-tools button:disabled {
    opacity: 0.35;
  }
  .side-tools button:hover:not(:disabled) {
    background: rgb(255 255 255 / 0.08);
    color: var(--side-ink);
  }
  .side-tools button[aria-pressed="true"] {
    color: var(--accent-bright);
  }
  .side-tools button:focus-visible {
    outline: 2px solid var(--accent-bright);
    outline-offset: -2px;
  }
  .songdrag {
    position: fixed;
    z-index: 70;
    padding: 3px 9px;
    border-radius: 10px;
    background: var(--accent);
    color: #fff;
    font-size: 12px;
    font-weight: 600;
    pointer-events: none;
    box-shadow: 0 4px 12px rgb(0 0 0 / 0.25);
  }
  .dropzone {
    position: fixed;
    inset: 0;
    z-index: 40;
    display: grid;
    place-items: center;
    background: rgb(122 59 20 / 0.18);
    border: 3px dashed var(--accent);
    pointer-events: none;
  }
  .dropzone p {
    padding: 10px 18px;
    border-radius: 9px;
    background: var(--label-bg);
    color: var(--label-ink);
    font-family: var(--rounded);
    font-weight: 600;
  }
  .main {
    grid-area: main;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .status {
    grid-area: status;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    color: var(--muted);
    background: var(--chrome);
    border-top: 1px solid var(--chrome-edge);
    font-variant-numeric: tabular-nums;
  }
  .error {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 14px;
    background: var(--warn-bg);
    color: var(--warn-ink);
    font-size: 12px;
    border-bottom: 1px solid var(--edge);
  }
  .error span {
    flex: 1;
  }
  .error button {
    border: 1px solid currentColor;
    border-radius: 5px;
    background: none;
    color: inherit;
    font: inherit;
    padding: 2px 10px;
  }
</style>
