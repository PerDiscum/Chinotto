<script lang="ts">
  import { store } from "./store.svelte";
  import { bytes, count, isVideo, longDuration } from "./format";

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
    { label: "Audio", value: stats.audio, cls: "audio" },
    { label: "Video", value: stats.video, cls: "video" },
    { label: "Other", value: stats.other, cls: "other" },
    { label: "Free", value: stats.free, cls: "free" },
  ].filter((s) => s.value > 0 || s.cls === "free"));
  const pct = (v: number) => (stats.cap ? (v / stats.cap) * 100 : 0);
  const userPlaylists = $derived(ipod.playlists.filter((p) => !p.master).length);
</script>

<div class="page">
  <section class="summary">
    <h1>{ipod.name}</h1>
    <dl>
      <dt>Model</dt><dd>{ipod.device.model ?? "iPod"}{#if ipod.device.modelNumber}{" "}<span class="muted">({ipod.device.modelNumber})</span>{/if}</dd>
      {#if stats.cap}<dt>Capacity</dt><dd>{bytes(stats.cap)}</dd>{/if}
      {#if ipod.serial}<dt>Serial number</dt><dd>{ipod.serial}</dd>{/if}
      <dt>Contents</dt><dd>{count(ipod.tracks.length)} songs, {longDuration(stats.ms)}, {count(userPlaylists)} playlists</dd>
      <dt>Database</dt><dd>version {ipod.dbVersion.toString(16)}{ipod.hasArtworkDb ? ", with artwork" : ", no artwork"}</dd>
      <dt>Location</dt><dd class="path">{ipod.path}</dd>
    </dl>
    <div class="actions">
      {#if !ipod.device.manual}
        <button onclick={() => store.eject(ipod.path)} disabled={!!store.ejecting}>Eject</button>
      {/if}
    </div>
    <p class="note">Chinotto can browse this iPod but can't change it yet. Adding songs, editing and syncing come in the next version.</p>
  </section>

  {#if stats.cap}
    <section class="capacity" aria-label="Capacity">
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
  .note {
    margin: 18px 0 0;
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
