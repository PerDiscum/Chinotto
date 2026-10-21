// Device pictures (app/src/assets/devices/<key>.png), drawn by the developer.
// Models without their own picture borrow a look-alike, else "generic".

const files = import.meta.glob<string>("../assets/devices/*.png", { eager: true, import: "default", query: "?url" });
const byKey = new Map(Object.entries(files).map(([path, url]) => [path.split("/").pop()!.replace(".png", ""), url]));

const ALIASES: Record<string, string> = {
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
  "ipod-3g": "iPod (3rd generation)",
  "ipod-4g": "iPod (4th generation) / photo",
  "ipod-7g": "iPod classic",
  "ipod-mini": "iPod mini",
  "ipod-nano-2g": "iPod nano (2nd generation)",
  "ipod-nano-3g": "iPod nano (3rd generation)",
  "ipod-shuffle": "iPod shuffle",
  "sansa-clip": "Sansa Clip",
  "sansa-fuze": "Sansa Fuze",
  "sansa-e200": "Sansa e200",
  "sony-nwz": "Sony Walkman NWZ",
  "sony-a10": "Sony Walkman A10",
  "fiio-x1": "FiiO X1",
  "cowon-iaudio-x5": "Cowon iAudio X5",
  "iriver-h100": "iriver H100",
  "iriver-ifp": "iriver iFP",
  xclef: "xclef",
  generic: "Generic player",
};

/** Every picture there is, iPods first, for the picture chooser. */
export const PICTURES: { key: string; label: string; url: string }[] = [...byKey.entries()]
  .map(([key, url]) => ({ key, url, label: LABELS[key] ?? key }))
  .sort((a, b) => Number(!a.key.startsWith("ipod")) - Number(!b.key.startsWith("ipod")) || Number(a.key === "generic") - Number(b.key === "generic") || a.label.localeCompare(b.label));
