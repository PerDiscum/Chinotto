<script lang="ts">
  import { t, tn } from "./i18n";
  // Albums as a grid of covers. Clicking one opens its songs in a band under
  // its row (as in iTunes 11); clicking it again, the ×, or Escape closes it.
  import { store } from "./store.svelte";
  import { count, longDuration } from "./format";
  import type { Album } from "./library";
  import Cover from "./Cover.svelte";
  import AlbumTracks from "./AlbumTracks.svelte";

  let { albums, empty = t("No albums") }: { albums: Album[]; empty?: string } = $props();

  const TILE = 150;
  const GAP = 22;
  let width = $state(800);
  let scroller: HTMLDivElement | undefined = $state();
  const cols = $derived(Math.max(1, Math.floor((width - 2 * GAP + GAP) / (TILE + GAP))));
  const rows = $derived.by(() => {
    const out: Album[][] = [];
    for (let i = 0; i < albums.length; i += cols) out.push(albums.slice(i, i + cols));
    return out;
  });
  const open = $derived(albums.find((a) => a.key === store.openAlbum) ?? null);
  const openCol = $derived(open ? albums.indexOf(open) % cols : 0);

  function toggle(a: Album, e: MouseEvent) {
    store.openAlbum = store.openAlbum === a.key ? null : a.key;
    if (store.openAlbum) {
      const tile = e.currentTarget as HTMLElement;
      queueMicrotask(() => {
        const panel = scroller?.querySelector<HTMLElement>(".panel");
        panel?.scrollIntoView({ block: "nearest", behavior: "smooth" });
        if (!panel) tile.focus();
      });
    }
  }

  function playAlbum(a: Album, shuffle: boolean) {
    if (shuffle !== store.shuffle) store.setShuffle(shuffle);
    void store.playList(a.tracks, shuffle ? Math.floor(Math.random() * a.tracks.length) : 0);
  }

  function key(e: KeyboardEvent) {
    if (e.key === "Escape" && store.openAlbum) {
      e.preventDefault();
      store.openAlbum = null;
    }
  }
  const total = (a: Album) => a.tracks.reduce((n, t) => n + t.lengthMs, 0);
</script>

<div class="grid" bind:this={scroller} bind:clientWidth={width} onkeydown={key} role="presentation" style:--tile="{TILE}px" style:--gap="{GAP}px">
  {#if !albums.length}
    <p class="empty">{empty}</p>
  {/if}
  {#each rows as row (row[0].key)}
    <div class="row">
      {#each row as a (a.key)}
        <button class="album" class:open={store.openAlbum === a.key} onclick={(e) => toggle(a, e)} aria-expanded={store.openAlbum === a.key}>
          <Cover track={a.coverTrack} px={300} alt="" />
          <span class="t">{a.title}</span>
          <span class="a">{a.artist}</span>
        </button>
      {/each}
    </div>
    {#if open && row.includes(open)}
      <section class="panel" aria-label={t("{album} by {artist}", { album: open.title, artist: open.artist })} style:--pointer="{GAP + openCol * (TILE + GAP) + TILE / 2}px">
        <button class="close" aria-label={t("Close")} onclick={() => (store.openAlbum = null)}>×</button>
        <div class="info">
          <header>
            <h2>{open.title}</h2>
            <p class="by">{open.artist}{open.year ? ` (${open.year})` : ""}</p>
            <div class="actions">
              <button onclick={() => playAlbum(open, false)} title={t("Play the album")}>{t("▶ Play")}</button>
              <button onclick={() => playAlbum(open, true)} title={t("Shuffle the album")}>{t("⤮ Shuffle")}</button>
              <span class="meta">{tn(open.tracks.length, "{n} song", "{n} songs")}, {longDuration(total(open))}</span>
            </div>
          </header>
          <AlbumTracks tracks={open.tracks} />
        </div>
        <div class="big"><Cover track={open.coverTrack} px={600} alt={t("Cover of {name}", { name: open.title })} /></div>
      </section>
    {/if}
  {/each}
</div>

<style>
  .grid {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 22px 0 30px;
    background: var(--bg);
  }
  .row {
    display: grid;
    grid-template-columns: repeat(auto-fill, var(--tile));
    gap: var(--gap);
    padding: 0 var(--gap) 22px;
    content-visibility: auto;
    contain-intrinsic-size: auto 210px;
  }
  .album {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    padding: 0;
    border: 0;
    background: none;
    color: var(--ink);
    font: inherit;
    text-align: left;
    cursor: default;
  }
  .album :global(.cover) {
    margin-bottom: 6px;
    transition: transform 0.12s ease;
  }
  .album:hover :global(.cover) {
    transform: translateY(-1px);
  }
  .album.open :global(.cover) {
    outline: 3px solid var(--accent);
    outline-offset: 2px;
  }
  .album:focus-visible {
    outline: none;
  }
  .album:focus-visible :global(.cover) {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .t,
  .a {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12.5px;
  }
  .t {
    font-weight: 600;
  }
  .a {
    color: var(--muted);
  }
  .panel {
    position: relative;
    display: flex;
    gap: 28px;
    margin: 0 0 24px;
    padding: 22px 28px 26px 44px;
    background: var(--header);
    border-top: 1px solid var(--edge);
    border-bottom: 1px solid var(--edge);
  }
  /* The little pointer up to the open album. */
  .panel::before {
    content: "";
    position: absolute;
    top: -9px;
    left: calc(var(--pointer) - 9px);
    width: 16px;
    height: 16px;
    background: var(--header);
    border-left: 1px solid var(--edge);
    border-top: 1px solid var(--edge);
    transform: rotate(45deg);
  }
  .close {
    position: absolute;
    top: 10px;
    left: 12px;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    background: var(--muted);
    color: var(--bg);
    font-size: 15px;
    line-height: 22px;
  }
  .info {
    flex: 1;
    min-width: 0;
  }
  header {
    margin-bottom: 12px;
  }
  h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
  }
  .by {
    margin: 2px 0 8px;
    color: var(--accent);
    font-size: 14px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .actions button {
    padding: 2px 10px;
    border: 1px solid var(--edge);
    border-radius: 999px;
    background: var(--bg);
    color: var(--ink);
    font: inherit;
    font-size: 12px;
  }
  .meta {
    margin-left: 6px;
    color: var(--muted);
    font-size: 12px;
  }
  .big {
    flex: none;
    width: 220px;
    align-self: flex-start;
  }
  .empty {
    margin: 60px 0;
    text-align: center;
    color: var(--muted);
  }
  @media (prefers-reduced-motion: reduce) {
    .album :global(.cover) {
      transition: none;
    }
  }
</style>
