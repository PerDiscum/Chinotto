// Fast sorting and searching over an iPod's tracks.
// Strings are ranked once with a locale-aware collator so later sorts only
// compare integers (23k tracks sort in a few milliseconds).

import type { BrowseField, SortKey, Track } from "./types";
import { browseAlbum, browseArtist, browseComposer, browseGenre, browseYear, COMPILATIONS, fold, formatName, stripThe, UNKNOWN_ALBUM, UNKNOWN_ARTIST, VARIOUS } from "./format";
import { t } from "./i18n";

export const collator = new Intl.Collator(undefined, { sensitivity: "base", numeric: true });

/** Rank of each value in sorted order; values the collator counts as equal
 *  ("Portishead", "portishead ") share a rank, so they sort as one group. */
function ranks(values: string[]): Uint32Array {
  const uniq = [...new Set(values)].sort(collator.compare);
  const m = new Map<string, number>();
  let r = 0;
  uniq.forEach((v, i) => {
    if (i && collator.compare(uniq[i - 1].trim(), v.trim()) !== 0) r++;
    m.set(v, r);
  });
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
  private rAlbumArtist: Uint32Array;
  /** Year of each track's album (earliest year among its songs). */
  private albumYear: Uint32Array;
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
    this.rAlbumArtist = ranks(tracks.map(albumArtistSortKey));
    const years = new Map<string, number>();
    for (const t of tracks) {
      const k = albumKey(t);
      if (t.year && (!years.has(k) || t.year < years.get(k)!)) years.set(k, t.year);
    }
    this.albumYear = Uint32Array.from(tracks, (t) => years.get(albumKey(t)) ?? 0);
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
    const inAlbum = (a: Track, b: Track) => a.discNr - b.discNr || a.trackNr - b.trackNr || this.rTitle[p(a)] - this.rTitle[p(b)];
    const albumOrder = (a: Track, b: Track) => this.rAlbum[p(a)] - this.rAlbum[p(b)] || inAlbum(a, b);
    // Everywhere: album artist, then the album's year, then album title; compilations last.
    const artistOrder = (a: Track, b: Track) =>
      Number(a.compilation) - Number(b.compilation) ||
      this.rAlbumArtist[p(a)] - this.rAlbumArtist[p(b)] ||
      this.albumYear[p(a)] - this.albumYear[p(b)] ||
      albumOrder(a, b);
    const primary: Record<SortKey, (a: Track, b: Track) => number> = {
      title: (a, b) => this.rTitle[p(a)] - this.rTitle[p(b)] || artistOrder(a, b),
      time: (a, b) => a.lengthMs - b.lengthMs,
      // The Artist column sorts by the artist it shows, then each album by year and title.
      artist: (a, b) => this.rArtist[p(a)] - this.rArtist[p(b)] || this.albumYear[p(a)] - this.albumYear[p(b)] || albumOrder(a, b),
      album: albumOrder,
      genre: (a, b) => this.rGenre[p(a)] - this.rGenre[p(b)] || artistOrder(a, b),
      year: (a, b) => a.year - b.year || artistOrder(a, b),
      rating: (a, b) => a.rating - b.rating || artistOrder(a, b),
      plays: (a, b) => a.playCount - b.playCount || artistOrder(a, b),
      added: (a, b) => (a.added ?? 0) - (b.added ?? 0),
      kind: (a, b) => this.rKind[p(a)] - this.rKind[p(b)] || artistOrder(a, b),
      format: (a, b) => this.rFormat[p(a)] - this.rFormat[p(b)] || artistOrder(a, b),
      bitrate: (a, b) => a.bitrate - b.bitrate || artistOrder(a, b),
      disc: (a, b) => a.discNr - b.discNr || artistOrder(a, b),
      track: (a, b) => a.trackNr - b.trackNr || artistOrder(a, b),
      path: (a, b) => collator.compare(a.location ?? "", b.location ?? ""),
      order: () => 0,
    };
    const cmp = primary[key];
    return [...tracks].sort((a, b) => dir * cmp(a, b));
  }
}

/** All browser columns, broadest first (the order they're shown in). */
export const BROWSE_FIELDS: { field: BrowseField; title: string; plural: [string, string] }[] = [
  { field: "genre", title: t("Genre"), plural: [t("Genre"), t("Genres")] },
  { field: "format", title: t("Format"), plural: [t("Format"), t("Formats")] },
  { field: "year", title: t("Year"), plural: [t("Year"), t("Years")] },
  { field: "composer", title: t("Composer"), plural: [t("Composer"), t("Composers")] },
  { field: "artist", title: t("Artist"), plural: [t("Artist"), t("Artists")] },
  { field: "album", title: t("Album"), plural: [t("Album"), t("Albums")] },
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
    let items = [...maps[i].entries()].sort((a, b) => collator.compare(a[1], b[1])).map(([v]) => v);
    if (field === "artist" && items.includes(COMPILATIONS)) items = [...items.filter((v) => v !== COMPILATIONS), COMPILATIONS];
    return { field, items };
  });
  return { columns, tracks: out };
}

// ---- Albums ----

/** Sort key of the artist an album is filed under (album artist, else artist). */
export function albumArtistSortKey(t: Track): string {
  if (t.albumArtist) return t.sortAlbumArtist ?? stripThe(t.albumArtist);
  return t.sortArtist ?? stripThe(t.artist ?? "");
}

/** Songs belong to the same album when album and album artist match (any artist, for compilations). */
export function albumKey(t: Track): string {
  const album = t.album ?? "";
  return t.compilation ? `c\u0000${album}` : `a\u0000${album}\u0000${t.albumArtist || t.artist || ""}`;
}

export interface Album {
  key: string;
  title: string;
  artist: string;
  year: number;
  compilation: boolean;
  /** In disc and track order. */
  tracks: Track[];
  /** A song with a cover, if any. */
  coverTrack: Track | null;
  sortArtist: string;
  sortTitle: string;
}

/** Albums of `tracks`: album artist, then year, then title; compilations last. */
export function albums(tracks: Track[]): Album[] {
  const byKey = new Map<string, Album>();
  for (const t of tracks) {
    const key = albumKey(t);
    let a = byKey.get(key);
    if (!a) {
      a = {
        key,
        title: t.album || UNKNOWN_ALBUM,
        artist: t.compilation ? VARIOUS : t.albumArtist || t.artist || UNKNOWN_ARTIST,
        year: 0,
        compilation: t.compilation,
        tracks: [],
        coverTrack: null,
        sortArtist: albumArtistSortKey(t),
        sortTitle: t.sortAlbum ?? t.album ?? "",
      };
      byKey.set(key, a);
    }
    a.tracks.push(t);
    if (t.year && (!a.year || t.year < a.year)) a.year = t.year;
    if (!a.coverTrack && (t.art || t.hasCover)) a.coverTrack = t;
  }
  const list = [...byKey.values()];
  for (const a of list) {
    a.tracks.sort((x, y) => x.discNr - y.discNr || x.trackNr - y.trackNr || collator.compare(x.title ?? "", y.title ?? ""));
  }
  return list.sort(compareAlbums);
}

export function compareAlbums(a: Album, b: Album): number {
  return (
    Number(a.compilation) - Number(b.compilation) ||
    (a.compilation ? 0 : collator.compare(a.sortArtist, b.sortArtist)) ||
    a.year - b.year ||
    collator.compare(a.sortTitle, b.sortTitle)
  );
}

/** Names to list in the Artists, Genres or Composers view, with their songs. */
export function groups(tracks: Track[], by: "artist" | "genre" | "composer"): { name: string; tracks: Track[] }[] {
  const m = new Map<string, { name: string; sort: string; tracks: Track[] }>();
  for (const t of tracks) {
    let name: string;
    let sort: string;
    if (by === "artist") {
      // Compilations (Various Artists, soundtracks) go together, at the end.
      name = t.compilation ? COMPILATIONS : t.albumArtist || t.artist || UNKNOWN_ARTIST;
      sort = t.compilation ? "\uffff" : albumArtistSortKey(t) || name;
    } else if (by === "genre") {
      name = browseGenre(t);
      sort = name;
    } else {
      if (!t.composer) continue;
      name = t.composer;
      sort = t.sortComposer ?? name;
    }
    let g = m.get(name);
    if (!g) m.set(name, (g = { name, sort, tracks: [] }));
    g.tracks.push(t);
  }
  return [...m.values()].sort((a, b) => collator.compare(a.sort, b.sort)).map(({ name, tracks }) => ({ name, tracks }));
}
