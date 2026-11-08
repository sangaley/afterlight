#!/usr/bin/env python3
"""Post-render pass: supersample down, then prove the tile actually tiles.

Runs on plain Pillow so it works before ImageMagick is installed; the
downscale and quantise steps move to `magick` once it is available.
"""

import os
import sys
from PIL import Image


def _size(spec):
    """Accept `256` or `794x378`. The original only handled squares, which
    silently distorts every non-square asset in the set."""
    if isinstance(spec, int):
        return spec, spec
    if "x" in str(spec):
        w, h = str(spec).split("x")
        return int(w), int(h)
    n = int(spec)
    return n, n


def downscale(src, dst, size):
    im = Image.open(src).convert("RGBA")
    im = im.resize(_size(size), Image.LANCZOS)
    im.save(dst)
    return im


def whiten(src, dst, alpha_mode="both", keep_warm=0.0):
    """Force RGB to white and carry the shape entirely in alpha.

    Bevy's `Sprite.color` MULTIPLIES the texture, and every spawn site already
    picks a meaningful colour -- the missile trail rolls its own grey per
    puff, `Blast` lerps hot to cool over the explosion's life. Ship a texture
    with colour baked in and that colour gets applied twice, which muddies the
    tint and silently breaks the per-ammo-type colouring the game relies on
    for readability.

    `alpha_mode="both"` folds render luminance into render alpha. Volume alpha
    alone can be near-flat across a lit puff, so this is what keeps the
    internal density structure visible once RGB is thrown away.

    `keep_warm` is the fire exception. Smoke goes to pure white because it is
    multiplied by a per-puff grey and any residual hue fights it. Fire's whole
    visual identity IS the blackbody gradient, so throwing all of it away
    leaves a grey ball. This value-normalises the original RGB first -- so only
    HUE survives, not brightness, and the alpha ramp still carries every bit of
    the shape -- then lerps that far toward white. Low values stay tint-safe.
    """
    im = Image.open(src).convert("RGBA")
    r, g, b, a = im.split()
    px_r, px_g, px_b, px_a = r.load(), g.load(), b.load(), a.load()
    w, h = im.size

    lum = [[0.0] * w for _ in range(h)]
    peak = 0.0
    for y in range(h):
        for x in range(w):
            v = (0.2126 * px_r[x, y] + 0.7152 * px_g[x, y] + 0.0722 * px_b[x, y]) / 255.0
            lum[y][x] = v
            if v > peak:
                peak = v
    if peak <= 0.0:
        raise SystemExit("whiten: %s is empty -- did the sim actually bake?" % src)

    out = Image.new("RGBA", (w, h))
    px_o = out.load()
    for y in range(h):
        for x in range(w):
            av = px_a[x, y] / 255.0
            if alpha_mode == "both":
                av = av * (lum[y][x] / peak)
            elif alpha_mode == "lum":
                av = lum[y][x] / peak
            a8 = int(round(max(0.0, min(1.0, av)) * 255))
            if keep_warm <= 0.0:
                px_o[x, y] = (255, 255, 255, a8)
            else:
                # Value-normalise: divide by the pixel's own max channel so
                # brightness is discarded and only the hue ratio is left.
                r0, g0, b0 = px_r[x, y], px_g[x, y], px_b[x, y]
                m = max(r0, g0, b0)
                if m == 0:
                    px_o[x, y] = (255, 255, 255, a8)
                else:
                    hr, hg, hb = 255.0 * r0 / m, 255.0 * g0 / m, 255.0 * b0 / m
                    k = keep_warm
                    px_o[x, y] = (
                        int(round(255.0 * (1.0 - k) + hr * k)),
                        int(round(255.0 * (1.0 - k) + hg * k)),
                        int(round(255.0 * (1.0 - k) + hb * k)),
                        a8,
                    )
    out.save(dst)
    return out


def normalize(src, dst, size, margin=0.08, floor=6, gain=1.0, keep_rgb=False):
    """Trim to the alpha bounding box, re-square, pad, resize.

    The sim keeps expanding, so frames harvested at f26 and f40 arrive at
    different scales. Shipped as-is they would read as three different sizes
    when the engine draws them all at the same `custom_size`. This makes the
    puff occupy the same fraction of every canvas so the variants are
    interchangeable.

    `margin` keeps a transparent border: a soft edge that touches the canvas
    shows as a straight cut the moment the engine rotates the sprite.
    """
    im = Image.open(src).convert("RGBA")
    a = im.split()[3]
    bbox = a.point(lambda v: 255 if v > floor else 0).getbbox()
    if bbox is None:
        raise SystemExit("normalize: %s has no visible pixels" % src)
    im = im.crop(bbox)

    side = max(im.size)
    sq = Image.new("RGBA", (side, side), (255, 255, 255, 0))
    sq.alpha_composite(im, ((side - im.width) // 2, (side - im.height) // 2))

    padded_side = int(round(side / (1.0 - 2.0 * margin)))
    canvas = Image.new("RGBA", (padded_side, padded_side), (255, 255, 255, 0))
    off = (padded_side - side) // 2
    canvas.alpha_composite(sq, (off, off))

    canvas = canvas.resize(_size(size), Image.LANCZOS)

    if gain != 1.0:
        # Lift the base tone without flattening the shading.
        #
        # Sprite.color MULTIPLIES, so a textured sprite is darker than the
        # solid quad it replaces by exactly the texture's own luminance --
        # debris tinted to 0.55 of its block colour and then multiplied by a
        # mid-grey render lands at ~0.30 and disappears. ART_BRIEF asks for a
        # "light/neutral base" for precisely this reason. Scaling rather than
        # adding keeps the facet contrast proportional.
        canvas.putdata([
            (min(255, int(r * gain)), min(255, int(g * gain)),
             min(255, int(b * gain)), a)
            for r, g, b, a in canvas.getdata()
        ])

    # Force RGB white across the WHOLE canvas, transparent padding included.
    # (Only when the caller asked for it -- see `keep_rgb`.)
    # The game samples these with linear filtering (nothing calls
    # ImagePlugin::default_nearest), and the sampler interpolates RGB and
    # alpha independently -- so black-but-transparent padding bleeds a dark
    # fringe into the soft rim. On a texture that is almost entirely soft rim
    # that is the whole edge.
    if not keep_rgb:
        canvas.putdata([(255, 255, 255, p[3]) for p in canvas.getdata()])

    canvas.save(dst)
    return canvas


def tile_check(src, dst, n=3):
    """Montage NxN copies. Any seam the render introduced shows up here."""
    im = Image.open(src).convert("RGBA")
    w, h = im.size
    sheet = Image.new("RGBA", (w * n, h * n), (0, 0, 0, 0))
    for x in range(n):
        for y in range(n):
            sheet.paste(im, (x * w, y * h))
    sheet.save(dst)
    return sheet


def compare(paths, dst, scale=1, pad=8, bg=(18, 20, 26, 255)):
    """Side-by-side strip at a fixed height for honest comparison."""
    ims = [Image.open(p).convert("RGBA") for p in paths]
    h = max(i.height for i in ims) * scale
    ws = [int(i.width * (h / i.height)) for i in ims]
    sheet = Image.new("RGBA", (sum(ws) + pad * (len(ims) + 1), h + pad * 2), bg)
    x = pad
    for im, w in zip(ims, ws):
        sheet.paste(im.resize((w, h), Image.NEAREST), (x, pad), im.resize((w, h), Image.NEAREST))
        x += w + pad
    sheet.save(dst)
    return sheet


def install(src, dst, size, margin=0.08):
    """whiten + normalize, straight into assets/.

    The module pass had no install step and was moved into assets by hand;
    this is the smallest thing that stops that being the norm. Refuses to
    write outside assets/ so a mistyped path cannot scribble on the tree.
    """
    real = os.path.abspath(dst)
    if os.sep + "assets" + os.sep not in real:
        raise SystemExit("install: refusing to write outside assets/: %s" % real)
    tmp = real + ".whiten.tmp.png"
    whiten(src, tmp)
    normalize(tmp, real, size, margin=margin)
    os.remove(tmp)
    print("installed", src, "->", real)


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "down":
        downscale(sys.argv[2], sys.argv[3], sys.argv[4])
    elif cmd == "tile":
        tile_check(sys.argv[2], sys.argv[3])
    elif cmd == "compare":
        compare(sys.argv[2:-1], sys.argv[-1])
    elif cmd == "whiten":
        kw = 0.0
        for a in sys.argv:
            if a.startswith("--keep-warm="):
                kw = float(a.split("=", 1)[1])
        mode = sys.argv[4] if len(sys.argv) > 4 and not sys.argv[4].startswith("--") else "both"
        whiten(sys.argv[2], sys.argv[3], mode, keep_warm=kw)
    elif cmd == "normalize":
        normalize(sys.argv[2], sys.argv[3], sys.argv[4],
                  gain=float(sys.argv[5]) if len(sys.argv) > 5 else 1.0,
                  keep_rgb="--keep-rgb" in sys.argv)
    elif cmd == "install":
        install(sys.argv[2], sys.argv[3], sys.argv[4])
    else:
        raise SystemExit("unknown command: %s" % cmd)
    print("ok", cmd)
