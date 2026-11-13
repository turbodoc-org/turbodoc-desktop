#!/usr/bin/env bash
# Makes every icon the installers need from the 1024 px master (assets/icon-1024.png):
# PNGs for Linux, Turbodoc.icns for macOS and Turbodoc.ico for Windows, in assets/icons/.
# Run it on a Mac (sips and iconutil) after changing the master; the results are checked in.
set -euo pipefail
cd "$(dirname "$0")/.."

SRC=assets/icon-1024.png
OUT=assets/icons
mkdir -p "$OUT"
for s in 32 64 128 256 512; do
  sips -z $s $s "$SRC" --out "$OUT/${s}x${s}.png" >/dev/null
done

ICONSET=$(mktemp -d)/Turbodoc.iconset
mkdir -p "$ICONSET"
for s in 16 32 128 256 512; do
  sips -z $s $s "$SRC" --out "$ICONSET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) "$SRC" --out "$ICONSET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$OUT/Turbodoc.icns"

# An .ico holds PNGs as they are (Windows Vista and later), so no image library is needed.
TMP=$(mktemp -d)
for s in 16 24 32 48 64 128 256; do sips -z $s $s "$SRC" --out "$TMP/$s.png" >/dev/null; done
python3 -I - "$TMP" "$OUT/Turbodoc.ico" <<'PY'
import struct, sys, pathlib
src, out = pathlib.Path(sys.argv[1]), sys.argv[2]
sizes = [16, 24, 32, 48, 64, 128, 256]
images = [(src / f"{s}.png").read_bytes() for s in sizes]
offset = 6 + 16 * len(sizes)
head = struct.pack("<HHH", 0, 1, len(sizes))
entries = b""
for s, data in zip(sizes, images):
    entries += struct.pack("<BBBBHHII", s % 256, s % 256, 0, 0, 1, 32, len(data), offset)
    offset += len(data)
pathlib.Path(out).write_bytes(head + entries + b"".join(images))
PY
echo "Wrote $OUT"
