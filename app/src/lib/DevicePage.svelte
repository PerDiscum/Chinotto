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
    <div class="changes">
      {#if !ipod.identity}
        <p class="note">This iPod doesn't report a FireWire ID or serial number, so Chinotto keeps it read-only.</p>
      {:else}
        <label class="switch">
          <input type="checkbox" role="switch" checked={ipod.writable} onchange={(e) => store.allowChanges(e.currentTarget.checked)} disabled={!!store.busy} />
          <span class="track" aria-hidden="true"><span class="knob"></span></span>
          Allow changes to this iPod
        </label>
        <p class="note">
          {#if ipod.writable}
            Chinotto can edit, add and delete songs and change playlists on this iPod. Before every save it copies the
            iPod's database to <code>iPod_Control/iTunes/Chinotto Backups</code> and keeps the newest 10.
          {:else}
            Chinotto only reads this iPod. Turn this on to edit songs, add or delete them and change playlists. The setting is
            remembered for this iPod only.
          {/if}
        </p>
      {/if}
    </div>
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
  code {
    font-size: 11px;
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
