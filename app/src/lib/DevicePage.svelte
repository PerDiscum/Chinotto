<script lang="ts">
  import { t, tn } from "./i18n";
  import { store } from "./store.svelte";
  import { bytes, count, isVideo, longDuration } from "./format";
  import { deviceImage, PICTURES } from "./deviceImages";
  import ConversionSettings from "./ConversionSettings.svelte";

  const ipod = $derived(store.ipod!);
  const stats = $derived.by(() => {
    let audio = 0, video = 0, ms = 0;
    for (const t of ipod.tracks) {
      if (isVideo(t)) video += t.size;
      else audio += t.size;
      ms += t.lengthMs;
    }
    const cap = ipod.device.capacity;
    const used = cap - ipod.device.free;
    return { audio, video, other: Math.max(0, used - audio - video), free: ipod.device.free, cap, ms };
  });
  const segs = $derived([
    { label: t("Audio"), value: stats.audio, cls: "audio" },
    { label: t("Video"), value: stats.video, cls: "video" },
    { label: t("Other"), value: stats.other, cls: "other" },
    { label: t("Free"), value: stats.free, cls: "free" },
  ].filter((s) => s.value > 0 || s.cls === "free"));
  const pct = (v: number) => (stats.cap ? (v / stats.cap) * 100 : 0);
  const identity = $derived(ipod.identity ?? ipod.device.identity ?? null);
  const chosen = $derived(identity ? store.deviceIcons[identity] : undefined);
  const shown = $derived(store.iconFor({ icon: ipod.device.icon, identity }));
  let choosing = $state(false);
  function pick(key: string | null) {
    if (identity) void store.setDeviceIcon(identity, key);
    choosing = false;
  }
  let pictureEl: HTMLElement | undefined = $state();
  function outside(e: MouseEvent) {
    if (choosing && pictureEl && !pictureEl.contains(e.target as Node)) choosing = false;
  }
  const userPlaylists = $derived(ipod.playlists.filter((p) => !p.master).length);
</script>

<svelte:window onmousedown={outside} onkeydown={(e) => e.key === "Escape" && (choosing = false)} />

<div class="page">
  <div class="top">
  <section class="summary">
    <h1>{ipod.name}</h1>
    <dl>
      <dt>{t("Model")}</dt><dd>{ipod.device.model ?? "iPod"}{#if ipod.device.modelNumber}{" "}<span class="muted">({ipod.device.modelNumber})</span>{/if}</dd>
      {#if stats.cap}<dt>{t("Capacity")}</dt><dd>{bytes(stats.cap)}</dd>{/if}
      {#if ipod.serial}<dt>{t("Serial number")}</dt><dd>{ipod.serial}</dd>{/if}
      <dt>{t("Contents")}</dt><dd>{tn(ipod.tracks.length, "{n} song", "{n} songs")}, {longDuration(stats.ms)}, {tn(userPlaylists, "{n} playlist", "{n} playlists")}</dd>
      <dt>{t("Database")}</dt><dd>{ipod.hasArtworkDb ? t("version {v}, with artwork", { v: ipod.dbVersion.toString(16) }) : t("version {v}, no artwork", { v: ipod.dbVersion.toString(16) })}</dd>
      <dt>{t("Location")}</dt><dd class="path">{ipod.path}</dd>
    </dl>
    <div class="actions">
      {#if !ipod.device.manual}
        <button onclick={() => store.eject(ipod.path)} disabled={!!store.ejecting}>{t("Eject")}</button>
      {/if}
    </div>
  </section>
  <div class="picture" bind:this={pictureEl}>
    <button
      class="device"
      onclick={() => (choosing = !choosing)}
      disabled={!identity}
      aria-expanded={choosing}
      title={identity ? t("Choose a picture for this iPod") : t("This iPod doesn't report a FireWire ID or serial number, so a picture can't be remembered for it")}
    >
      <img src={deviceImage(shown)} alt={ipod.device.model ?? "iPod"} />
    </button>
    {#if choosing}
      <div class="chooser" role="dialog" aria-label={t("Choose a picture")}>
        <button class="auto" class:on={!chosen} onclick={() => pick(null)}>
          {t("Use automatic")}{#if ipod.device.model}{" "}<span class="muted">({ipod.device.model})</span>{/if}
        </button>
        <div class="grid">
          {#each PICTURES as p (p.key)}
            <button class:on={chosen === p.key} onclick={() => pick(p.key)} title={p.label} aria-label={p.label}>
              <img src={p.url} alt="" />
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>
  </div>
  <section class="summary">
    <div class="changes">
      {#if !ipod.identity}
        <p class="note">{t("This iPod doesn't report a FireWire ID or serial number, so Chinotto keeps it read-only.")}</p>
      {:else}
        <label class="switch">
          <input type="checkbox" role="switch" checked={ipod.writable} onchange={(e) => store.allowChanges(e.currentTarget.checked)} disabled={!!store.busy} />
          <span class="track" aria-hidden="true"><span class="knob"></span></span>
          {t("Allow changes to this device.")}
        </label>
        <p class="note">
          {#if ipod.device.icon === "ipod-nano-6g" && ipod.writable}
            {t("Nano 6 playlist writing is experimental. It can change playlists only; songs and covers remain read-only. Chinotto saves the ten most recent copies of the iPod's databases to {path}.", { path: "iPod_Control/iTunes/Chinotto Backups" })}
          {:else if ipod.device.icon === "ipod-nano-6g"}
            {t("Nano 6 changes are experimental. Turn this on to change playlists only; songs and covers remain read-only.")}
          {:else if ipod.writable}
            {t("Chinotto can edit, add, and delete songs and change playlists on this iPod. It saves the ten most recent copies of the iPod's database to {path}.", { path: "iPod_Control/iTunes/Chinotto Backups" })}
          {:else}
            {t("Chinotto only reads this iPod. Turn this on to edit songs, add or delete them and change playlists. The setting is remembered for this iPod only.")}
          {/if}
        </p>
        <ConversionSettings conversion={ipod.conversion} plays={ipod.plays} device="iPod" disabled={!!store.busy} onchange={(c) => store.setConversion(c)} />
      {/if}
    </div>
  </section>

  {#if stats.cap}
    <section class="capacity" aria-label={t("Capacity")}>
      <div class="bar" role="img" aria-label={segs.map((s) => `${s.label} ${bytes(s.value)}`).join(", ")}>
        {#each segs as s (s.cls)}
          <span class={s.cls} style:width="{pct(s.value)}%"></span>
        {/each}
      </div>
      <ul class="legend">
        {#each segs as s (s.cls)}
          <li><span class="swatch {s.cls}"></span>{s.label}<span class="v">{bytes(s.value)}</span></li>
        {/each}
      </ul>
    </section>
  {/if}
</div>

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
  .chooser {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    z-index: 30;
    width: 316px;
    padding: 8px;
    border: 1px solid var(--edge);
    border-radius: 10px;
    background: var(--bg);
    box-shadow: 0 10px 30px rgb(0 0 0 / 0.25);
  }
  .chooser .auto {
    width: 100%;
    margin-bottom: 8px;
    padding: 5px 8px;
    border: 1px solid var(--edge);
    border-radius: 6px;
    background: none;
    color: var(--ink);
    font: inherit;
    font-size: 12px;
    text-align: left;
  }
  .chooser .grid {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 4px;
  }
  .chooser .grid button {
    display: grid;
    place-items: center;
    height: 64px;
    padding: 4px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: none;
  }
  .chooser .grid img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  .chooser button:hover {
    background: var(--zebra);
  }
  .chooser button.on {
    border-color: var(--accent);
    background: var(--sel-quiet);
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
</style>
