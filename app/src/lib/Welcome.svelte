<script lang="ts">
  import { store } from "./store.svelte";
</script>

<div class="welcome">
  <div class="inner">
    <h1>Ready? Plug in your digital audio player to begin!</h1>
    <p>
      If you’re using a classic, pre-iOS Apple iPod or an MTP device (such as a Creative Zen), it will show up here
      automatically. If you are using an MSC device (such as most Sony Walkman and Sansa models), click below to add the
      drive manually to Chinotto.
    </p>
    <button onclick={() => store.openFolder()}>Add Drive…</button>
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
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
