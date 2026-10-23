<script lang="ts">
  import { t } from "./i18n";
  import type { Conversion } from "./types";

  // A device's conversion settings. Simple: everything it can't play becomes
  // one format. Advanced: a rule for each kind of file.
  let {
    conversion,
    plays,
    disabled = false,
    device,
    onchange,
  }: { conversion: Conversion; plays: string[]; disabled?: boolean; device: string; onchange: (c: Conversion) => void } = $props();

  const KINDS: [string, string][] = [
    ["flac", "FLAC"],
    ["alac", "Apple Lossless"],
    ["aac", "AAC"],
    ["mp3", "MP3"],
    ["ogg", "Ogg Vorbis"],
    ["opus", "Opus"],
    ["wav", "WAV"],
    ["aiff", "AIFF"],
    ["wma", "WMA"],
  ];
  const aac = $derived(plays.includes("aac"));
  const alac = $derived(plays.includes("alac"));
  const set = (patch: Partial<Conversion>) => onchange({ ...conversion, ...patch });
  const rule = (kind: string) => conversion.rules?.[kind] ?? "keep";
  const setRule = (kind: string, v: string) => set({ rules: { ...(conversion.rules ?? {}), [kind]: v } });
</script>

<div class="convert">
  <div class="mode" role="radiogroup" aria-label={t("Conversion settings")}>
    <label><input type="radio" checked={!conversion.advanced} onchange={() => set({ advanced: false })} {disabled} /> {t("Simple")}</label>
    <label><input type="radio" checked={conversion.advanced} onchange={() => set({ advanced: true })} {disabled} /> {t("Advanced")}</label>
  </div>
  <label>
    {conversion.advanced ? t("Anything else it can't play becomes") : t("Convert songs the {device} can't play to", { device })}
    <select value={conversion.format} onchange={(e) => set({ format: e.currentTarget.value })} {disabled}>
      {#if aac}
        <option value="aac256">{t("AAC 256 kbps (iTunes Plus)")}</option>
        <option value="aac128">{t("AAC 128 kbps (smaller)")}</option>
      {/if}
      <option value="mp3_320">{t("MP3 320 kbps")}</option>
      <option value="mp3_256">{t("MP3 256 kbps")}</option>
      <option value="mp3_192">{t("MP3 192 kbps")}</option>
      <option value="mp3_128">{t("MP3 128 kbps (smaller)")}</option>
    </select>
  </label>
  {#if !conversion.advanced}
    {#if alac}
      <label class="check">
        <input type="checkbox" checked={conversion.convertAlac} onchange={(e) => set({ convertAlac: e.currentTarget.checked })} {disabled} />
        {t("Also convert Apple Lossless (it plays on this device, but takes about four times the space)")}
      </label>
    {/if}
  {:else}
    <table class="rules">
      <thead><tr><th>{t("Songs in")}</th><th>{t("are copied as")}</th></tr></thead>
      <tbody>
        {#each KINDS as [kind, name] (kind)}
          <tr>
            <td>{name}</td>
            <td>
              <select value={rule(kind)} onchange={(e) => setRule(kind, e.currentTarget.value)} {disabled}>
                <option value="keep">{plays.includes(kind) ? t("Keep as is") : t("Convert (it doesn't play here)")}</option>
                {#if aac}
                  <option value="aac256">{t("AAC 256 kbps")}</option>
                  <option value="aac128">{t("AAC 128 kbps")}</option>
                {/if}
                <option value="mp3_320">{t("MP3 320 kbps")}</option>
                <option value="mp3_256">{t("MP3 256 kbps")}</option>
                <option value="mp3_192">{t("MP3 192 kbps")}</option>
                <option value="mp3_128">{t("MP3 128 kbps")}</option>
                {#if alac}<option value="alac">{t("Apple Lossless")}</option>{/if}
              </select>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="note">{t("WMA can't be converted yet (it needs an extra decoder); WMA songs are copied only to devices that play them.")}</p>
  {/if}
  <p class="note">{t("Converted copies are made in a temporary folder just for the device, then deleted. Your files and libraries don't change.")}</p>
</div>

<style>
  .convert {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 6px;
    font-size: 13px;
  }
  .mode {
    display: flex;
    gap: 16px;
  }
  .mode label,
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  label {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  select {
    font: inherit;
  }
  .rules {
    border-collapse: collapse;
    width: max-content;
  }
  .rules th {
    text-align: left;
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
    padding: 2px 16px 4px 0;
  }
  .rules td {
    padding: 2px 16px 2px 0;
  }
  .note {
    margin: 0;
    font-size: 12px;
    color: var(--muted);
    line-height: 1.5;
    max-width: 60ch;
  }
</style>
