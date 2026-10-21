<script lang="ts">
  import { t, tn } from "./i18n";
  import { store } from "./store.svelte";
</script>

<div class="welcome">
  <div class="inner">
    <h1>{t("Connect your device!")}</h1>
    <p>
      {t("Welcome to Chinotto! Ready? Plug in your digital audio player to begin! If you’re using a classic, pre-iOS Apple iPod or an MTP device (such as a Creative Zen), it will show up here automatically. If you are using an MSC device (e.g., most Sony Walkman and Sansa models), click below to add the drive manually to Chinotto.")}
    </p>
    <div class="buttons">
      <button onclick={() => store.openFolder()}>{t("Add Drive…")}</button>
      <button class="quiet" onclick={() => (store.dialog = { kind: "library", id: null })}>{t("New Library…")}</button>
    </div>
    <p class="small">{t("A library holds the music on your computer: choose its folders and Chinotto lists what's in them, without changing your files.")}</p>
  </div>
</div>

<style>
  .welcome {
    flex: 1;
    display: grid;
    place-items: center;
    background: var(--bg);
    padding: 24px;
  }
  .inner {
    max-width: 52ch;
  }
  h1 {
    margin: 0 0 8px;
    font-size: 22px;
    font-weight: 600;
  }
  p {
    margin: 0 0 18px;
    line-height: 1.5;
    color: var(--muted);
  }
  button {
    padding: 6px 16px;
    border: 0;
    border-radius: 6px;
    background: var(--accent);
    color: #fff;
    font: inherit;
    font-weight: 500;
  }
  .buttons {
    display: flex;
    gap: 8px;
  }
  button.quiet {
    background: none;
    color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  .small {
    margin-top: 14px;
    font-size: 12px;
  }
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
