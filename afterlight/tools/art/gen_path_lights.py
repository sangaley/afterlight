#!/usr/bin/env python3
"""Art for the walkway path lights (src/vfx/path_lights.rs).

Writes three white masks into assets/sprites/path_light/, tinted in game:
  post.png  - the lamp post seen from above: a dark housing, a lit lens
  pool.png  - its light on the deck, a soft round falloff (alpha only)
  strip.png - a floor strip along a wall, brightest at its left end and
              fading out to the right, like an aircraft's floor path light

Run from the crate root:  python3 tools/art/gen_path_lights.py
"""
from pathlib import Path

import numpy as np
from PIL import Image

OUT = Path(__file__).resolve().parents[2] / "assets/sprites/path_light"


def save(rgba: np.ndarray, name: str) -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    Image.fromarray(np.clip(rgba, 0, 255).astype(np.uint8)).save(OUT / name)


def post(n: int = 48) -> np.ndarray:
    y, x = np.mgrid[0:n, 0:n] + 0.5
    r = np.hypot(x - n / 2, y - n / 2) / (n / 2)
    a = np.clip((1.0 - r) * 6.0, 0, 1)  # the housing's edge, anti-aliased
    # Dark rim, then the lens: white, hottest in the middle.
    lens = np.clip((0.62 - r) * 8.0, 0, 1)
    v = 70 + (255 - 70) * lens
    v = v * (1.0 - 0.25 * np.clip((r - 0.62) * 4, 0, 1))
    return np.dstack([v, v, v, a * 255])


def pool(n: int = 128) -> np.ndarray:
    y, x = np.mgrid[0:n, 0:n] + 0.5
    r = np.hypot(x - n / 2, y - n / 2) / (n / 2)
    a = np.clip(1.0 - r, 0, 1) ** 2.2
    w = np.full_like(a, 255.0)
    return np.dstack([w, w, w, a * 255])


def strip(w: int = 96, h: int = 12) -> np.ndarray:
    y, x = np.mgrid[0:h, 0:w] + 0.5
    across = np.clip(1.0 - np.abs(y - h / 2) / (h / 2), 0, 1) ** 1.5
    along = np.clip(1.0 - x / w, 0, 1) ** 1.3
    # Little LEDs along it, not one smooth tube.
    leds = 0.65 + 0.35 * (0.5 + 0.5 * np.cos(x / w * 2 * np.pi * 5))
    a = across * along * leds
    v = np.full_like(a, 255.0)
    return np.dstack([v, v, v, a * 255])


if __name__ == "__main__":
    save(post(), "post.png")
    save(pool(), "pool.png")
    save(strip(), "strip.png")
    print("wrote", OUT)


# --- Deck variation (src/vfx/deck.rs) -----------------------------------

def runner(n: int = 64) -> np.ndarray:
    """A corridor's centre lane, running along x: a darker inset band with a
    worn, lighter line down each edge. Tinted and faded in game."""
    y, x = np.mgrid[0:n, 0:n] + 0.5
    d = np.abs(y - n / 2) / (n / 2)  # 0 at the lane's centre, 1 at the cell edge
    band = np.clip((0.42 - d) * 30, 0, 1)
    edge = np.clip(1 - np.abs(d - 0.42) * 40, 0, 1)
    v = 255 * edge  # the edges light, the band dark
    a = np.maximum(band * 0.85, edge * 0.6)
    return np.dstack([v, v, v, a * 255])


def shade(w: int = 64, h: int = 24) -> np.ndarray:
    """Shadow at the foot of a wall: dark at row 0 (the wall), gone by the
    bottom."""
    y, x = np.mgrid[0:h, 0:w] + 0.5
    a = np.clip(1 - y / h, 0, 1) ** 1.8
    z = np.zeros_like(a)
    return np.dstack([z, z, z, a * 255])


if __name__ == "__main__":
    out = Path(__file__).resolve().parents[2] / "assets/sprites/deck"
    out.mkdir(parents=True, exist_ok=True)
    for name, img in (("runner.png", runner()), ("shade.png", shade())):
        Image.fromarray(np.clip(img, 0, 255).astype(np.uint8)).save(out / name)
    print("wrote", out)
