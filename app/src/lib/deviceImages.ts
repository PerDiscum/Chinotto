// Device pictures (app/src/assets/devices/<key>.webp), drawn by the developer.
// Models without their own picture borrow a look-alike, else "generic".

const files = import.meta.glob<string>("../assets/devices/*.webp", { eager: true, import: "default", query: "?url" });
const byKey = new Map(Object.entries(files).map(([path, url]) => [path.split("/").pop()!.replace(".webp", ""), url]));

const ALIASES: Record<string, string> = {
  "ipod-2g": "ipod-1g", // same body
  "ipod-6g": "ipod-7g", // same body
  "ipod-shuffle-1g": "ipod-shuffle",
  "ipod-shuffle-2g": "ipod-shuffle",
  "ipod-shuffle-3g": "ipod-shuffle",
  "ipod-shuffle-4g": "ipod-shuffle",
};

export function deviceImage(key: string | undefined): string | undefined {
  const k = key ?? "generic";
  return byKey.get(k) ?? byKey.get(ALIASES[k] ?? "") ?? byKey.get("generic");
}

const LABELS: Record<string, string> = {
  "ipod-1g": "iPod (1st/2nd generation)",
  "ipod-3g": "iPod (3rd generation)",
  "ipod-4g": "iPod (4th generation) / photo",
  "ipod-4g-u2": "iPod U2 Special Edition (4th generation)",
  "ipod-5g": "iPod with video (5th generation)",
  "ipod-5g-u2": "iPod U2 Special Edition (5th generation)",
  "ipod-5g-smile": "iPod with video (5th generation), smiling",
  "ipod-7g-smile": "iPod classic (6th/7th generation), smiling",
  "ipod-7g": "iPod classic",
  "ipod-mini": "iPod mini",
  "ipod-nano-1g": "iPod nano (1st generation)",
  "ipod-nano-2g": "iPod nano (2nd generation)",
  "ipod-nano-3g": "iPod nano (3rd generation)",
  "ipod-nano-6g": "iPod nano (6th generation)",
  "ipod-shuffle": "iPod shuffle",
  android: "Android phone",
  "cowon-iaudio-x5": "Cowon iAudio X5",
  "creative-zen-micro": "Creative Zen Micro",
  "creative-zen-stone": "Creative Zen Stone",
  "creative-zen-touch": "Creative Zen Touch",
  "creative-zen-vision-m": "Creative Zen Vision:M",
  "creative-zen-vision-w": "Creative Zen Vision W",
  "fiio-x1": "FiiO X1",
  "hifiwalker-h2": "HiFi Walker H2",
  "intenso-music-walker": "Intenso Music Walker",
  psp: "PlayStation Portable",
  "sansa-clip": "Sansa Clip",
  "sansa-e130": "Sansa e130",
  "sansa-e200": "Sansa e200",
  "sansa-fuze": "Sansa Fuze",
  "sansa-m200": "Sansa m200",
  "sony-a10": "Sony Walkman A10",
  "sony-nwz": "Sony Walkman NWZ",
  "sony-nwz-smile": "Sony Walkman NWZ, smiling",
  "sony-a10-smile": "Sony Walkman A10/A20, smiling",
  "sony-openmg": "Sony Walkman (OpenMG, Japan)",
  "windows-phone": "Windows Phone",
  "iriver-h100": "iriver H100",
  "iriver-ifp": "iriver iFP",
  xclef: "DMC Xclef Pogo Jukebox",
  generic: "Generic player",
};

/** Every picture there is, iPods first, for the picture chooser. */
export const PICTURES: { key: string; label: string; url: string }[] = [...byKey.entries()]
  .map(([key, url]) => ({ key, url, label: LABELS[key] ?? key }))
  .sort((a, b) => Number(!a.key.startsWith("ipod")) - Number(!b.key.startsWith("ipod")) || Number(a.key === "generic") - Number(b.key === "generic") || a.label.localeCompare(b.label));
