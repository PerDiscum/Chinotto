<script lang="ts">
  import { t } from "./i18n";
  import { store } from "./store.svelte";
  import { duration } from "./format";
  import { cdMusicInfo, imagePreview, pickImage } from "./api";
  import { isMac } from "./store.svelte";
  import happyDisc from "../assets/disc-happy.webp";

  // An audio CD's page: name it (no online lookup yet), choose a format and
  // rip. Files go to ARTIST/ARTIST - YEAR - ALBUM/… in a folder chosen on Rip.
  let { id }: { id: string } = $props();
  const disc = $derived(store.discs.find((d) => d.id === id));
  // Made in refreshDiscs for every disc found.
  const draft = $derived(store.discDrafts[id]);
  const chosen = $derived((disc?.tracks ?? []).filter((x) => !draft?.skip[x.number]));
  const total = $derived(chosen.reduce((a, x) => a + x.lengthMs, 0));
  let problem = $state<string | null>(null);

  async function chooseCover() {
    const path = await pickImage();
    if (!path || !draft) return;
    try {
      draft.preview = (await imagePreview(path)).split("#")[0];
      draft.cover = path;
      problem = null;
    } catch (e) {
      problem = String(e);
    }
  }

  let asking = $state(false);
  /** The names Music (iTunes) found for this CD with Gracenote. */
  async function fromMusic() {
    if (!draft) return;
    asking = true;
    problem = null;
    try {
      const i = await cdMusicInfo();
      if (!i.titles.length) {
        problem = t("Music doesn't show this CD. Open Music, let it look the CD up, then try again.");
        return;
      }
      draft.artist = i.artist;
      draft.album = i.album;
      if (i.year) draft.year = String(i.year);
      draft.genre = i.genre;
      for (const x of i.titles) draft.titles[x.number] = x.title;
    } catch (e) {
      problem = String(e);
    } finally {
      asking = false;
    }
  }

  function rip() {
    if (!disc || !draft || !chosen.length) return;
    try {
      localStorage.setItem("chinotto.ripFormat", draft.format);
    } catch {
      /* not kept */
    }
    void store.rip({
      disc: disc.id,
      format: draft.format,
      artist: draft.artist,
      album: draft.album,
      year: Number.parseInt(draft.year, 10) || 0,
      genre: draft.genre,
      cover: draft.cover,
      tracks: chosen.map((x) => ({ number: x.number, title: draft.titles[x.number] ?? "" })),
    });
  }
</script>

{#if disc && draft}
  <div class="page">
    <div class="top">
      <div class="head">
        <button class="cover" onclick={chooseCover} title={t("Choose a cover for the album")} aria-label={t("Choose a cover for the album")}>
          {#if draft.preview}<img src={draft.preview} alt="" />{:else}<span>{t("Add Cover")}</span>{/if}
        </button>
        <div class="names">
          <input class="artist" bind:value={draft.artist} placeholder={t("Artist")} aria-label={t("Artist")} spellcheck="false" />
          <input class="album" bind:value={draft.album} placeholder={t("Album")} aria-label={t("Album")} spellcheck="false" />
          <div class="line">
            <input class="year" bind:value={draft.year} placeholder={t("Year")} aria-label={t("Year")} inputmode="numeric" maxlength="4" />
            <input class="genre" bind:value={draft.genre} placeholder={t("Genre")} aria-label={t("Genre")} spellcheck="false" />
          </div>
        </div>
      </div>
      <img class="mascot" src={happyDisc} alt="" />
    </div>

    <div class="controls">
      <select bind:value={draft.format} aria-label={t("Format")}>
        <option value="alac">{t("Apple Lossless")}</option>
        <option value="flac">FLAC</option>
        <option value="aac256">{t("AAC 256 kbps (iTunes Plus)")}</option>
        <option value="mp3_320">{t("MP3 320 kbps")}</option>
        <option value="mp3_256">{t("MP3 256 kbps")}</option>
        <option value="mp3_192">{t("MP3 192 kbps")}</option>
        <option value="mp3_128">{t("MP3 128 kbps (smaller)")}</option>
      </select>
      {#if isMac}<button onclick={fromMusic} disabled={asking} title={t("The names Music found for this CD (Music looks CDs up with Gracenote)")}>{asking ? t("Asking Music…") : t("Get Names from Music")}</button>{/if}
      <button disabled title={t("Coming soon")}>{t("Fetch from MusicBrainz")}</button>
      <button class="primary" onclick={rip} disabled={!chosen.length || !!store.busy}>{t("Rip…")}</button>
    </div>
    {#if problem}<p class="problem" role="alert">{problem}</p>{/if}

    <div class="tracks" role="table" aria-label={t("Songs")}>
      {#each disc.tracks as x (x.number)}
        <div class="track" role="row" class:off={draft.skip[x.number]}>
          <input type="checkbox" checked={!draft.skip[x.number]} onchange={(e) => (draft.skip[x.number] = !e.currentTarget.checked)} aria-label={t("Rip track {n}", { n: x.number })} />
          <span class="n">{x.number}</span>
          <input class="title" placeholder={t("Track {n}", { n: x.number })} bind:value={draft.titles[x.number]} spellcheck="false" />
          <span class="len">{duration(x.lengthMs)}</span>
        </div>
      {/each}
    </div>
    <p class="note">
      {t("{n} songs, {time}.", { n: chosen.length, time: duration(total) })}
      {t("Songs are saved as Artist/Artist - Year - Album/Artist - Year - Album - 01 - Title, in a folder you choose next. They join a library only if that folder is inside one of its folders.")}
    </p>
  </div>
{/if}

<style>
  .page { position: relative; flex: 1; overflow: auto; padding: 32px 40px; background: var(--bg); }
  .top { padding-right: 230px; }
  .head { display: flex; align-items: flex-end; gap: 18px; min-width: 0; }
  .cover { flex: none; width: 160px; height: 160px; padding: 0; border: 1px dashed var(--edge); border-radius: 12px; background: var(--zebra); color: var(--muted); font: inherit; font-size: 12px; overflow: hidden; cursor: pointer; }
  .cover img { width: 100%; height: 100%; object-fit: cover; display: block; }
  .names { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .names input { border: 1px solid transparent; border-radius: 5px; padding: 1px 4px; margin-left: -5px; background: none; color: var(--ink); font: inherit; min-width: 0; }
  .names input:hover, .names input:focus { border-color: var(--edge); background: var(--bg); outline: none; }
  .names input::placeholder { color: var(--muted); opacity: 0.7; }
  .artist { font-size: 22px; font-weight: 700; }
  .album { font-size: 22px; color: var(--muted) !important; }
  .line { display: flex; gap: 8px; }
  .year { width: 5ch; font-size: 20px; color: var(--muted) !important; }
  .genre { font-size: 14px; color: var(--muted) !important; align-self: center; }
  /* Out of the flow, so it doesn't push the controls down. */
  .mascot { position: absolute; top: 28px; right: 40px; width: 200px; height: auto; pointer-events: none; }
  .controls { display: flex; flex-wrap: wrap; gap: 8px; margin: 14px 0 10px; }
  .controls select, .controls button { font: inherit; font-size: 13px; padding: 4px 10px; border: 1px solid var(--edge); border-radius: 6px; background: var(--zebra); color: var(--ink); }
  .controls button.primary { background: var(--accent); border-color: var(--accent); color: #fff; }
  .controls button:disabled { opacity: 0.5; }
  .tracks { max-width: 760px; border: 1px solid var(--edge); border-radius: 8px; overflow: hidden; }
  .track { display: flex; align-items: center; gap: 10px; padding: 3px 10px; }
  .track:nth-child(even) { background: var(--zebra); }
  .track.off .title, .track.off .len { opacity: 0.45; }
  .n { width: 2.5ch; text-align: right; color: var(--muted); font-variant-numeric: tabular-nums; }
  .title { flex: 1; min-width: 0; border: 1px solid transparent; border-radius: 4px; padding: 2px 4px; background: none; color: var(--ink); font: inherit; }
  .title:hover, .title:focus { border-color: var(--edge); background: var(--bg); outline: none; }
  .len { color: var(--muted); font-variant-numeric: tabular-nums; }
  .note { max-width: 760px; color: var(--muted); font-size: 12px; line-height: 1.5; }
  .problem { color: #b3261e; font-size: 12px; }
  @media (max-width: 760px) { .mascot { display: none; } .top { padding-right: 0; } }
</style>
