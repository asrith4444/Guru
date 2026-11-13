#!/usr/bin/env python3
"""Draw Guru's app icon (no image libraries needed) into app/icons/."""
import math, os, struct, zlib

OUT = os.path.join(os.path.dirname(__file__), "..", "app", "icons")

def png(size):
    rows = []
    c = (size - 1) / 2
    for y in range(size):
        row = bytearray([0])
        for x in range(size):
            # Rounded square with a diagonal indigo→teal gradient.
            r = size * 0.22
            dx = max(abs(x - c) - (c - r), 0)
            dy = max(abs(y - c) - (c - r), 0)
            inside = math.hypot(dx, dy) <= r
            if not inside:
                row += bytes([0, 0, 0, 0]); continue
            t = (x + y) / (2 * size)
            col = (int(79 + (20 - 79) * t), int(70 + (184 - 70) * t), int(229 + (166 - 229) * t))
            # A white open ring (the "G" / an orbit) with a dot: a path and a goal.
            d = math.hypot(x - c, y - c) / size
            ang = math.atan2(y - c, x - c)
            ring = 0.23 < d < 0.31 and not (-0.6 < ang < 0.35)
            bar = abs(y - c) < size * 0.04 and c - size * 0.02 < x < c + size * 0.27
            dot = math.hypot(x - (c + size * 0.27), y - (c - size * 0.20)) < size * 0.055
            if ring or bar or dot:
                col = (255, 255, 255)
            row += bytes([*col, 255])
        rows.append(bytes(row))
    raw = b"".join(rows)
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)) + \
        chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")

def ico(pngs):
    header = struct.pack("<HHH", 0, 1, len(pngs))
    offset = 6 + 16 * len(pngs)
    entries, data = b"", b""
    for size, blob in pngs:
        entries += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(blob), offset + len(data))
        data += blob
    return header + entries + data

os.makedirs(OUT, exist_ok=True)
for name, size in [("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256), ("icon.png", 512)]:
    open(os.path.join(OUT, name), "wb").write(png(size))
open(os.path.join(OUT, "icon.ico"), "wb").write(ico([(32, png(32)), (64, png(64)), (256, png(256))]))
print("icons written")
