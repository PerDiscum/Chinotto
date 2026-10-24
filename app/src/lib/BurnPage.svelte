<script lang="ts">
  import { t } from "./i18n";
  import { store, isMac } from "./store.svelte";
  import { bytes } from "./format";
  import happyDisc from "../assets/disc-happy.webp";

  // Burn a disc image (.iso, or .bin + .cue) onto a blank CD or DVD: by
  // default at the slowest speed the drive and disc allow, then checked.
  $effect(() => {
    void store.refreshBurn();
    const timer = setInterval(() => void store.refreshBurn(), 3000);
    return () => clearInterval(timer);
  });

  const st = $derived(store.burnStatus);
  const img = $derived(store.burnImage);
  const media = $derived(st?.media ?? null);
  const fits = $derived(!img || !media || !media.freeSectors || img.sectors <= media.freeSectors);
  const ready = $derived(!!img && !!media?.blank && fits && !store.busy && !(img.needsCdrdao && media.dvd));
  const mediaText = $derived(
    !st ? t("Looking for a burner…")
    : !st.drive ? t("No disc burner found.")
    : !media ? t("No disc in {drive}. Put in a blank CD-R or DVD-R.", { drive: st.drive })
    : !media.blank ? t("The {kind} in the burner isn't blank.", { kind: media.kind })
    : t("Blank {kind}, {size} free", { kind: media.kind, size: bytes(media.freeSectors * 2048) }),
  );
  const name = $derived(img ? img.path.split(/[\\/]/).pop() : "");
</script>

<div class="page">
  <div class="top">
    <h1>{t("Burn a Disc Image")}</h1>
    <dl>
      <dt>{t("Image")}</dt>
      <dd>
        {#if img}
          <strong class="file">{name}</strong>
          <span class="muted">{img.kind === "iso" ? "ISO" : "BIN/CUE"} · {img.description} · {bytes(img.bytes)}</span>
        {:else}
          <span class="muted">{t("None chosen")}</span>
        {/if}
      </dd>
      <dt>{t("Burner")}</dt>
      <dd>{#if st?.drive}{st.drive}<br />{/if}<span class:muted={!media?.blank}>{mediaText}</span></dd>
      <dt>{t("Speed")}</dt>
      <dd>
        <select bind:value={store.burnSpeed} disabled={!!store.busy}>
          <option value={0}>{media?.speeds.length ? t("Slowest ({x}×, most reliable)", { x: media.speeds[0] }) : t("Slowest (most reliable)")}</option>
          {#each (media?.speeds.length ? media.speeds.slice(1) : [2, 4, 8, 16, 24]) as x (x)}<option value={x}>{x}×</option>{/each}
        </select>
      </dd>
      <dt></dt>
      <dd>
        <label class="check"><input type="checkbox" bind:checked={store.burnVerify} disabled={!!store.busy} /> {t("Check the disc against the image afterwards")}</label>
      </dd>
    </dl>
    {#if img && !fits}<p class="warn">{t("The image is bigger than the disc.")}</p>{/if}
    {#if img?.kind === "cue"}
      <p class="muted note">
        {isMac
          ? t("A .bin/.cue is burned through macOS's own disc burning, session-at-once and raw: every sector exactly as in the image, error correction included. macOS can check the disc afterwards.")
          : img.needsCdrdao
            ? t("A .bin/.cue like this one (PlayStation and other mode 2 discs, audio, several tracks) is burned raw with cdrdao: every sector exactly as in the image, in one pass. cdrdao can't check the disc afterwards.")
            : t("A .bin/.cue is burned raw with cdrdao when it's installed: every sector exactly as in the image. Without it, this one's data track is burned as an ISO, which is the same disc unless the image has deliberately damaged sectors.")}
      </p>
    {/if}
    <div class="controls">
      <button onclick={() => store.chooseBurnImage()} disabled={!!store.busy}>{t("Choose Image…")}</button>
      <button class="primary" onclick={() => store.burn()} disabled={!ready}>{t("Burn")}</button>
    </div>
    <p class="muted note">{t("Burning slowly, in one pass, gives the most reliable discs, especially for older players and consoles. The disc is closed afterwards, so nothing more can be added to it.")}</p>
  </div>
  <img class="mascot" src={happyDisc} alt="" />
</div>

<style>
  .page { position: relative; flex: 1; overflow: auto; padding: 32px 40px; background: var(--bg); }
  .top { padding-right: 230px; max-width: 900px; }
  h1 { margin: 0 0 18px; font-size: 26px; }
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 10px 20px; margin: 0; }
  dt { color: var(--muted); text-align: right; }
  dd { margin: 0; min-width: 0; line-height: 1.5; }
  .file { display: block; overflow-wrap: anywhere; }
  .muted { color: var(--muted); }
  .warn { color: #b3261e; }
  .note { max-width: 640px; font-size: 12px; line-height: 1.5; }
  .check { display: inline-flex; align-items: center; gap: 6px; }
  select { font: inherit; }
  .controls { display: flex; gap: 8px; margin: 18px 0 10px; }
  .controls button { font: inherit; font-size: 13px; padding: 4px 12px; border: 1px solid var(--edge); border-radius: 6px; background: var(--zebra); color: var(--ink); }
  .controls button.primary { background: var(--accent); border-color: var(--accent); color: #fff; }
  .controls button:disabled { opacity: 0.5; }
  .mascot { position: absolute; top: 28px; right: 40px; width: 200px; height: auto; pointer-events: none; }
  @media (max-width: 760px) { .mascot { display: none; } .top { padding-right: 0; } }
</style>
