<script lang="ts">
  import { t } from "./i18n";
  import { untrack } from "svelte";
  import { store } from "./store.svelte";
  import Modal from "./Modal.svelte";
  import { duration } from "./format";

  // Rip an audio CD: the user names it (no online lookup yet), picks a format,
  // and on Rip… a folder. Files go to ARTIST/ARTIST - YEAR - ALBUM/….
  let { disc: chosen }: { disc: string | null } = $props();

  let looking = $state(false);
  let discId = $state<string | null>(untrack(() => chosen));
  const disc = $derived(store.discs.find((d) => d.id === discId) ?? store.discs[0]);

  let artist = $state("");
  let album = $state("");
  let year = $state("");
  let genre = $state("");
  const FORMAT_KEY = "chinotto.ripFormat";
  let format = $state((() => { try { return localStorage.getItem(FORMAT_KEY) ?? "alac"; } catch { return "alac"; } })());
  let titles = $state<Record<number, string>>({});
  let skip = $state<Record<number, boolean>>({});

  $effect(() => {
    untrack(() => {
      if (!store.discs.length) {
        looking = true;
        void store.refreshDiscs().finally(() => (looking = false));
      }
    });
  });
  // A disc's volume name is often its title (CD-Text); "Audio CD" isn't.
  $effect(() => {
    const d = disc;
    if (d && !album && d.name && d.name !== "Audio CD") untrack(() => (album = d.name));
  });

  const close = () => (store.dialog = null);
  const chosenTracks = $derived((disc?.tracks ?? []).filter((x) => !skip[x.number]));
  function rip() {
    if (!disc || !chosenTracks.length) return;
    try { localStorage.setItem(FORMAT_KEY, format); } catch { /* not kept */ }
    void store.rip({
      disc: disc.id,
      format,
      artist,
      album,
      year: Number.parseInt(year, 10) || 0,
      genre,
      tracks: chosenTracks.map((x) => ({ number: x.number, title: titles[x.number] ?? "" })),
    });
  }
</script>

<Modal title={t("Rip CD")} onclose={close} width={620}>
  {#if !disc}
    <p class="muted">{looking ? t("Looking for an audio CD…") : t("No audio CD found. Put one in the drive, then try again. (On Linux, ripping needs cdparanoia.)")}</p>
  {:else}
    {#if store.discs.length > 1}
      <label class="row">
        {t("Disc")}
        <select bind:value={discId}>
          {#each store.discs as d (d.id)}<option value={d.id}>{d.name}</option>{/each}
        </select>
      </label>
    {/if}
    <div class="grid">
      <label>{t("Artist")}<input bind:value={artist} spellcheck="false" /></label>
      <label>{t("Album")}<input bind:value={album} spellcheck="false" /></label>
      <label>{t("Year")}<input bind:value={year} inputmode="numeric" maxlength="4" /></label>
      <label>{t("Genre")}<input bind:value={genre} spellcheck="false" /></label>
      <label>
        {t("Format")}
        <select bind:value={format}>
          <option value="alac">{t("Apple Lossless")}</option>
          <option value="flac">FLAC</option>
          <option value="aac256">{t("AAC 256 kbps (iTunes Plus)")}</option>
          <option value="mp3_320">{t("MP3 320 kbps")}</option>
          <option value="mp3_256">{t("MP3 256 kbps")}</option>
          <option value="mp3_192">{t("MP3 192 kbps")}</option>
          <option value="mp3_128">{t("MP3 128 kbps (smaller)")}</option>
        </select>
      </label>
    </div>
    <div class="tracks">
      {#each disc.tracks as x (x.number)}
        <div class="track">
          <input type="checkbox" checked={!skip[x.number]} onchange={(e) => (skip = { ...skip, [x.number]: !e.currentTarget.checked })} aria-label={t("Rip track {n}", { n: x.number })} />
          <span class="n">{x.number}</span>
          <input class="title" placeholder={t("Track {n}", { n: x.number })} value={titles[x.number] ?? ""} oninput={(e) => (titles = { ...titles, [x.number]: e.currentTarget.value })} spellcheck="false" />
          <span class="len">{duration(x.lengthMs)}</span>
        </div>
      {/each}
    </div>
    <p class="muted">{t("Songs are saved as Artist/Artist - Year - Album/Artist - Year - Album - 01 - Title, in a folder you choose next. They join a library only if that folder is inside one of its folders.")}</p>
  {/if}
  {#snippet buttons()}
    <button onclick={close}>{t("Cancel")}</button>
    <button class="primary" onclick={rip} disabled={!disc || !chosenTracks.length || !!store.busy}>{t("Rip…")}</button>
  {/snippet}
</Modal>

<style>
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 8px 14px; margin-bottom: 12px; }
  label { display: flex; flex-direction: column; gap: 3px; font-size: 12px; color: var(--muted); }
  label.row { flex-direction: row; align-items: center; gap: 8px; margin-bottom: 10px; }
  input, select { font: inherit; font-size: 13px; color: var(--ink); }
  .tracks { max-height: 300px; overflow: auto; border: 1px solid var(--edge); border-radius: 6px; }
  .track { display: flex; align-items: center; gap: 8px; padding: 3px 8px; }
  .track:nth-child(even) { background: var(--zebra); }
  .n { width: 2ch; text-align: right; color: var(--muted); font-variant-numeric: tabular-nums; }
  .title { flex: 1; min-width: 0; }
  .len { color: var(--muted); font-variant-numeric: tabular-nums; }
  .muted { color: var(--muted); font-size: 12px; line-height: 1.5; }
</style>
