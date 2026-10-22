<script lang="ts">
  import { t, tn } from "./i18n";
  import { store } from "./store.svelte";
  import { bytes, longDuration } from "./format";
  import { deviceImage } from "./deviceImages";
  import ConversionSettings from "./ConversionSettings.svelte";

  // The page of an open MTP device.
  let { path }: { path: string } = $props();

  const d = $derived(store.devices.find((x) => x.path === path));
  const info = $derived(store.mtpInfos[path]);
  const view = $derived(info ? store.libViews[info.library]?.view : undefined);
  const stats = $derived.by(() => {
    let audio = 0, ms = 0;
    for (const t of view?.tracks ?? []) {
      audio += t.size;
      ms += t.lengthMs;
    }
    const cap = info?.capacity ?? 0;
    const free = info?.free ?? 0;
    return { audio, other: Math.max(0, cap - free - audio), free, cap, ms, songs: view?.tracks.length ?? 0 };
  });
  const segs = $derived(
    [
      { label: t("Audio"), value: stats.audio, cls: "audio" },
      { label: t("Other"), value: stats.other, cls: "other" },
      { label: t("Free"), value: stats.free, cls: "free" },
    ].filter((s) => s.value > 0 || s.cls === "free"),
  );
  const pct = (v: number) => (stats.cap ? (v / stats.cap) * 100 : 0);
  const playlists = $derived(view?.playlists.filter((p) => !p.master).length ?? 0);
  const st = $derived(store.playerStates[path]);
  const NAMES: Record<string, string> = { mp3: "MP3", aac: "AAC", flac: "FLAC", wma: "WMA", wav: "WAV", ogg: "Ogg Vorbis" };
  const plays = $derived((st?.plays ?? []).map((p) => NAMES[p] ?? p.toUpperCase()).join(", "));
</script>

{#if d && info}
  <div class="page">
    <div class="top">
      <section class="summary">
        <h1>{info.name}</h1>
        <dl>
          <dt>{t("Model")}</dt><dd>{[info.manufacturer, info.model].filter(Boolean).join(" ") || d.model || t("MTP device")}</dd>
          {#if stats.cap}<dt>{t("Capacity")}</dt><dd>{bytes(stats.cap)}</dd>{/if}
          {#if info.serial}<dt>{t("Serial number")}</dt><dd>{info.serial}</dd>{/if}
          {#if info.battery != null}<dt>{t("Battery")}</dt><dd>{info.battery}%</dd>{/if}
          <dt>{t("Contents")}</dt><dd>{tn(stats.songs, "{n} song", "{n} songs")}, {longDuration(stats.ms)}, {tn(playlists, "{n} playlist", "{n} playlists")}</dd>
          {#if plays}<dt>{t("Plays")}</dt><dd>{plays}</dd>{/if}
          <dt>{t("Connection")}</dt><dd>MTP</dd>
        </dl>
        <div class="actions">
          <button onclick={() => store.closeMtp(path)} disabled={!!store.busy}>{t("Let Other Programs Use It")}</button>
          {#if st?.writable}<button onclick={() => store.pickAndCopyToPlayer(path)} disabled={!!store.busy}>{t("Add Songs…")}</button>{/if}
        </div>
      </section>
      <div class="picture">
        <span class="device"><img src={deviceImage(store.iconFor(d))} alt={d.model ?? ""} /></span>
      </div>
    </div>
    <section class="summary">
      {#if st}
        <div class="changes">
          <label class="switch">
            <input type="checkbox" role="switch" checked={st.writable} onchange={(e) => store.setPlayerAllow(path, e.currentTarget.checked)} disabled={!!store.busy} />
            <span class="track" aria-hidden="true"><span class="knob"></span></span>
            {t("Allow changes to this device.")}
          </label>
          <p class="note">
            {#if st.writable}
              {t("Chinotto can copy songs to this device over MTP (drag them onto it in the sidebar, or use Add Songs…) and delete them from it. Its playlists can't be changed yet.")}
            {:else}
              {t("Chinotto only reads this device. Turn this on to copy songs to it and delete them. The setting is remembered for this device only.")}
            {/if}
          </p>
          <ConversionSettings conversion={st.conversion} plays={st.plays} device={t("player")} disabled={!!store.busy} onchange={(c) => store.setPlayerConversion(path, c)} />
        </div>
      {/if}
      <p class="note">
        {t("Songs on it play after they're copied to the computer. While Chinotto has it open, other programs can't use it.")}
      </p>
    </section>
    {#if stats.cap}
      <section class="capacity" aria-label={t("Capacity")}>
        <div class="bar" role="img" aria-label={segs.map((s) => `${s.label} ${bytes(s.value)}`).join(", ")}>
          {#each segs as s (s.cls)}<span class={s.cls} style:width="{pct(s.value)}%"></span>{/each}
        </div>
        <ul class="legend">
          {#each segs as s (s.cls)}<li><span class="swatch {s.cls}"></span>{s.label}<span class="v">{bytes(s.value)}</span></li>{/each}
        </ul>
      </section>
    {/if}
  </div>
{/if}

<style>
  .page {
    flex: 1;
    overflow: auto;
    padding: 32px 40px;
    background: var(--bg);
  }
  .summary {
    max-width: 640px;
  }
  .top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 24px;
    max-width: 820px;
  }
  .picture {
    position: relative;
    flex: none;
    max-width: 40%;
  }
  .device {
    display: block;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: none;
    cursor: pointer;
  }
  .device:disabled {
    cursor: default;
  }
  .device:hover:not(:disabled) img,
  .device:focus-visible img {
    filter: drop-shadow(0 0 6px rgb(0 0 0 / 0.25));
  }
  .device:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .device img {
    display: block;
    width: auto;
    height: 180px;
    max-width: 100%;
    object-fit: contain;
  }
  /* Too narrow for the details and the picture side by side: hide the picture. */
  .top {
    container-type: inline-size;
  }
  .top > .summary {
    min-width: 0;
  }
  @container (max-width: 600px) {
    .picture {
      display: none;
    }
  }
  h1 {
    margin: 0 0 18px;
    font-size: 26px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 18px;
    margin: 0;
    font-size: 13px;
  }
  dt {
    color: var(--muted);
    text-align: right;
  }
  dd {
    margin: 0;
    font-variant-numeric: tabular-nums;
  }
  .path {
    word-break: break-all;
  }
  .muted {
    color: var(--muted);
  }
  .actions {
    margin: 20px 0 0;
  }
  button {
    padding: 5px 16px;
    border: 1px solid var(--field-edge);
    border-radius: 6px;
    background: var(--field);
    color: var(--ink);
    font: inherit;
  }
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .changes {
    margin-top: 22px;
    padding-top: 16px;
    border-top: 1px solid var(--edge);
  }
  .note {
    margin: 10px 0 0;
    max-width: 52ch;
    font-size: 12px;
    line-height: 1.5;
    color: var(--muted);
  }
  .capacity {
    margin-top: 40px;
    max-width: 820px;
  }
  .bar {
    display: flex;
    height: 22px;
    border-radius: 11px;
    overflow: hidden;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
  }
  .bar span {
    min-width: 2px;
  }
  .audio { background: var(--cap-audio); }
  .video { background: var(--cap-video); }
  .other { background: var(--cap-other); }
  .free { background: var(--cap-free); }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 26px;
    margin: 12px 0 0;
    padding: 0;
    list-style: none;
    font-size: 12px;
  }
  .legend li {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .legend .v {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .swatch {
    width: 10px;
    height: 10px;
    border-radius: 3px;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.15);
  }
  .switch {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    font-weight: 500;
  }
  .switch input {
    position: absolute;
    opacity: 0;
    width: 1px;
    height: 1px;
  }
  .switch .track {
    position: relative;
    width: 34px;
    height: 20px;
    border-radius: 10px;
    background: var(--cap-other);
    transition: background 0.15s;
  }
  .switch .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.3);
    transition: transform 0.15s;
  }
  .switch input:checked + .track {
    background: var(--accent);
  }
  .switch input:checked + .track .knob {
    transform: translateX(14px);
  }
  .switch input:focus-visible + .track {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  @media (prefers-reduced-motion: reduce) {
    .switch .track,
    .switch .knob {
      transition: none;
    }
  }
</style>
