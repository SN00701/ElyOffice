#!/usr/bin/env python3
"""Writes assets/icons/mpelyoffice.ico.

The mark is a solid rounded square in Ely blue with a white circle and a
45-degree band, drawn at four times the size and sampled down, so the edges
are smooth at 16 pixels. No image library is needed.
"""

import struct
from pathlib import Path

SIZES = (16, 24, 32, 48, 64, 128, 256)
SCALE = 4
BLUE = (0x2F, 0x6F, 0xED)
WHITE = (0xFF, 0xFF, 0xFF)


def rounded(x: float, y: float, size: float, radius: float) -> bool:
    """Whether the point is inside the rounded square."""
    left, top = radius, radius
    right, bottom = size - radius, size - radius
    cx = min(max(x, left), right)
    cy = min(max(y, top), bottom)
    return (x - cx) ** 2 + (y - cy) ** 2 <= radius**2


def in_circle(x: float, y: float, size: float) -> bool:
    cx = cy = size * 0.38
    radius = size * 0.19
    return (x - cx) ** 2 + (y - cy) ** 2 <= radius**2


def in_band(x: float, y: float, size: float) -> bool:
    """A 45-degree band across the lower right."""
    offset = (x - y) - size * 0.22
    return abs(offset) <= size * 0.075


def pixel(x: int, y: int, size: int) -> tuple[int, int, int, int]:
    """The colour of one pixel, sampled at four times the size."""
    hits = 0
    white = 0
    for sy in range(SCALE):
        for sx in range(SCALE):
            fx = x + (sx + 0.5) / SCALE
            fy = y + (sy + 0.5) / SCALE
            if not rounded(fx, fy, size, size * 0.22):
                continue
            hits += 1
            if in_circle(fx, fy, size) or in_band(fx, fy, size):
                white += 1
    if hits == 0:
        return (0, 0, 0, 0)
    alpha = round(255 * hits / (SCALE * SCALE))
    if white * 2 >= hits:
        return (WHITE[0], WHITE[1], WHITE[2], alpha)
    return (BLUE[0], BLUE[1], BLUE[2], alpha)


def bitmap(size: int) -> bytes:
    """One 32-bit BGRA image with its mask, bottom-up as the format wants."""
    pixels = bytearray()
    mask = bytearray()
    for y in range(size - 1, -1, -1):
        for x in range(size):
            red, green, blue, alpha = pixel(x, y, size)
            pixels += bytes((blue, green, red, alpha))
    row = ((size + 31) // 32) * 4
    mask += bytes(row) * size
    header = struct.pack(
        "<IiiHHIIiiII",
        40,
        size,
        size * 2,
        1,
        32,
        0,
        len(pixels) + len(mask),
        0,
        0,
        0,
        0,
    )
    return header + bytes(pixels) + bytes(mask)


def main() -> None:
    root = Path(__file__).resolve().parents[2]
    images = [bitmap(size) for size in SIZES]
    offset = 6 + 16 * len(images)
    entries = bytearray()
    for size, image in zip(SIZES, images):
        entries += struct.pack(
            "<BBBBHHII",
            0 if size >= 256 else size,
            0 if size >= 256 else size,
            0,
            0,
            1,
            32,
            len(image),
            offset,
        )
        offset += len(image)
    ico = struct.pack("<HHH", 0, 1, len(images)) + bytes(entries) + b"".join(images)
    target = root / "assets" / "icons" / "mpelyoffice.ico"
    target.write_bytes(ico)
    print(f"{target}: {len(ico)} bytes, sizes {SIZES}")


if __name__ == "__main__":
    main()
