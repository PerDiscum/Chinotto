#!/usr/bin/env python3
"""Writes app/src-tauri/licenses/THIRD-PARTY-LICENSES.txt: every piece of
other people's software that goes into Chinotto, its licence, and the
licence texts (each text once). The file ships inside the app.

    python3 scripts/licenses.py        (needs cargo and app/node_modules)

Run it after changing dependencies; CI runs it before every build.
"""
import hashlib, json, os, re, subprocess, sys, tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "app/src-tauri/licenses/THIRD-PARTY-LICENSES.txt"
LICENCE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE|COPYRIGHT)([-_.].*)?$", re.I)

texts = {}  # hash -> (text, [users])


def add_text(text, user):
    text = text.strip().replace("\r\n", "\n")
    h = hashlib.sha256(text.encode()).hexdigest()[:12]
    texts.setdefault(h, (text, []))[1].append(user)
    return h


def licence_files(folder):
    folder = Path(folder)
    found = sorted(p for p in folder.iterdir() if p.is_file() and LICENCE_FILE.match(p.name)) if folder.is_dir() else []
    return [p.read_text(errors="replace") for p in found]


entries = []  # (name, licence, where the source is, [text hashes])
missing = []  # (index in entries, licence expression): crates without a licence file

# Rust crates (everything cargo may build in, for any platform).
meta = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--format-version", "1", "--manifest-path", str(ROOT / "app/src-tauri/Cargo.toml")], text=True))
members = set(meta["workspace_members"])
for p in sorted(meta["packages"], key=lambda p: (p["name"], p["version"])):
    if p["id"] in members:
        continue
    user = f'{p["name"]} {p["version"]}'
    hashes = [add_text(t, user) for t in licence_files(Path(p["manifest_path"]).parent)]
    lic = p.get("license") or "see its licence file"
    if not hashes:
        # No licence file in the package: the standard text of its licence (below), and its authors.
        authors = ", ".join(a.split(" <")[0] for a in p.get("authors") or []) or f"the {p['name']} authors"
        lic += f" (Copyright: {authors})"
        missing.append((len(entries), p.get("license") or ""))
    entries.append((user, lic, f'https://crates.io/crates/{p["name"]}/{p["version"]}', hashes))

# Standard texts for crates without a licence file, taken from crates that have them.
signatures = {
    "MIT": "Permission is hereby granted, free of charge",
    "Apache-2.0": "Apache License\n                           Version 2.0",
    "Zlib": "This software is provided 'as-is'",
    "BSL-1.0": "Boost Software License - Version 1.0",
    "BSD-3-Clause": "Neither the name of",
    "MPL-2.0": "Mozilla Public License Version 2.0",
}
reference = {}
for h, (text, _) in texts.items():
    for spdx, sig in signatures.items():
        if spdx not in reference and sig in text and len(text) < 40000:
            reference[spdx] = h
for i, expr in missing:
    ids = re.findall(r"[A-Za-z0-9.\-]+", expr.replace(" OR ", " ").replace(" AND ", " "))
    pick = next((x for x in ("MIT", "Apache-2.0", "Zlib", "BSL-1.0", "BSD-3-Clause", "MPL-2.0") if x in ids and x in reference), None)
    if pick:
        name, lic, where, _ = entries[i]
        entries[i] = (name, lic, where, [reference[pick]])
        texts[reference[pick]][1].append(name)
    else:
        print(f"warning: no licence text for {entries[i][0]} ({expr})")

# LAME, the MP3 encoder, inside mp3lame-sys (its own licence files).
lame = next((p for p in meta["packages"] if p["name"] == "mp3lame-sys"), None)
if lame:
    d = next(Path(lame["manifest_path"]).parent.glob("lame-*"), None)
    if d:
        entries.append((f"LAME {d.name.removeprefix('lame-')} (in mp3lame-sys)", "LGPL-2.0-or-later",
                        f'https://crates.io/crates/mp3lame-sys/{lame["version"]} ({d.name})', [add_text(t, "LAME") for t in licence_files(d)]))
# The LGPL 3 is a set of additions to the GPL 3, so both texts go with it.
if any("LGPL-3.0" in e[1] for e in entries):
    gpl3 = (ROOT / "third_party/licenses/GPL-3.0.txt").read_text()
    entries.append(("GNU GPL 3 (part of the LGPL 3 terms above)", "GPL-3.0", "https://www.gnu.org/licenses/gpl-3.0.txt", [add_text(gpl3, "GPL 3")]))

# The window's interface: what's bundled from npm.
node = ROOT / "app/node_modules"
for dep in json.loads((ROOT / "app/package.json").read_text())["dependencies"].keys() | {"svelte"}:
    pj = node / dep / "package.json"
    if not pj.exists():
        sys.exit(f"{pj} is missing: run npm ci in app/ first")
    info = json.loads(pj.read_text())
    user = f'{dep} {info["version"]} (npm)'
    entries.append((user, info.get("license", "?"), f"https://www.npmjs.com/package/{dep}/v/{info['version']}",
                    [add_text(t, user) for t in licence_files(node / dep)]))

# C libraries built into Chinotto from this repository.
vendored = [
    ("libgpod", "LGPL-2.1-or-later", "crates/chinotto-gpod/libgpod"),
    ("PodBox (in libgpod)", "MIT", "crates/chinotto-gpod/libgpod/src/podbox"),
    ("HashAB (in libgpod)", "Unlicense", "crates/chinotto-gpod/libgpod/src/hashab"),
    ("libmtp", "LGPL-2.1-or-later", "crates/chinotto-mtp/libmtp"),
    ("libusb", "LGPL-2.1-or-later", "crates/chinotto-mtp/libusb"),
    ("Handjet typeface (Rosetta Type)", "OFL-1.1", "app/src/assets/fonts"),
]
for name, lic, folder in vendored:
    entries.append((name, lic, f"this repository, {folder}", [add_text(t, name) for t in licence_files(ROOT / folder)]))

# cdrdao: a separate program in the macOS app, from the unmodified release.
cdrdao = next((ROOT / "third_party/sources").glob("cdrdao-*.tar.bz2"))
with tarfile.open(cdrdao) as tf:
    copying = next(m for m in tf.getmembers() if m.name.count("/") == 1 and m.name.endswith("/COPYING"))
    gpl = tf.extractfile(copying).read().decode(errors="replace")
entries.append((cdrdao.name.removesuffix(".tar.bz2") + " (separate program, unmodified; macOS app)", "GPL-2.0-or-later",
                f"this repository, third_party/sources/{cdrdao.name}, and attached to each release", [add_text(gpl, "cdrdao")]))

# glib: built from GNOME's release into the macOS app (scripts/build-glib-macos.sh).
glib = re.search(r'GLIB_VERSION="\$\{GLIB_VERSION:-([0-9.]+)\}"', (ROOT / "scripts/build-glib-macos.sh").read_text()).group(1)
lgpl21 = next(t for t, users in texts.values() if "libgpod" in users)
entries.append((f"glib {glib} (macOS app), with proxy-libintl, pcre2, libffi and zlib", "LGPL-2.1-or-later (glib, proxy-libintl); BSD-3-Clause (pcre2); MIT (libffi); Zlib (zlib)",
                f"https://download.gnome.org/sources/glib/ (glib-{glib}.tar.xz, also attached to each release)", [add_text(lgpl21, "glib")]))

lines = [
    "Chinotto: third-party software and licences",
    "=" * 44,
    "",
    "Chinotto itself is released under the MIT licence (see LICENSE in its",
    "repository, https://github.com/PerDiscum/Chinotto). It is built with the",
    "software below, each under its own licence.",
    "",
    "LGPL libraries (libgpod, libmtp, libusb, glib, LAME through mp3lame-sys) are",
    "built into Chinotto. You may replace them with modified versions: Chinotto's",
    "complete source, including theirs, is in its repository, and the build",
    "scripts there rebuild Chinotto with whatever versions you put in their place.",
    "",
    "cdrdao (GPL) is a separate program that the macOS app carries and runs to",
    "burn .bin/.cue images; Chinotto doesn't link it. Its unmodified source is in",
    "the repository and attached to each release, as is glib's.",
    "",
    "Source for each item: where it says. Licence texts follow the list.",
    "",
]
for name, lic, where, hashes in entries:
    lines.append(f"- {name}: {lic}")
    lines.append(f"  Source: {where}")
    if hashes:
        lines.append(f"  Licence text: {', '.join(f'[{h}]' for h in dict.fromkeys(hashes))}")
lines += ["", "", "Licence texts", "=" * 13, ""]
for h, (text, users) in texts.items():
    lines += [f"[{h}]", "-" * 72, text, "", ""]

OUT.parent.mkdir(parents=True, exist_ok=True)
OUT.write_text("\n".join(lines) + "\n")
print(f"{OUT.relative_to(ROOT)}: {len(entries)} components, {len(texts)} licence texts, {OUT.stat().st_size // 1024} KB")
