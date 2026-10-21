<script lang="ts">
  import { t, tn } from "./i18n";
  import { untrack } from "svelte";
  import { store } from "./store.svelte";
  import Modal from "./Modal.svelte";
  import { imagePreview, pasteImage as pastedImagePath, pickImage } from "./api";
  import { bytes, count, date, duration } from "./format";
  import type { Track, TrackChanges } from "./types";

  type TextKey =
    | "title" | "artist" | "albumArtist" | "album" | "genre" | "composer" | "comment" | "grouping"
    | "sortTitle" | "sortArtist" | "sortAlbum" | "sortAlbumArtist" | "sortComposer";
  type NumKey = "year" | "trackNr" | "trackCount" | "discNr" | "discCount";
  const TEXT: TextKey[] = ["title", "artist", "albumArtist", "album", "genre", "composer", "comment", "grouping", "sortTitle", "sortArtist", "sortAlbum", "sortAlbumArtist", "sortComposer"];
  const NUM: NumKey[] = ["year", "trackNr", "trackCount", "discNr", "discCount"];
  const numOf = (t: Track, k: NumKey): number => t[k];

  // Snapshot of the selection when the window opened.
  const tracks: Track[] = untrack(() => store.selectedTracks);
  const ids = tracks.map((t) => t.dbid);
  const many = tracks.length > 1;
  const readOnly = untrack(() => !store.writable);

  /** The shared value of a field, or undefined when the songs differ. */
  function common<T>(get: (t: Track) => T): T | undefined {
    const first = get(tracks[0]);
    return tracks.every((t) => get(t) === first) ? first : undefined;
  }

  const initialText: Record<string, string | undefined> = {};
  for (const k of TEXT) initialText[k] = common((t) => t[k] ?? "");
  const initialNum: Record<string, string | undefined> = {};
  for (const k of NUM) {
    const v = common((t) => numOf(t, k));
    initialNum[k] = v === undefined ? undefined : v ? String(v) : "";
  }
  const initialRating = common((t) => t.rating);
  const initialCompilation = common((t) => t.compilation);

  let text = $state<Record<string, string>>(Object.fromEntries(TEXT.map((k) => [k, initialText[k] ?? ""])));
  let num = $state<Record<string, string>>(Object.fromEntries(NUM.map((k) => [k, initialNum[k] ?? ""])));
  let rating = $state(initialRating ?? 0);
  let compilation = $state(initialCompilation ?? false);
  let dirty = $state<Set<string>>(new Set());
  let tab = $state<"info" | "sorting" | "artwork" | "details">("info");
  /** Artwork change to apply on OK. */
  let artwork = $state<{ set: string; preview: string; size: string } | "remove" | null>(null);
  let artProblem = $state<string | null>(null);
  const covers = untrack(() => tracks.map((t) => store.coverUrl(t, 320)));
  const sameCover = covers.every((c) => c === covers[0]);

  async function useImage(path: string) {
    artProblem = null;
    try {
      const [preview, size] = (await imagePreview(path)).split("#");
      artwork = { set: path, preview, size: size.replace("x", " × ") };
    } catch (e) {
      artProblem = String(e);
    }
  }
  async function pasteImage() {
    artProblem = null;
    try {
      await useImage(await pastedImagePath());
    } catch (e) {
      artProblem = String(e);
    }
  }
  // ⌘V / Ctrl+V on the Artwork tab pastes an image (unless typing in a field).
  function pasteKey(e: KeyboardEvent) {
    if (tab !== "artwork" || readOnly || !(e.metaKey || e.ctrlKey) || e.key.toLowerCase() !== "v") return;
    if ((e.target as HTMLElement).closest("input, textarea, [contenteditable]")) return;
    e.preventDefault();
    void pasteImage();
  }
  async function chooseImage() {
    const p = await pickImage();
    if (p) await useImage(p);
  }
  // An image dropped on the window while this is open.
  $effect(() => {
    const dropped = store.infoImageDrop;
    if (dropped && !readOnly) {
      store.infoImageDrop = null;
      tab = "artwork";
      void useImage(dropped);
    }
  });
  let problem = $state<string | null>(null);
  let saving = $state(false);

  const touch = (k: string) => {
    if (!dirty.has(k)) dirty = new Set(dirty).add(k);
  };
  const mixed = (k: string, initial: Record<string, string | undefined>) => many && initial[k] === undefined && !dirty.has(k);

  function setRating(n: number) {
    if (readOnly) return;
    rating = rating === n ? n - 1 : n;
    touch("rating");
  }

  function changes(): TrackChanges | null {
    const out: Record<string, unknown> = {};
    for (const k of TEXT) if (dirty.has(k)) out[k] = text[k].trim() || null;
    for (const k of NUM) {
      if (!dirty.has(k)) continue;
      const v = num[k].trim();
      if (v && !/^\d{1,4}$/.test(v)) {
        problem = `${LABELS[k]} must be a whole number.`;
        return null;
      }
      out[k] = v ? Number(v) : null;
    }
    if (dirty.has("rating")) out.rating = rating;
    if (dirty.has("compilation")) out.compilation = compilation;
    return out as TrackChanges;
  }

  async function save() {
    if (readOnly) return close();
    problem = null;
    const c = changes();
    if (!c) return;
    saving = true;
    const ok = await store.saveInfo(ids, c, artwork && artwork !== "remove" ? { set: artwork.set } : artwork);
    saving = false;
    if (ok) close();
  }

  function close() {
    store.dialog = null;
  }

  function keydown(e: KeyboardEvent) {
    const inTextarea = (e.target as HTMLElement).tagName === "TEXTAREA";
    if (e.key === "Enter" && (!inTextarea || e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      void save();
    }
  }

  const LABELS: Record<string, string> = {
    title: t("Name"), artist: t("Artist"), albumArtist: t("Album Artist"), album: t("Album"), genre: t("Genre"), composer: t("Composer"),
    comment: t("Comments"), grouping: t("Grouping"), year: t("Year"), trackNr: t("Track"), trackCount: t("of"), discNr: t("Disc"), discCount: t("of"),
    sortTitle: t("Sort Name"), sortArtist: t("Sort Artist"), sortAlbum: t("Sort Album"), sortAlbumArtist: t("Sort Album Artist"), sortComposer: t("Sort Composer"),
  };
  const SORT_PAIRS: [TextKey, TextKey][] = [
    ["title", "sortTitle"], ["artist", "sortArtist"], ["albumArtist", "sortAlbumArtist"], ["album", "sortAlbum"], ["composer", "sortComposer"],
  ];

  const first = tracks[0];
  const totalMs = tracks.reduce((a, t) => a + t.lengthMs, 0);
  const totalSize = tracks.reduce((a, t) => a + t.size, 0);
  const cover = untrack(() => store.coverUrl(first, 128));
  const heading = many ? tn(tracks.length, "{n} song", "{n} songs") : first?.title || t("Untitled");
</script>

<svelte:window onkeydown={pasteKey} />

{#snippet field(k: TextKey, wide = true)}
  <label class="f" class:wide>
    <span>{LABELS[k]}</span>
    <input
      type="text"
      bind:value={text[k]}
      oninput={() => touch(k)}
      placeholder={mixed(k, initialText) ? t("Mixed") : ""}
      disabled={readOnly}
      spellcheck="false"
      autocomplete="off"
    />
  </label>
{/snippet}

{#snippet numField(k: NumKey)}
  <input
    class="num"
    type="text"
    inputmode="numeric"
    aria-label={k === "trackCount" ? t("Track count") : k === "discCount" ? t("Disc count") : LABELS[k]}
    bind:value={num[k]}
    oninput={() => touch(k)}
    placeholder={mixed(k, initialNum) ? "–" : ""}
    disabled={readOnly}
    autocomplete="off"
  />
{/snippet}

<Modal title={t("Get Info")} onclose={close} width={600}>
  <div class="info" role="presentation" onkeydown={keydown}>
    <header>
      <div class="thumb">{#if cover && !many}<img src={cover} alt="" />{/if}</div>
      <div class="what">
        <strong>{heading}</strong>
        {#if !many}<span>{[first.artist, first.album].filter(Boolean).join(" — ")}</span>{/if}
        <span class="muted">{duration(totalMs)}, {bytes(totalSize)}{#if !many}, {first.kind}{/if}</span>
      </div>
    </header>

    <div class="tabs" role="tablist">
      {#each [["info", t("Info")], ["sorting", t("Sorting")], ["artwork", t("Artwork")], ["details", t("Details")]] as [id, label] (id)}
        <button role="tab" aria-selected={tab === id} class:on={tab === id} onclick={() => (tab = id as typeof tab)}>{label}</button>
      {/each}
    </div>

    {#if tab === "info"}
      <div class="grid">
        {#if !many}{@render field("title")}{/if}
        {@render field("artist")}
        {@render field("albumArtist")}
        {@render field("album")}
        {@render field("genre")}
        {@render field("composer")}
        {@render field("grouping")}
        <div class="row">
          <label class="f"><span>{t("Year")}</span>{@render numField("year")}</label>
          <div class="f"><span>{t("Track")}</span>{@render numField("trackNr")}<em>{t("of")}</em>{@render numField("trackCount")}</div>
          <div class="f"><span>{t("Disc")}</span>{@render numField("discNr")}<em>{t("of")}</em>{@render numField("discCount")}</div>
        </div>
        <div class="row">
          <div class="f">
            <span>{t("Rating")}</span>
            <div class="stars" role="radiogroup" aria-label={t("Rating")}>
              {#each [1, 2, 3, 4, 5] as n (n)}
                <button
                  type="button"
                  role="radio"
                  aria-checked={rating === n}
                  aria-label={tn(n, "{n} star", "{n} stars")}
                  class:lit={n <= rating && (!many || initialRating !== undefined || dirty.has("rating"))}
                  disabled={readOnly}
                  onclick={() => setRating(n)}>★</button
                >
              {/each}
              {#if many && initialRating === undefined && !dirty.has("rating")}<em>{t("Mixed")}</em>{/if}
            </div>
          </div>
          <label class="check">
            <input
              type="checkbox"
              bind:checked={compilation}
              indeterminate={many && initialCompilation === undefined && !dirty.has("compilation")}
              onchange={() => touch("compilation")}
              disabled={readOnly}
            />
            {t("Part of a compilation")}
          </label>
        </div>
        <label class="f wide">
          <span>{t("Comments")}</span>
          <textarea rows="2" bind:value={text.comment} oninput={() => touch("comment")} placeholder={mixed("comment", initialText) ? t("Mixed") : ""} disabled={readOnly}></textarea>
        </label>
      </div>
    {:else if tab === "sorting"}
      <p class="hint">{store.source.kind === "library" ? t("Chinotto and iPods sort by these instead of the names when they're filled in (for example “Beatles” for “The Beatles”).") : t("The iPod sorts by these instead of the names when they're filled in (for example “Beatles” for “The Beatles”).")}</p>
      <div class="sortgrid">
        {#each SORT_PAIRS as [name, sort] (sort)}
          {#if !(many && name === "title")}
            <span class="sname">{LABELS[name]}</span>
            <span class="sval">{mixed(name, initialText) ? t("Mixed") : text[name] || "—"}</span>
            <input
              type="text"
              aria-label={LABELS[sort]}
              bind:value={text[sort]}
              oninput={() => touch(sort)}
              placeholder={mixed(sort, initialText) ? t("Mixed") : ""}
              disabled={readOnly}
              spellcheck="false"
              autocomplete="off"
            />
          {/if}
        {/each}
      </div>
    {:else if tab === "artwork"}
      <div class="artwork">
        <div class="big" role="img" aria-label={t("Cover")} oncontextmenu={(e) => {
          e.preventDefault();
          const t0 = tracks[0];
          if (!t0?.art || artwork) return;
          store.menu = { x: e.clientX, y: e.clientY, items: [{ label: t("Save Cover As…"), action: () => store.saveCover(t0) }] };
        }}>
          {#if artwork === "remove"}
            <p>{t("No artwork")}</p>
          {:else if artwork}
            <img src={artwork.preview} alt={t("New cover")} />
          {:else if sameCover && covers[0]}
            <img src={covers[0]} alt={t("Current cover")} />
          {:else}
            <p>{many && !sameCover ? t("Mixed") : t("No artwork")}</p>
          {/if}
        </div>
        <div class="side">
          {#if artwork && artwork !== "remove"}
            <p class="hint">{t("This new cover is {size}.", { size: artwork.size })} {store.source.kind === "library" ? t("It's saved into the song files when you press OK; images larger than 600 px on the shorter side are shrunk to that.") : t("It will be automatically resized such that its shortest side measures 600px. The change applies once the image is saved to your iPod.")}</p>
          {:else if artwork === "remove"}
            <p class="hint">{t("The cover is taken off when you press OK.")}</p>
          {:else}
            <p class="hint">{many ? t("Choose an image, paste one, or drop one on this window to use it for all {n} songs.", { n: tracks.length }) : t("Choose an image, paste one, or drop one on this window.")}</p>
          {/if}
          {#if !readOnly}
            <div class="artbuttons">
              <button onclick={chooseImage}>{t("Choose Image…")}</button>
              <button onclick={pasteImage} title={t("Use the image on the clipboard (⌘V)")}>{t("Paste")}</button>
              <button onclick={() => (artwork = "remove")} disabled={artwork === "remove" || (!artwork && covers.every((c) => !c))}>{t("Remove")}</button>
              {#if artwork}<button onclick={() => (artwork = null)}>{t("Undo")}</button>{/if}
            </div>
          {/if}
          {#if artProblem}<p class="problem" role="alert">{artProblem}</p>{/if}
        </div>
      </div>
    {:else}
      <dl class="details">
        {#if many}
          <dt>{t("Songs")}</dt><dd>{count(tracks.length)}</dd>
          <dt>{t("Total time")}</dt><dd>{duration(totalMs)}</dd>
          <dt>{t("Total size")}</dt><dd>{bytes(totalSize)}</dd>
        {:else}
          <dt>{t("Kind")}</dt><dd>{first.kind}</dd>
          <dt>{t("Size")}</dt><dd>{bytes(first.size)}</dd>
          <dt>{t("Bit rate")}</dt><dd>{first.bitrate ? `${first.bitrate} kbps` : "—"}</dd>
          <dt>{t("Sample rate")}</dt><dd>{first.sampleRate ? `${(first.sampleRate / 1000).toFixed(1)} kHz` : "—"}</dd>
          <dt>{t("Plays")}</dt><dd>{count(first.playCount)}</dd>
          <dt>{t("Last played")}</dt><dd>{date(first.played) || "—"}</dd>
          <dt>{t("Date added")}</dt><dd>{date(first.added) || "—"}</dd>
          <dt>{t("Modified")}</dt><dd>{date(first.modified) || "—"}</dd>
          <dt>{t("Where")}</dt><dd class="loc">{first.location ?? "—"}</dd>
        {/if}
      </dl>
    {/if}
    {#if problem}<p class="problem" role="alert">{problem}</p>{/if}
  </div>

  {#snippet buttons()}
    {#if readOnly}
      <span class="left">{t("Read-only: turn on “Allow changes” on the iPod's page to edit.")}</span>
      <button class="primary" onclick={close}>{t("Done")}</button>
    {:else}
      <button onclick={close} disabled={saving}>{t("Cancel")}</button>
      <button class="primary" onclick={save} disabled={saving}>{saving ? t("Saving…") : t("OK")}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  header {
    display: flex;
    gap: 12px;
    align-items: center;
    margin-bottom: 12px;
  }
  .thumb {
    flex: none;
    width: 56px;
    height: 56px;
    border-radius: 4px;
    background: var(--zebra);
    box-shadow: inset 0 0 0 1px var(--edge);
    overflow: hidden;
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .what {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .what strong {
    font-size: 14px;
  }
  .what span {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .muted,
  .hint {
    color: var(--muted);
    font-size: 12px;
  }
  .hint {
    margin: 0 0 10px;
    line-height: 1.45;
  }
  .tabs {
    display: flex;
    gap: 2px;
    margin-bottom: 12px;
    border-bottom: 1px solid var(--edge);
  }
  .tabs button {
    padding: 5px 12px;
    border: 0;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--muted);
    font: inherit;
  }
  .tabs button.on {
    color: var(--ink);
    border-bottom-color: var(--accent);
  }
  .grid {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 7px 22px;
    align-items: center;
  }
  .f {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .f > span {
    flex: none;
    width: 92px;
    text-align: right;
    color: var(--muted);
    font-size: 12px;
  }
  .row .f > span {
    width: auto;
  }
  .row .f:first-child > span {
    width: 92px;
  }
  .f em {
    font-style: normal;
    color: var(--muted);
    font-size: 12px;
  }
  .wide input,
  .wide textarea {
    flex: 1;
  }
  input[type="text"],
  textarea {
    min-width: 0;
    padding: 4px 7px;
    border: 1px solid var(--field-edge);
    border-radius: 5px;
    background: var(--field);
    color: var(--ink);
    font: inherit;
    user-select: text;
    -webkit-user-select: text;
  }
  textarea {
    resize: vertical;
  }
  input.num {
    width: 52px;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  input:focus-visible,
  textarea:focus-visible,
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  input:disabled,
  textarea:disabled {
    opacity: 0.75;
  }
  .stars {
    display: flex;
    align-items: center;
  }
  .stars button {
    padding: 0 2px;
    border: 0;
    background: none;
    color: var(--edge);
    font-size: 17px;
    line-height: 1;
  }
  .stars button.lit {
    color: var(--star);
  }
  .stars em {
    margin-left: 6px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }
  .sortgrid {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr) minmax(0, 1fr);
    gap: 7px 10px;
    align-items: center;
  }
  .sname {
    color: var(--muted);
    font-size: 12px;
    text-align: right;
  }
  .sval {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .artwork {
    display: flex;
    gap: 16px;
    align-items: flex-start;
  }
  .big {
    flex: none;
    display: grid;
    place-items: center;
    width: 200px;
    height: 200px;
    border-radius: 6px;
    background: var(--zebra);
    box-shadow: inset 0 0 0 1px var(--edge);
    overflow: hidden;
  }
  .big img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .big p {
    margin: 0;
    color: var(--muted);
    font-size: 12px;
  }
  .artbuttons {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .artbuttons button {
    padding: 4px 12px;
    border: 1px solid var(--field-edge);
    border-radius: 6px;
    background: var(--field);
    color: var(--ink);
    font: inherit;
  }
  .details {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 16px;
    margin: 0;
    font-size: 13px;
  }
  .details dt {
    color: var(--muted);
    text-align: right;
  }
  .details dd {
    margin: 0;
    font-variant-numeric: tabular-nums;
  }
  .loc {
    word-break: break-all;
  }
  .problem {
    margin: 10px 0 0;
    color: #a3261a;
    font-size: 12px;
  }
</style>
