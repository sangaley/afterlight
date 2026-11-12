#!/usr/bin/env python3
"""Procedural nebula for the main menu, one per playthrough (the seed).

Built the way NASA's pictures are coloured, not the way the eye would see
them: three emission gases (sulfur, hydrogen, oxygen) are separate noise
layers, mapped to colour by a palette, as the Hubble palette maps SII, H-alpha
and OIII to red, green and blue. Dark dust is ridged noise that eats the gas
and catches light along one edge, like the Cosmic Cliffs in Carina. Stars on
top, the brightest with diffraction spikes (six for Webb, four for Hubble).

The Cascade drains it: `drain` greys and darkens everything past a front
that comes in from the far side, leaving a pocket of colour round the player.

    python3 nebula.py <seed> <out.png> [--drain 0..1]
"""
import argparse
import math

import numpy as np
from PIL import Image

W, H = 1280, 720

# name, colour of each gas (sulfur, hydrogen, oxygen), dust colour, spikes
PALETTES = [
    ("hubble", [(1.00, 0.62, 0.22), (0.95, 0.80, 0.40), (0.20, 0.62, 0.95)], (0.30, 0.16, 0.08), 4),
    ("cliffs", [(0.95, 0.48, 0.20), (0.85, 0.58, 0.32), (0.30, 0.55, 1.00)], (0.36, 0.20, 0.10), 6),
    ("hoo", [(0.95, 0.22, 0.30), (0.90, 0.30, 0.38), (0.25, 0.85, 0.95)], (0.20, 0.08, 0.10), 4),
    ("veil", [(0.70, 0.30, 0.95), (0.95, 0.35, 0.65), (0.30, 0.75, 0.90)], (0.16, 0.08, 0.20), 6),
]


def noise(rng, scale, octaves=6, persistence=0.55):
    """Fractal value noise: random grids upsampled smooth and summed."""
    out = np.zeros((H, W), np.float32)
    amp, total = 1.0, 0.0
    for o in range(octaves):
        gw = max(2, int(W / scale * 2 ** o))
        gh = max(2, int(H / scale * 2 ** o))
        g = rng.random((gh, gw)).astype(np.float32)
        layer = np.asarray(Image.fromarray(g).resize((W, H), Image.BICUBIC))
        out += amp * layer
        total += amp
        amp *= persistence
    return out / total


def warp(field, dx, dy, strength):
    """Sample `field` at positions pushed by two other fields (domain warp)."""
    ys, xs = np.mgrid[0:H, 0:W].astype(np.float32)
    x = np.clip(xs + (dx - 0.5) * strength, 0, W - 1).astype(np.int32)
    y = np.clip(ys + (dy - 0.5) * strength, 0, H - 1).astype(np.int32)
    return field[y, x]


def box(a, r, axis):
    """Box blur of radius r along one axis (running sums)."""
    if r < 1:
        return a
    pad = [(0, 0), (0, 0)]
    pad[axis] = (r + 1, r)
    c = np.cumsum(np.pad(a, pad, mode="edge"), axis=axis)
    hi = np.take(c, np.arange(2 * r + 1, c.shape[axis]), axis=axis)
    lo = np.take(c, np.arange(0, c.shape[axis] - 2 * r - 1), axis=axis)
    return (hi - lo) / (2 * r + 1)


def blur(a, r):
    """Close to a Gaussian of radius r: three box passes each way."""
    a = a.astype(np.float32)
    k = max(1, int(round(r * 0.6)))
    for _ in range(3):
        a = box(box(a, k, 0), k, 1)
    return a


def nebula(seed, drain=0.0):
    rng = np.random.default_rng(seed)
    name, gases, dust_col, spikes = PALETTES[seed % len(PALETTES)]

    # Where the cloud sits: right of centre, a tilted blob, so the menu's
    # left side stays dark for the title.
    ys, xs = np.mgrid[0:H, 0:W].astype(np.float32)
    cx, cy = W * rng.uniform(0.66, 0.76), H * rng.uniform(0.42, 0.58)
    ang = rng.uniform(-0.6, 0.6)
    u = ((xs - cx) * math.cos(ang) + (ys - cy) * math.sin(ang)) / (W * rng.uniform(0.30, 0.40))
    v = (-(xs - cx) * math.sin(ang) + (ys - cy) * math.cos(ang)) / (H * rng.uniform(0.38, 0.52))
    shape = np.exp(-(u * u + v * v) * 1.6)

    wx, wy = noise(rng, 300, 4), noise(rng, 300, 4)
    emission = []
    for i in range(3):
        n = warp(noise(rng, rng.uniform(160, 260), 7), wx, wy, rng.uniform(160, 260))
        n = np.clip((n - 0.32) * 2.1, 0, None) ** rng.uniform(1.5, 2.1)
        emission.append(n * shape * rng.uniform(0.8, 1.25))

    # Dust: ridged noise, thickest where it folds; lit along its upper edge.
    d = warp(noise(rng, 220, 7), wy, wx, 220)
    ridges = 1.0 - np.abs(d * 2 - 1)
    dust = blur(np.clip((ridges - 0.66) * 3.2, 0, 1) ** 1.4 * np.clip(shape * 1.6, 0, 1), 2)
    rim = np.clip(dust - np.roll(blur(dust, 3), 4, axis=0), 0, 1) * 2.2

    rgb = np.zeros((H, W, 3), np.float32)
    for n, c in zip(emission, gases):
        rgb += n[..., None] * np.array(c, np.float32)
    rgb *= (1.0 - dust * 0.9)[..., None]
    rgb += (dust * 0.08)[..., None] * np.array(dust_col, np.float32)
    rgb += (rim * shape)[..., None] * np.array(gases[0], np.float32) * 0.6
    glow = blur(sum(emission), 40)
    rgb += glow[..., None] * np.array(gases[2], np.float32) * 0.3
    haze = blur(sum(emission), 14)
    rgb += haze[..., None] * np.array(gases[1], np.float32) * 0.08

    # Tone map, then darken the left so the title reads.
    rgb = 1.0 - np.exp(-rgb * 1.6)
    grey = rgb.mean(axis=2, keepdims=True)
    rgb = np.clip(grey + (rgb - grey) * 1.55, 0, 1)
    rgb *= np.clip((xs / W - 0.18) * 1.9, 0.08, 1.0)[..., None]

    # Stars: most faint, a few bright, the brightest with spikes.
    stars = np.zeros((H, W), np.float32)
    count = 4200
    sx, sy = rng.integers(0, W, count), rng.integers(0, H, count)
    mag = rng.pareto(2.2, count).astype(np.float32) * 0.18 + 0.05
    stars[sy, sx] = np.clip(mag, 0, 3)
    starlight = stars * 0.9 + blur(stars, 1.6) * 1.4
    bright = [i for i in np.argsort(mag)[::-1] if sx[i] > W * 0.45][:5]
    for i in bright:
        x0, y0, m = int(sx[i]), int(sy[i]), float(mag[i])
        L = int(10 + 22 * min(m, 2.0))
        for k in range(spikes):
            a = math.pi * k / (spikes / 2) + (math.pi / 2 if spikes == 4 else 0)
            for t in range(1, L):
                px, py = int(x0 + math.cos(a) * t), int(y0 + math.sin(a) * t)
                if 0 <= px < W and 0 <= py < H:
                    starlight[py, px] += m * 0.35 * (1 - t / L) ** 2
    tint = np.array((0.92, 0.95, 1.0), np.float32)
    rgb = 1.0 - (1.0 - rgb) * (1.0 - np.clip(starlight, 0, 1)[..., None] * tint)

    if drain > 0:
        rgb = cascade(rgb, drain, rng)
    return Image.fromarray((np.clip(rgb, 0, 1) * 255).astype(np.uint8)), name


def cascade(rgb, drain, rng):
    """The dark comes in from the right: grey, then black, with a faint
    violet seam at its front. A pocket of colour stays round the player."""
    ys, xs = np.mgrid[0:H, 0:W].astype(np.float32)
    front = W * (1.0 - drain * 0.95)
    edge = noise(rng, 120, 4) * 140
    t = np.clip((xs - front + edge) / 160.0, 0, 1)
    if drain > 0.6:
        px, py = W * 0.6, H * 0.56
        pocket = np.exp(-(((xs - px) / 150) ** 2 + ((ys - py) / 120) ** 2))
        t = np.minimum(t, 1 - pocket)
    grey = rgb.mean(axis=2, keepdims=True)
    out = rgb * (1 - t[..., None]) + grey * 0.16 * t[..., None]
    seam = np.exp(-((xs - front + edge) / 24.0) ** 2) * 0.10
    return out + seam[..., None] * np.array((0.45, 0.32, 0.75), np.float32)


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("seed", type=int)
    p.add_argument("out")
    p.add_argument("--drain", type=float, default=0.0)
    a = p.parse_args()
    img, name = nebula(a.seed, a.drain)
    img.save(a.out)
    print(a.out, name)
