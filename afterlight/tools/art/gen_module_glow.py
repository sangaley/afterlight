"""Glow masks for the module art's lights (vfx::module_glow).

The module art draws its lights -- reactor cores, status lamps, screens --
in a few blues, greens and oranges, and flat: nothing spills off them. For
each module sprite this finds the lit pixels and writes two white masks the
game tints by what the module is:

    sprites/module_glow/<stem>_glow.png   the lights themselves plus a soft
                                          halo round them, padded by PAD on
                                          every side so the halo can spill
                                          onto the deck round the part
    sprites/module_glow/<stem>_lit.png    the lights alone, laid over them in
                                          the module's colour
    sprites/module_glow/<stem>_core.png   the hottest pixels only, drawn
                                          untinted over it: a white-hot centre

Lit means bright and coloured, or simply very bright; the hazard stripes and
the grey panels are neither. A sprite with no lights gets no masks.

    python3 tools/art/gen_module_glow.py
"""
import os
import numpy as np
from PIL import Image, ImageFilter

HERE = os.path.dirname(__file__)
SRC = os.path.join(HERE, "..", "..", "assets", "sprites", "modules")
OUT = os.path.join(HERE, "..", "..", "assets", "sprites", "module_glow")
# The halo's room past the sprite's edge, as a share of its size, each side.
PAD = 0.25
# Working size: masks are small, blurred things.
WORK = 160

SKIP = ("turret_", "slope_", "plate", "hull_beam", "ballast", "light_frame")
# Painted markings, not lamps: a warhead's red tip shouldn't glow.
NO_DIM = ("torpedo_tube", "heavy_missile", "mine_layer", "cluster_rocket", "guided_missile")


def masks(path, dim=True):
    im = Image.open(path).convert("RGBA")
    w, h = im.size
    scale = WORK / max(w, h)
    sw, sh = max(1, round(w * scale)), max(1, round(h * scale))
    small = im.resize((sw, sh), Image.LANCZOS)
    a = np.asarray(small).astype(np.float32) / 255.0
    rgb, alpha = a[..., :3], a[..., 3]
    mx, mn = rgb.max(-1), rgb.min(-1)
    lum = rgb @ np.array([0.299, 0.587, 0.114], dtype=np.float32)
    sat = np.where(mx > 0, (mx - mn) / np.maximum(mx, 1e-6), 0)
    # How lit a pixel is, 0..1: bright and coloured, or white-hot.
    lit = np.clip((lum - 0.36) / 0.22, 0, 1) * np.clip((sat - 0.25) / 0.25, 0, 1)
    lit = np.maximum(lit, np.clip((lum - 0.72) / 0.15, 0, 1)) * alpha
    if lit.max() < 0.5 or (lit > 0.5).sum() < 3:
        if not dim:
            return None
        # Art whose lamps were drawn dim -- the quarters' window strips, the
        # life support's green lamps: the coloured marks are its lights.
        lit = np.clip((lum - 0.2) / 0.12, 0, 1) * np.clip((sat - 0.3) / 0.2, 0, 1) * alpha
        if lit.max() < 0.5 or (lit > 0.5).sum() < 3:
            return None
    core = np.clip((lum - 0.78) / 0.15, 0, 1) * alpha

    pw, ph = round(sw * PAD), round(sh * PAD)
    W, H = sw + 2 * pw, sh + 2 * ph

    def place(m):
        out = Image.new("L", (W, H), 0)
        out.paste(Image.fromarray((m * 255).astype(np.uint8)), (pw, ph))
        return out

    body = place(lit)
    halo = body.filter(ImageFilter.GaussianBlur(max(sw, sh) * 0.06))
    wide = body.filter(ImageFilter.GaussianBlur(max(sw, sh) * 0.14))
    # The lights solid, a near halo and a wide faint one.
    g = np.maximum(np.asarray(body) / 255.0, np.asarray(halo) / 255.0 * 2.4)
    g = np.maximum(g, np.asarray(wide) / 255.0 * 1.7)
    g = np.clip(g, 0, 1)
    white = np.full((H, W), 255, np.uint8)
    glow = Image.merge("RGBA", [Image.fromarray(white)] * 3 + [Image.fromarray((g * 255).astype(np.uint8))])
    c = np.asarray(place(core)) / 255.0
    c = np.clip(np.maximum(c, np.asarray(place(core).filter(ImageFilter.GaussianBlur(1.2))) / 255.0), 0, 1)
    core_im = Image.merge("RGBA", [Image.fromarray(white)] * 3 + [Image.fromarray((c * 255).astype(np.uint8))])
    b = np.asarray(body.filter(ImageFilter.GaussianBlur(0.8))) / 255.0
    lit_im = Image.merge("RGBA", [Image.fromarray(white)] * 3 + [Image.fromarray((np.clip(b * 1.3, 0, 1) * 255).astype(np.uint8))])
    return glow, core_im, lit_im


def main():
    os.makedirs(OUT, exist_ok=True)
    n = 0
    for f in sorted(os.listdir(SRC)):
        if not f.endswith(".png") or any(s in f for s in SKIP):
            continue
        got = masks(os.path.join(SRC, f), dim=not any(n in f for n in NO_DIM))
        if not got:
            continue
        stem = f[:-4]
        got[0].save(os.path.join(OUT, f"{stem}_glow.png"))
        got[1].save(os.path.join(OUT, f"{stem}_core.png"))
        got[2].save(os.path.join(OUT, f"{stem}_lit.png"))
        n += 1
    print(f"{n} modules with lights in {os.path.normpath(OUT)}")


if __name__ == "__main__":
    main()
