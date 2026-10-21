<script lang="ts">
  import { t } from "./i18n";
  // Artists, Genres or Composers: names on the left; the picked name's albums,
  // each with its cover and songs, on the right.
  import { store, type MusicView } from "./store.svelte";
  import { albums as toAlbums, groups } from "./library";
  import { count } from "./format";
  import type { Track } from "./types";
  import Cover from "./Cover.svelte";
  import AlbumTracks from "./AlbumTracks.svelte";

  let { tracks, by, view }: { tracks: Track[]; by: "artist" | "genre" | "composer"; view: MusicView } = $props();

  const list = $derived(groups(tracks, by));
  const picked = $derived(list.find((g) => g.name === store.groupPick[view]) ?? list[0] ?? null);
  const shown = $derived(picked ? toAlbums(picked.tracks) : []);
  let right: HTMLDivElement | undefined = $state();

  function pick(name: string) {
    store.groupPick = { ...store.groupPick, [view]: name };
    right?.scrollTo({ top: 0 });
  }
  function key(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const i = picked ? list.indexOf(picked) : -1;
    const n = Math.max(0, Math.min(list.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)));
    if (list[n]) pick(list[n].name);
    queueMicrotask(() => (e.currentTarget as HTMLElement | null)?.querySelector(".on")?.scrollIntoView({ block: "nearest" }));
  }
</script>

<div class="split">
  <div class="names" role="listbox" tabindex="0" aria-label={by === "composer" ? t("Composers") : by === "genre" ? t("Genres") : t("Artists")} onkeydown={key}>
    {#each list as g (g.name)}
      <button role="option" aria-selected={picked === g} class:on={picked === g} onclick={() => pick(g.name)}>
        <span>{g.name}</span><span class="n">{count(g.tracks.length)}</span>
      </button>
    {:else}
      <p class="empty">{by === "composer" ? t("No composers (no songs have a composer).") : by === "genre" ? t("No genres.") : t("No artists.")}</p>
    {/each}
  </div>
  <div class="albums" bind:this={right}>
    {#if picked}
      <h1>{picked.name}</h1>
      {#each shown as a (a.key)}
        <section class="album">
          <div class="cover"><Cover track={a.coverTrack} px={300} alt={t("Cover of {name}", { name: a.title })} /></div>
          <div class="body">
            <h2>{a.title}</h2>
            <p class="meta">{by !== "artist" ? `${a.artist}${a.year ? " · " : ""}` : ""}{a.year || ""}</p>
            <AlbumTracks tracks={a.tracks} />
          </div>
        </section>
      {/each}
    {/if}
  </div>
</div>

<style>
  .split {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 230px minmax(0, 1fr);
    background: var(--bg);
  }
  .names {
    overflow-y: auto;
    border-right: 1px solid var(--edge);
    padding: 6px 0;
    outline: none;
  }
  .names button {
    display: flex;
    gap: 8px;
    width: 100%;
    padding: 4px 12px;
    border: 0;
    background: none;
    color: var(--ink);
    font: inherit;
    font-size: 12.5px;
    text-align: left;
    cursor: default;
  }
  .names button span:first-child {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .names .n {
    color: var(--muted);
    font-size: 11px;
  }
  .names button.on {
    background: var(--sel-quiet);
  }
  .names:focus-within button.on {
    background: var(--sel);
    color: var(--sel-ink);
  }
  .names:focus-within button.on .n {
    color: inherit;
  }
  .albums {
    overflow-y: auto;
    padding: 18px 28px 40px;
  }
  h1 {
    margin: 0 0 18px;
    font-size: 22px;
    font-weight: 600;
  }
  .album {
    display: flex;
    gap: 22px;
    padding: 0 0 22px;
    margin-bottom: 22px;
    border-bottom: 1px solid var(--edge);
    content-visibility: auto;
    contain-intrinsic-size: auto 220px;
  }
  .album:last-child {
    border-bottom: 0;
  }
  .cover {
    flex: none;
    width: 150px;
  }
  .body {
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .meta {
    margin: 2px 0 10px;
    color: var(--muted);
    font-size: 12px;
  }
  .empty {
    margin: 16px 12px;
    color: var(--muted);
    font-size: 12px;
  }
</style>
