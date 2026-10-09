#!/usr/bin/env python3
"""Draw the listenBli app icon and pack it into `assets/ListenBli.icns`.

The mark is the one from the design's favicon: the Bilibili-pink rounded tile
with three "on air" bars. macOS icons are drawn on a squircle that covers ~82%
of the canvas with a soft drop shadow underneath, so the same shape is used here
rather than a plain rounded rectangle.

    python3 scripts/make-icns.py            # writes assets/icon.png + icns
"""

from __future__ import annotations

import math
import pathlib
import shutil
import subprocess
import sys
import tempfile

from PIL import Image, ImageDraw, ImageFilter

ROOT = pathlib.Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"

CANVAS = 1024
# Apple's icon grid: the artwork occupies 824/1024 of the canvas.
TILE = 824
# Accent ramp from the design tokens (--accent -> --accent-deep).
TOP = (251, 114, 153)
BOTTOM = (217, 87, 138)
BAR_HEIGHTS = (0.44, 0.86, 0.64)


def squircle(size: int, exponent: float = 5.0, supersample: int = 4) -> Image.Image:
    """A superellipse mask: macOS's icon shape, not a plain rounded rectangle."""
    big = size * supersample
    mask = Image.new("L", (big, big), 0)
    draw = ImageDraw.Draw(mask)
    half = big / 2
    # Sample the boundary and fill the polygon; exact enough at this size.
    points = []
    steps = 2048
    for i in range(steps):
        t = 2 * math.pi * i / steps
        cos, sin = math.cos(t), math.sin(t)
        x = math.copysign(abs(cos) ** (2.0 / exponent), cos) * half
        y = math.copysign(abs(sin) ** (2.0 / exponent), sin) * half
        points.append((half + x, half + y))
    draw.polygon(points, fill=255)
    return mask.resize((size, size), Image.LANCZOS)


def vertical_gradient(size: tuple[int, int], top: tuple[int, int, int], bottom: tuple[int, int, int]) -> Image.Image:
    width, height = size
    gradient = Image.new("RGB", (1, height))
    for y in range(height):
        t = y / max(1, height - 1)
        # A slight ease keeps the light in the upper half, as in the design.
        t = t ** 0.85
        gradient.putpixel(
            (0, y),
            tuple(round(top[c] + (bottom[c] - top[c]) * t) for c in range(3)),
        )
    return gradient.resize((width, height), Image.BILINEAR)


def build_master() -> Image.Image:
    tile = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))

    # -- drop shadow --------------------------------------------------------
    shape = squircle(TILE)
    shadow = Image.new("RGBA", (CANVAS, CANVAS), (0, 0, 0, 0))
    shadow.paste((0, 0, 0, 90), ((CANVAS - TILE) // 2, (CANVAS - TILE) // 2 + 14), shape)
    shadow = shadow.filter(ImageFilter.GaussianBlur(26))
    tile = Image.alpha_composite(tile, shadow)

    # -- body ---------------------------------------------------------------
    body = vertical_gradient((TILE, TILE), TOP, BOTTOM).convert("RGBA")
    # Top-left highlight, matching the `.brand__mark` inset ring.
    highlight = Image.new("RGBA", (TILE, TILE), (0, 0, 0, 0))
    ImageDraw.Draw(highlight).ellipse(
        (-TILE * 0.35, -TILE * 0.85, TILE * 0.95, TILE * 0.35),
        fill=(255, 255, 255, 38),
    )
    highlight = highlight.filter(ImageFilter.GaussianBlur(60))
    body = Image.alpha_composite(body, highlight)
    body.putalpha(shape)
    tile.alpha_composite(body, ((CANVAS - TILE) // 2, (CANVAS - TILE) // 2))

    # -- three bars ---------------------------------------------------------
    draw = ImageDraw.Draw(tile)
    baseline = (CANVAS + TILE) // 2 - round(TILE * 0.20)
    bar_w = round(TILE * 0.115)
    gap = round(TILE * 0.062)
    total = bar_w * 3 + gap * 2
    x = (CANVAS - total) // 2
    for height in BAR_HEIGHTS:
        bar_h = round(TILE * 0.42 * height)
        draw.rounded_rectangle(
            (x, baseline - bar_h, x + bar_w, baseline),
            radius=bar_w // 2,
            fill=(255, 255, 255, 244),
        )
        x += bar_w + gap
    return tile


def main() -> int:
    ASSETS.mkdir(parents=True, exist_ok=True)
    master = build_master()
    master.save(ASSETS / "icon.png")

    with tempfile.TemporaryDirectory() as tmp:
        iconset = pathlib.Path(tmp) / "ListenBli.iconset"
        iconset.mkdir()
        for size in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                pixels = size * scale
                name = f"icon_{size}x{size}{'@2x' if scale == 2 else ''}.png"
                master.resize((pixels, pixels), Image.LANCZOS).save(iconset / name)
        target = ASSETS / "ListenBli.icns"
        result = subprocess.run(
            ["iconutil", "-c", "icns", str(iconset), "-o", str(target)],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            print(result.stderr, file=sys.stderr)
            return result.returncode
    print(f"wrote {ASSETS / 'icon.png'} and {ASSETS / 'ListenBli.icns'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
