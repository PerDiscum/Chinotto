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
