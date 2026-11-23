"""Draws the Mod UI app icon (three stacked cards, the top one being dragged) with the standard library: python make_icons.py"""
import os
import struct
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
GREEN, DARK, CREAM, GOLD = (47, 111, 79), (22, 22, 21), (246, 245, 242), (251, 231, 166)


def inside(x, y, x0, y0, x1, y1, r):
    if x < x0 or x > x1 or y < y0 or y > y1:
        return False
    cx, cy = min(max(x, x0 + r), x1 - r), min(max(y, y0 + r), y1 - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r


def pixel(x, y):
    """x, y in 0..1; the whole square is filled (a maskable icon keeps its art inside the middle 80%)"""
    if not inside(x, y, 0.0, 0.0, 1.0, 1.0, 0.0):
        return DARK
    c = GREEN
    for (y0, y1, col) in ((0.30, 0.42, CREAM), (0.47, 0.59, CREAM), (0.64, 0.76, CREAM)):
        if inside(x, y, 0.24, y0, 0.76, y1, 0.04):
            c = col
    if inside(x, y, 0.27, 0.285, 0.73, 0.435, 0.05):
        c = GOLD
    for dy in (0.325, 0.36, 0.395):
        if inside(x, y, 0.62, dy - 0.006, 0.70, dy + 0.006, 0.006):
            c = GREEN
    return c


def png(size):
    ss = 3
    rows = []
    for j in range(size):
        row = bytearray([0])
        for i in range(size):
            r = g = b = 0
            for sj in range(ss):
                for si in range(ss):
                    p = pixel((i + (si + .5) / ss) / size, (j + (sj + .5) / ss) / size)
                    r += p[0]; g += p[1]; b += p[2]
            n = ss * ss
            row += bytes((r // n, g // n, b // n))
        rows.append(bytes(row))
    raw = b"".join(rows)

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


if __name__ == "__main__":
    for s in (192, 512):
        with open(os.path.join(HERE, "icon-%d.png" % s), "wb") as f:
            f.write(png(s))
        print("icon-%d.png" % s)
