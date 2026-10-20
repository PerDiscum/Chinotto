<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    onclose,
    children,
    buttons,
    width = 460,
  }: { title: string; onclose: () => void; children: Snippet; buttons?: Snippet; width?: number } = $props();

  let box: HTMLDivElement | undefined = $state();

  $effect(() => {
    // Focus the first field (or button) when the dialog opens.
    const first = box?.querySelector<HTMLElement>("input:not([disabled]), textarea:not([disabled]), button.primary, button");
    first?.focus();
  });

  function keydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
    // Keep Tab inside the dialog.
    if (e.key === "Tab" && box) {
      const f = [...box.querySelectorAll<HTMLElement>("input, textarea, button, select")].filter((x) => !x.hasAttribute("disabled"));
      if (!f.length) return;
      const i = f.indexOf(document.activeElement as HTMLElement);
      if (e.shiftKey && i <= 0) {
        e.preventDefault();
        f[f.length - 1].focus();
      } else if (!e.shiftKey && i === f.length - 1) {
        e.preventDefault();
        f[0].focus();
      }
    }
  }
</script>

<div class="backdrop" role="presentation" onmousedown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={title} style:width="{width}px" bind:this={box} onkeydown={keydown} tabindex="-1">
    <h2>{title}</h2>
    <div class="body">{@render children()}</div>
    {#if buttons}<div class="buttons">{@render buttons()}</div>{/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(20 12 8 / 0.35);
  }
  .modal {
    max-width: 100%;
    max-height: calc(100vh - 48px);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--ink);
    border: 1px solid var(--edge);
    border-radius: 12px;
    box-shadow: 0 18px 50px rgb(0 0 0 / 0.35);
    outline: none;
  }
  h2 {
    margin: 0;
    padding: 14px 18px 0;
    font-size: 15px;
    font-weight: 600;
  }
  .body {
    padding: 12px 18px;
    overflow: auto;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 10px 18px 14px;
    border-top: 1px solid var(--edge);
  }
  .buttons :global(button) {
    padding: 5px 14px;
    border: 1px solid var(--field-edge);
    border-radius: 6px;
    background: var(--field);
    color: var(--ink);
    font: inherit;
  }
  .buttons :global(button.primary) {
    border-color: var(--accent);
    background: var(--accent);
    color: #fff;
  }
  .buttons :global(button.danger) {
    border-color: #a3261a;
    background: #a3261a;
    color: #fff;
  }
  .buttons :global(button:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .buttons :global(.left) {
    margin-right: auto;
    align-self: center;
    font-size: 12px;
    color: var(--muted);
  }
</style>
