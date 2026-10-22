<script lang="ts">
  import { t, tn } from "./i18n";
  import { store } from "./store.svelte";
  import emblem from "../assets/welcome-emblem.png";
</script>

<div class="welcome">
  {#if store.openFailed && store.devices.some((d) => d.path === store.openFailed)}
    {@const path = store.openFailed}
    <div class="inner">
      <h1>{t("Chinotto couldn't read this iPod's database")}</h1>
      <p>
        {t("It may be damaged, or written by software Chinotto doesn't understand. You can rebuild it: the songs on the iPod are added back from their files (playlists, play counts and ratings may be lost), and the old database is kept in a backup folder on the iPod.")}
      </p>
      <div class="buttons">
        <button onclick={() => store.open(path)}>{t("Try Again")}</button>
        <button class="quiet" onclick={() => (store.dialog = { kind: "setup", path, rebuild: true })}>{t("Rebuild Database…")}</button>
      </div>
    </div>
  {:else}
  <div class="inner">
    <img class="emblem" src={emblem} alt="" aria-hidden="true" />
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
  {/if}
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
  .emblem {
    display: block;
    width: 160px;
    height: 160px;
    object-fit: contain;
    margin: 0 auto 22px;
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
  @media (max-width: 760px), (max-height: 680px) {
    .emblem {
      display: none;
    }
  }
</style>
