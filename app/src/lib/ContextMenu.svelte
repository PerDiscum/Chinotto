<script lang="ts">
  import { store, type MenuItem } from "./store.svelte";

  const menu = $derived(store.menu);
  let open = $state<number | null>(null); // index of the open submenu
  let el: HTMLDivElement | undefined = $state();

  // Keep the menu inside the window.
  const pos = $derived.by(() => {
    if (!menu) return { x: 0, y: 0 };
    const w = 230, h = menu.items.length * 24 + 10;
    return { x: Math.min(menu.x, window.innerWidth - w - 4), y: Math.min(menu.y, window.innerHeight - h - 4) };
  });

  $effect(() => {
    if (menu) {
      open = null;
      el?.querySelector<HTMLElement>("button:not([disabled])")?.focus();
    }
  });

  function run(item: MenuItem) {
    if (item.disabled || item.submenu) return;
    store.menu = null;
    item.action?.();
  }

  function keydown(e: KeyboardEvent) {
    const buttons = [...(el?.querySelectorAll<HTMLElement>(":scope > .items > button:not([disabled]), .sub button:not([disabled])") ?? [])];
    const i = buttons.indexOf(document.activeElement as HTMLElement);
    if (e.key === "Escape") store.menu = null;
    else if (e.key === "ArrowDown") buttons[(i + 1) % buttons.length]?.focus();
    else if (e.key === "ArrowUp") buttons[(i - 1 + buttons.length) % buttons.length]?.focus();
    else return;
    e.preventDefault();
  }
</script>

{#if menu}
  <div class="catcher" role="presentation" onmousedown={() => (store.menu = null)} oncontextmenu={(e) => { e.preventDefault(); store.menu = null; }}></div>
  <div class="menu" role="menu" tabindex="-1" bind:this={el} style:left="{pos.x}px" style:top="{pos.y}px" onkeydown={keydown}>
    <div class="items">
      {#each menu.items as item, i (i)}
        {#if item.separator}
          <hr />
        {:else}
          <button
            role="menuitem"
            class:danger={item.danger}
            disabled={item.disabled}
            aria-haspopup={item.submenu ? "menu" : undefined}
            aria-expanded={item.submenu ? open === i : undefined}
            onmouseenter={() => (open = item.submenu ? i : null)}
            onfocus={() => (open = item.submenu ? i : null)}
            onclick={() => run(item)}
          >
            {item.label}{#if item.submenu}<span class="arrow">›</span>{/if}
          </button>
          {#if item.submenu && open === i}
            <div class="sub" role="menu">
              {#each item.submenu as s, j (j)}
                <button role="menuitem" disabled={s.disabled} onclick={() => run(s)}>{s.label}</button>
              {:else}
                <button role="menuitem" disabled>No playlists</button>
              {/each}
            </div>
          {/if}
        {/if}
      {/each}
    </div>
  </div>
{/if}

<style>
  .catcher {
    position: fixed;
    inset: 0;
    z-index: 60;
  }
  .menu {
    position: fixed;
    z-index: 61;
    min-width: 200px;
    max-width: 260px;
    padding: 4px;
    background: var(--bg);
    border: 1px solid var(--edge);
    border-radius: 8px;
    box-shadow: 0 10px 30px rgb(0 0 0 / 0.25);
    outline: none;
  }
  .items {
    position: relative;
  }
  button {
    display: flex;
    align-items: center;
    width: 100%;
    height: 24px;
    padding: 0 10px;
    border: 0;
    border-radius: 5px;
    background: none;
    color: var(--ink);
    font: inherit;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  button:hover:not(:disabled),
  button:focus-visible {
    background: var(--sel);
    color: var(--sel-ink);
    outline: none;
  }
  button:disabled {
    color: var(--muted);
  }
  button.danger:not(:hover):not(:focus-visible) {
    color: #a3261a;
  }
  .arrow {
    margin-left: auto;
  }
  hr {
    margin: 4px 6px;
    border: 0;
    border-top: 1px solid var(--edge);
  }
  .sub {
    position: relative;
    margin: 2px 0 4px 12px;
    padding-left: 6px;
    border-left: 2px solid var(--edge);
    max-height: 260px;
    overflow-y: auto;
  }
</style>
