"""Erzeugt assets/icon.ico (Play-Button-Logo) und assets/icon.png fuer Playtube.

Nur ein Build-Hilfsskript - Pillow wird dafuer benoetigt (nicht Laufzeit-Abhaengigkeit
der App selbst): pip install Pillow
Aufruf: python tools/generate_icon.py
"""
from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets"
ASSETS.mkdir(exist_ok=True)

SIZE = 512
BG_COLOR = (124, 58, 237, 255)      # violett - eigenstaendige Playtube-Marke
BG_COLOR_2 = (236, 72, 153, 255)    # pink, fuer diagonalen Verlauf
PLAY_COLOR = (255, 255, 255, 255)


def make_base_image() -> Image.Image:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    px = img.load()
    for y in range(SIZE):
        for x in range(SIZE):
            t = (x + y) / (2 * SIZE)
            r = int(BG_COLOR[0] + (BG_COLOR_2[0] - BG_COLOR[0]) * t)
            g = int(BG_COLOR[1] + (BG_COLOR_2[1] - BG_COLOR[1]) * t)
            b = int(BG_COLOR[2] + (BG_COLOR_2[2] - BG_COLOR[2]) * t)
            px[x, y] = (r, g, b, 255)

    # Abgerundetes Quadrat als Maske (App-Icon-Look).
    mask = Image.new("L", (SIZE, SIZE), 0)
    mdraw = ImageDraw.Draw(mask)
    radius = int(SIZE * 0.22)
    mdraw.rounded_rectangle([0, 0, SIZE - 1, SIZE - 1], radius=radius, fill=255)

    out = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    out.paste(img, (0, 0), mask)

    draw = ImageDraw.Draw(out)
    # Play-Dreieck mittig.
    w = SIZE * 0.34
    h = SIZE * 0.4
    cx, cy = SIZE / 2 + SIZE * 0.03, SIZE / 2
    points = [
        (cx - w / 2, cy - h / 2),
        (cx - w / 2, cy + h / 2),
        (cx + w / 2, cy),
    ]
    draw.polygon(points, fill=PLAY_COLOR)
    return out


def main() -> None:
    base = make_base_image()
    base.save(ASSETS / "icon.png")
    sizes = [16, 24, 32, 48, 64, 128, 256]
    base.save(ASSETS / "icon.ico", sizes=[(s, s) for s in sizes])
    print(f"Icon geschrieben nach {ASSETS / 'icon.ico'}")


if __name__ == "__main__":
    main()
