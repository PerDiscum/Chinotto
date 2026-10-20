<script lang="ts">
  import { store } from "./store.svelte";
</script>

<div class="welcome">
  <div class="inner">
    <h1>Connect an iPod</h1>
    <p>Plug in a classic iPod and Chinotto opens it as soon as it mounts. You can also open a copy of an iPod's folder from your disk.</p>
    <button onclick={() => store.openFolder()}>Open iPod folder…</button>
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
    max-width: 44ch;
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
