// Fast sorting and searching over an iPod's tracks.
// Strings are ranked once with a locale-aware collator so later sorts only
// compare integers (23k tracks sort in a few milliseconds).

import type { BrowseField, SortKey, Track } from "./types";
import { browseAlbum, browseArtist, browseComposer, browseGenre, browseYear, fold, formatName, stripThe } from "./format";

export const collator = new Intl.Collator(undefined, { sensitivity: "base", numeric: true });

function ranks(values: string[]): Uint32Array {
  const uniq = [...new Set(values)].sort(collator.compare);
  const m = new Map(uniq.map((v, i) => [v, i]));
  return Uint32Array.from(values, (v) => m.get(v)!);
}

export class TrackIndex {
  readonly tracks: Track[];
  readonly byId = new Map<number, Track>();
  private pos = new Map<number, number>();
  private rTitle: Uint32Array;
  private rArtist: Uint32Array;
  private rAlbum: Uint32Array;
  private rGenre: Uint32Array;
  private rKind: Uint32Array;
  private rFormat: Uint32Array;
  private haystack: string[];

  constructor(tracks: Track[]) {
    this.tracks = tracks;
    tracks.forEach((t, i) => {
      this.byId.set(t.id, t);
      this.pos.set(t.id, i);
    });
    this.rTitle = ranks(tracks.map((t) => t.sortTitle ?? t.title ?? ""));
    this.rArtist = ranks(tracks.map((t) => t.sortArtist ?? stripThe(t.artist ?? "")));
    this.rAlbum = ranks(tracks.map((t) => t.sortAlbum ?? t.album ?? ""));
    this.rGenre = ranks(tracks.map((t) => t.genre ?? ""));
    this.rKind = ranks(tracks.map((t) => t.kind));
    this.rFormat = ranks(tracks.map(formatName));
    this.haystack = tracks.map((t) =>
      fold([t.title, t.artist, t.albumArtist, t.album, t.composer, t.genre].filter(Boolean).join("\n")),
    );
  }

  get(ids: number[]): Track[] {
    const out: Track[] = [];
    for (const id of ids) {
      const t = this.byId.get(id);
      if (t) out.push(t);
    }
    return out;
  }

  /** Every word of the query must appear somewhere in the track's main fields. */
  search(tracks: Track[], query: string): Track[] {
    const words = fold(query).split(/\s+/).filter(Boolean);
    if (!words.length) return tracks;
    return tracks.filter((t) => {
      const h = this.haystack[this.pos.get(t.id)!];
      return words.every((w) => h.includes(w));
    });
  }

  /** `order` keeps the list's own order (a playlist's), reversed for -1. */
  sort(tracks: Track[], key: SortKey, dir: 1 | -1): Track[] {
    if (key === "order") return dir === 1 ? tracks : [...tracks].reverse();
    const p = (t: Track) => this.pos.get(t.id)!;
    const albumOrder = (a: Track, b: Track) =>
      this.rAlbum[p(a)] - this.rAlbum[p(b)] || a.discNr - b.discNr || a.trackNr - b.trackNr || this.rTitle[p(a)] - this.rTitle[p(b)];
    const artistOrder = (a: Track, b: Track) => this.rArtist[p(a)] - this.rArtist[p(b)] || albumOrder(a, b);
    const primary: Record<SortKey, (a: Track, b: Track) => number> = {
      title: (a, b) => this.rTitle[p(a)] - this.rTitle[p(b)] || artistOrder(a, b),
      time: (a, b) => a.lengthMs - b.lengthMs,
      artist: artistOrder,
      album: albumOrder,
      genre: (a, b) => this.rGenre[p(a)] - this.rGenre[p(b)] || artistOrder(a, b),
      year: (a, b) => a.year - b.year || artistOrder(a, b),
      rating: (a, b) => a.rating - b.rating || artistOrder(a, b),
      plays: (a, b) => a.playCount - b.playCount || artistOrder(a, b),
      added: (a, b) => (a.added ?? 0) - (b.added ?? 0),
      kind: (a, b) => this.rKind[p(a)] - this.rKind[p(b)] || artistOrder(a, b),
      format: (a, b) => this.rFormat[p(a)] - this.rFormat[p(b)] || artistOrder(a, b),
      bitrate: (a, b) => a.bitrate - b.bitrate || artistOrder(a, b),
      order: () => 0,
    };
    const cmp = primary[key];
    return [...tracks].sort((a, b) => dir * cmp(a, b));
  }
}

/** All browser columns, broadest first (the order they're shown in). */
export const BROWSE_FIELDS: { field: BrowseField; title: string; plural: [string, string] }[] = [
  { field: "genre", title: "Genre", plural: ["Genre", "Genres"] },
  { field: "format", title: "Format", plural: ["Format", "Formats"] },
  { field: "year", title: "Year", plural: ["Year", "Years"] },
  { field: "composer", title: "Composer", plural: ["Composer", "Composers"] },
  { field: "artist", title: "Artist", plural: ["Artist", "Artists"] },
  { field: "album", title: "Album", plural: ["Album", "Albums"] },
];

const VALUE: Record<BrowseField, (t: Track) => string> = {
  genre: browseGenre,
  format: formatName,
  year: browseYear,
  composer: browseComposer,
  artist: browseArtist,
  album: browseAlbum,
};

/** How a value sorts in its column. */
function sortKey(field: BrowseField, t: Track, v: string): string {
  if (field === "artist") return t.compilation ? v : ((t.albumArtist ? t.sortAlbumArtist : t.sortArtist) ?? stripThe(v));
  if (field === "album") return t.sortAlbum ?? v;
  if (field === "composer") return t.sortComposer ?? v;
  return v;
}

export interface BrowserColumn {
  field: BrowseField;
  items: string[];
}

/**
 * Column browser lists, each narrowed by the selections to its left, plus the
 * tracks matching every selection.
 */
export function browse(
  tracks: Track[],
  fields: BrowseField[],
  sel: Partial<Record<BrowseField, string | null>>,
): { columns: BrowserColumn[]; tracks: Track[] } {
  const maps = fields.map(() => new Map<string, string>()); // value → sort key
  const out: Track[] = [];
  outer: for (const t of tracks) {
    for (let i = 0; i < fields.length; i++) {
      const f = fields[i];
      const v = VALUE[f](t);
      if (!maps[i].has(v)) maps[i].set(v, sortKey(f, t, v));
      const want = sel[f] ?? null;
      if (want !== null && v !== want) continue outer;
    }
    out.push(t);
  }
  const columns = fields.map((field, i) => {
    const items = [...maps[i].entries()].sort((a, b) => collator.compare(a[1], b[1])).map(([v]) => v);
    return { field, items };
  });
  return { columns, tracks: out };
}
