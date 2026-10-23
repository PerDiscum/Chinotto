<script lang="ts">
  // Small line icons drawn for Chinotto (16×16 grid).
  let { name, size = 16 }: { name: "ipod" | "music" | "playlist" | "smart" | "eject" | "folder" | "columns" | "artwork" | "add" | "newlist" | "shuffle" | "repeat" | "repeat-one" | "library" | "disc"; size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  {#if name === "disc"}
    <circle cx="8" cy="8" r="6.5" />
    <circle cx="8" cy="8" r="1.6" />
  {:else if name === "ipod"}
    <rect x="3.5" y="1.5" width="9" height="13" rx="2" />
    <rect x="5" y="3" width="6" height="4" rx="0.6" />
    <circle cx="8" cy="10.8" r="2.1" />
  {:else if name === "music"}
    <path d="M6 12.5V3.5l7-1.5v9" />
    <circle cx="4.3" cy="12.5" r="1.7" />
    <circle cx="11.3" cy="11" r="1.7" />
  {:else if name === "playlist"}
    <path d="M2.5 4h8M2.5 7.5h8M2.5 11h5" />
    <path d="M13 9v4.2" /><circle cx="11.8" cy="13.2" r="1.2" />
  {:else if name === "smart"}
    <path d="M2.5 4h6M2.5 7.5h4M2.5 11h4" />
    <path d="M12 6.5l.8 1.9 1.9.8-1.9.8-.8 1.9-.8-1.9-1.9-.8 1.9-.8z" />
  {:else if name === "eject"}
    <path d="M8 3.5l4.5 5h-9z" fill="currentColor" stroke="none" />
    <path d="M3.5 11.5h9" stroke-width="1.6" />
  {:else if name === "folder"}
    <path d="M1.8 4.2c0-.6.4-1 1-1h3.3l1.4 1.5h5.7c.6 0 1 .4 1 1v6.6c0 .6-.4 1-1 1H2.8c-.6 0-1-.4-1-1z" />
  {:else if name === "columns"}
    <rect x="1.8" y="2.5" width="12.4" height="11" rx="1.2" />
    <path d="M1.8 7h12.4M6 2.5V7M10 2.5V7" />
  {:else if name === "add"}
    <path d="M6 12.5V3.5l7-1.5v5" />
    <circle cx="4.3" cy="12.5" r="1.7" />
    <path d="M12 10v5M9.5 12.5h5" />
  {:else if name === "newlist"}
    <path d="M2.5 4h8M2.5 7.5h8M2.5 11h4" />
    <path d="M12 9.5v5M9.5 12h5" />
  {:else if name === "shuffle"}
    <path d="M1.8 4.5h2.4c2.2 0 3.4 7 6 7h3.6M1.8 11.5h2.4c1 0 1.7-1.3 2.4-2.9M9.2 7.4c.7-1.6 1.4-2.9 2.4-2.9h2.2" />
    <path d="M12.4 2.8l1.7 1.7-1.7 1.7M12.4 9.8l1.7 1.7-1.7 1.7" />
  {:else if name === "repeat" || name === "repeat-one"}
    <path d="M3 9V7.2c0-1.2.9-2.1 2.1-2.1h8.2M13 7v1.8c0 1.2-.9 2.1-2.1 2.1H2.7" />
    <path d="M11.6 3.4l1.7 1.7-1.7 1.7M4.4 12.6l-1.7-1.7 1.7-1.7" />
    {#if name === "repeat-one"}<path d="M7.3 6.6l.9-.7v3.2" stroke-width="1.1" />{/if}
  {:else if name === "library"}
    <rect x="2" y="2.5" width="3" height="11" rx="0.6" />
    <rect x="6.5" y="2.5" width="3" height="11" rx="0.6" />
    <path d="M11.2 3.4l2.6-.6 1.6 10.1-2.6.6z" />
  {:else if name === "artwork"}
    <rect x="2" y="2" width="12" height="12" rx="1.2" />
    <circle cx="8" cy="8" r="3" /><circle cx="8" cy="8" r="0.6" fill="currentColor" />
  {/if}
</svg>
