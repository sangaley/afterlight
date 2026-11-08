"""Gun house options, drawn over the real turrets with the game's own sprites.

Each turret is laid out the way the game lays it (deck cells, armour rim,
angle pieces, parts at their turret cells, barrels out front) and then
covered by a gun house drawn by one of the candidate styles. Writes one PNG
per style plus a side-by-side sheet into this folder.

Run from afterlight/: python3 tools/gun-house-preview/preview.py
"""
import json, math, os
from PIL import Image, ImageChops, ImageDraw, ImageFilter, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
_FONT_PATH = "/System/Library/Fonts/Helvetica.ttc"
FONT = ImageFont.truetype(_FONT_PATH, 22) if os.path.isfile(_FONT_PATH) else ImageFont.load_default()
SMALL = ImageFont.truetype(_FONT_PATH, 17) if os.path.isfile(_FONT_PATH) else ImageFont.load_default()
CELL = 56
SPRITES = {
    "FiringPiece": "firing_piece", "Mantlet": "mantlet", "GunBarrel": "barrel", "GaugeIncreaser": "gauge_increaser",
    "GaugeReducer": "gauge_reducer", "Loader": "autoloader", "ClipRack": "ammo_clip", "BreechCooler": "cooler",
    "RecoilAbsorber": "recoil_absorber",
}
OUT_FRONT = {"GunBarrel", "MuzzleBrake", "Mantlet", "RailMagnet"}
TINT = (0.86, 0.92, 1.0)


def sprite(name, rot=0.0):
    im = Image.open(os.path.join(ROOT, "assets/sprites/gun_parts", name + ".png")).convert("RGBA").resize((CELL, CELL), Image.LANCZOS)
    return im.rotate(math.degrees(rot), resample=Image.BICUBIC) if rot else im


def tinted(im, t):
    r, g, b, a = im.split()
    return Image.merge("RGBA", (r.point(lambda v: int(v * t[0])), g.point(lambda v: int(v * t[1])), b.point(lambda v: int(v * t[2])), a))


def deck_cells(size):
    r = size / 2 + 1.05
    span = math.ceil(r)
    return {(x, y) for x in range(-span, span + 1) for y in range(-span, span + 1) if math.hypot(x, y) <= r}


def deck_wedges(size):
    cells = deck_cells(size)
    span = math.ceil(size / 2 + 1.05) + 1
    out = []
    for x in range(-span, span + 1):
        for y in range(-span, span + 1):
            if (x, y) in cells:
                continue
            for d in [(1, 1), (1, -1), (-1, 1), (-1, -1)]:
                if (x + d[0], y) in cells and (x, y + d[1]) in cells:
                    out.append(((x, y), d))
                    break
    return out


ROT = {"North": 0.0, "East": -math.pi / 2, "South": math.pi, "West": math.pi / 2}


# ---- gun house styles ------------------------------------------------------
STEEL = (92, 104, 120)


def shade(c, k):
    return tuple(max(0, min(255, int(v * k))) for v in c[:3]) + ((c[3],) if len(c) > 3 else ())


def breech_box(parts):
    """The breech: firing pieces and loaders, and the lanes the guns run in."""
    core = [(x, y) for t, (x, y), _ in parts if t in ("FiringPiece", "Loader", "RecoilAbsorber")]
    fps = [(x, y) for t, (x, y), _ in parts if t == "FiringPiece"]
    x0 = min(x for x, _ in core)
    x1 = max(x for x, _ in fps)
    lanes = sorted({y for _, y in fps})
    return x0, x1, lanes


def poly_px(pts, origin):
    ox, oy = origin
    return [(ox + x * CELL, oy - y * CELL) for x, y in pts]


def detail_roof(d, img, pts, lanes, front_x, back_x, y_lo, y_hi, origin, style):
    ox, oy = origin
    P = lambda x, y: (ox + x * CELL, oy - y * CELL)
    # Roof plates: seams across the roof every half cell.
    x = back_x + 0.5
    while x < front_x - 0.3:
        d.line([P(x, y_lo + 0.12), P(x, y_hi - 0.12)], fill=shade(STEEL, 0.72), width=2)
        x += 0.55
    # Rivets along the long edges.
    x = back_x + 0.25
    while x < front_x - 0.25:
        for y in (y_lo + 0.1, y_hi - 0.1):
            cx, cy = P(x, y)
            d.rectangle([cx - 1, cy - 1, cx + 1, cy + 1], fill=shade(STEEL, 1.35))
        x += 0.28
    # Barrel ports in the faceplate: a dark slot and a collar per gun.
    for ly in lanes:
        cx, cy = P(front_x - 0.02, ly)
        d.rectangle([cx - 10, cy - 7, cx + 3, cy + 7], fill=shade(STEEL, 0.55))
        d.rectangle([cx - 6, cy - 4, cx + 3, cy + 4], fill=(22, 24, 28))
    # Periscope hoods near the front corners, a square hatch at the back.
    for y in (y_hi - 0.32, y_lo + 0.32):
        cx, cy = P(front_x - 0.75, y)
        d.rectangle([cx - 5, cy - 4, cx + 5, cy + 4], fill=shade(STEEL, 0.6))
        d.rectangle([cx - 4, cy - 3, cx + 4, cy - 1], fill=(110, 170, 200))
    hx, hy = P(back_x + 0.55, (y_lo + y_hi) / 2 + (0.25 if style != "shield" else 0))
    d.rectangle([hx - 9, hy - 9, hx + 9, hy + 9], fill=shade(STEEL, 0.78), outline=shade(STEEL, 0.5))
    d.rectangle([hx - 6, hy - 6, hx + 6, hy + 6], outline=shade(STEEL, 1.2))
    d.rectangle([hx - 9, hy - 2, hx - 7, hy + 2], fill=shade(STEEL, 0.4))
    # Vent grille at the back.
    if style != "shield":
        vx, vy = P(back_x + 0.55, (y_lo + y_hi) / 2 - 0.35)
        for i in range(4):
            d.line([(vx - 9, vy - 6 + i * 4), (vx + 9, vy - 6 + i * 4)], fill=shade(STEEL, 0.5), width=2)


def draw_house(base, parts, origin, style):
    if style == "current":
        housed = [c for t, c, _ in parts if t not in OUT_FRONT]
        lo = (min(x for x, _ in housed), min(y for _, y in housed))
        hi = (max(x for x, _ in housed), max(y for _, y in housed))
        w, h = (hi[0] - lo[0] + 1) * CELL - 5, (hi[1] - lo[1] + 1) * CELL - 5
        house = Image.open(os.path.join(ROOT, "assets/sprites/gun_parts/gun_house.png")).convert("RGBA")
        # 9-slice it the way the game does.
        b = 24
        sw, sh = house.size
        out = Image.new("RGBA", (w, h))
        cs = min(b, w // 2, h // 2)
        tiles = [(0, 0, b, b), (b, 0, sw - b, b), (sw - b, 0, sw, b), (0, b, b, sh - b), (b, b, sw - b, sh - b), (sw - b, b, sw, sh - b), (0, sh - b, b, sh), (b, sh - b, sw - b, sh), (sw - b, sh - b, sw, sh)]
        dst = [(0, 0, cs, cs), (cs, 0, w - cs, cs), (w - cs, 0, w, cs), (0, cs, cs, h - cs), (cs, cs, w - cs, h - cs), (w - cs, cs, w, h - cs), (0, h - cs, cs, h), (cs, h - cs, w - cs, h), (w - cs, h - cs, w, h)]
        for s, t in zip(tiles, dst):
            if t[2] > t[0] and t[3] > t[1]:
                out.paste(house.crop(s).resize((t[2] - t[0], t[3] - t[1])), (t[0], t[1]))
        hatch = Image.open(os.path.join(ROOT, "assets/sprites/gun_parts/gun_house_hatch.png")).convert("RGBA").resize((25, 25))
        out.alpha_composite(hatch, (int(w / 2 - w * 0.22 - 12), int(h / 2 - 12)))
        cx = origin[0] + (lo[0] + hi[0]) / 2 * CELL
        cy = origin[1] - (lo[1] + hi[1]) / 2 * CELL
        base.alpha_composite(out, (int(cx - w / 2), int(cy - h / 2)))
        return

    if style in ("plated", "roof"):
        draw_full_house(base, parts, origin, style)
        return

    x0, x1, lanes = breech_box(parts)
    if style == "tapered":
        # Over the breech and loaders, a sloped faceplate the barrels run out
        # through: the sides close in toward the front at 45 degrees.
        back, front = x0 - 0.45, x1 + 0.5
        y_lo, y_hi = min(lanes) - 0.62, max(lanes) + 0.62
        t = 0.38
        pts = [(back + 0.15, y_hi), (front - t, y_hi), (front, y_hi - t), (front, y_lo + t), (front - t, y_lo), (back + 0.15, y_lo), (back, y_lo + 0.15), (back, y_hi - 0.15)]
    elif style == "faceted":
        # Low and angular, sides broken by a second facet: wider amidships,
        # pinched at the front and at the back.
        back, front = x0 - 0.35, x1 + 0.45
        y_lo, y_hi = min(lanes) - 0.7, max(lanes) + 0.7
        mid = back + (front - back) * 0.4
        pts = [(back, y_hi - 0.3), (mid, y_hi), (front - 0.3, y_hi - 0.15), (front, y_hi - 0.45), (front, y_lo + 0.45), (front - 0.3, y_lo + 0.15), (mid, y_lo), (back, y_lo + 0.3)]
    else:  # shield: only round the firing pieces; loaders and racks stay out in the open
        back, front = x1 - 0.45, x1 + 0.45
        y_lo, y_hi = min(lanes) - 0.55, max(lanes) + 0.55
        pts = [(back, y_hi - 0.2), (back + 0.25, y_hi), (front - 0.3, y_hi), (front, y_hi - 0.3), (front, y_lo + 0.3), (front - 0.3, y_lo), (back + 0.25, y_lo), (back, y_lo + 0.2)]

    px = poly_px(pts, origin)
    # A soft shadow on the deck, down-right, so the house stands up off it.
    sh = Image.new("RGBA", base.size)
    ImageDraw.Draw(sh).polygon([(x + 5, y + 6) for x, y in px], fill=(0, 0, 0, 120))
    base.alpha_composite(sh.filter(ImageFilter.GaussianBlur(3)))
    top = Image.new("RGBA", base.size)
    d = ImageDraw.Draw(top)
    d.polygon(px, fill=STEEL + (255,))
    # The front facets catch less light: a darker band along the faceplate.
    fb = poly_px([(front - 0.28, y_hi - 0.05), (front, y_hi - 0.3), (front, y_lo + 0.3), (front - 0.28, y_lo + 0.05)], origin)
    if style == "tapered":
        fb = poly_px([(front - 0.38, y_hi), (front, y_hi - 0.38), (front, y_lo + 0.38), (front - 0.38, y_lo)], origin)
    d.polygon(fb, fill=shade(STEEL, 0.78) + (255,))
    detail_roof(d, top, pts, lanes, front, back, y_lo, y_hi, origin, style)
    # Lit edges up-left, dark edges down-right.
    n = len(px)
    for i in range(n):
        a, b = px[i], px[(i + 1) % n]
        mx, my = (a[0] + b[0]) / 2 - origin[0], (a[1] + b[1]) / 2 - origin[1]
        lit = (b[0] - a[0]) * 1 + (b[1] - a[1]) * -1 < 0
        d.line([a, b], fill=shade(STEEL, 1.45 if lit else 0.45), width=2)
    base.alpha_composite(top)


def draw_full_house(base, parts, origin, style):
    """Over every part but the barrels, like the game's house now -- but
    built to look like part of the ship rather than a sticker on it."""
    housed = [c for t, c, _ in parts if t not in OUT_FRONT]
    fps = [c for t, c, _ in parts if t == "FiringPiece"]
    lanes = sorted({y for _, y in fps})
    x_lo, x_hi = min(x for x, _ in housed), max(x for x, _ in housed)
    y_lo, y_hi = min(y for _, y in housed), max(y for _, y in housed)
    ox, oy = origin
    P = lambda x, y: (ox + x * CELL, oy - y * CELL)
    L, R, B, T = x_lo - 0.46, x_hi + 0.46, y_lo - 0.46, y_hi + 0.46
    c = 0.42 if (x_hi - x_lo) >= 1 and (y_hi - y_lo) >= 1 else 0.3  # 45-degree corners, like the deck rim
    outline = [(L + c, T), (R - c, T), (R, T - c), (R, B + c), (R - c, B), (L + c, B), (L, B + c), (L, T - c)]
    px = [P(x, y) for x, y in outline]
    # It stands up off the deck: a shadow down and to the right.
    sh = Image.new("RGBA", base.size)
    ImageDraw.Draw(sh).polygon([(x + 9, y + 11) for x, y in px], fill=(0, 0, 0, 175))
    base.alpha_composite(sh.filter(ImageFilter.GaussianBlur(5)))
    top = Image.new("RGBA", base.size)
    if style == "plated":
        # Roof laid from the same armour plates as the deck rim and the hull,
        # a shade lighter: it is the highest thing on the ship.
        tile = tinted(sprite("turret_armour"), (1.32, 1.38, 1.5))
        for x in range(x_lo, x_hi + 1):
            for y in range(y_lo, y_hi + 1):
                top.alpha_composite(tile, (ox + x * CELL - CELL // 2, oy - y * CELL - CELL // 2))
        mask = Image.new("L", base.size, 0)
        ImageDraw.Draw(mask).polygon(px, fill=255)
        top.putalpha(ImageChops.multiply(top.getchannel("A"), mask))
    else:
        d = ImageDraw.Draw(top)
        # Not one flat colour: light falls from the upper left across the roof.
        dark = (66, 76, 90)
        grad = Image.new("RGBA", base.size)
        gd = ImageDraw.Draw(grad)
        x0p, y0p = P(L, T)
        x1p, y1p = P(R, B)
        steps = 24
        for i in range(steps):
            k = 1.18 - 0.36 * i / steps
            gd.rectangle([x0p, y0p + (y1p - y0p) * i / steps, x1p, y0p + (y1p - y0p) * (i + 1) / steps + 1], fill=shade(dark, k) + (255,))
        mask = Image.new("L", base.size, 0)
        ImageDraw.Draw(mask).polygon(px, fill=255)
        grad.putalpha(mask)
        top.alpha_composite(grad)
        STEEL_ = dark
        # Roof plates, a seam on every cell line, rivets along them.
        for x in range(x_lo, x_hi):
            d.line([P(x + 0.5, B + 0.08), P(x + 0.5, T - 0.08)], fill=shade(STEEL, 0.7), width=2)
        for y in range(y_lo, y_hi):
            d.line([P(L + 0.08, y + 0.5), P(R - 0.4, y + 0.5)], fill=shade(STEEL, 0.78), width=1)
        for x in range(x_lo, x_hi + 1):
            for y in (B + 0.12, T - 0.12):
                for dx in (-0.25, 0.0, 0.25):
                    cx, cy = P(x + dx, y)
                    d.rectangle([cx - 1, cy - 1, cx + 1, cy + 1], fill=shade(STEEL, 1.4))
    d = ImageDraw.Draw(top)
    # The faceplate: a darker sloped band across the front, a port per gun.
    fb = [P(R - 0.32, T - 0.1), P(R, T - c), P(R, B + c), P(R - 0.32, B + 0.1)]
    d.polygon(fb, fill=shade(STEEL, 0.62) + (220,))
    for ly in lanes:
        cx, cy = P(R - 0.05, ly)
        d.rectangle([cx - 12, cy - 9, cx + 3, cy + 9], fill=shade(STEEL, 0.45))
        d.rectangle([cx - 8, cy - 5, cx + 3, cy + 5], fill=(20, 22, 26))
    # Periscopes either side at the front, hatches at the back, a vent grille.
    for y in (T - 0.36, B + 0.36):
        cx, cy = P(R - 0.72, y)
        d.rectangle([cx - 7, cy - 5, cx + 7, cy + 5], fill=shade(STEEL, 0.5))
        d.rectangle([cx - 5, cy - 4, cx + 5, cy - 1], fill=(110, 170, 200))
    hatches = [(L + 0.62, (B + T) / 2 + 0.38), (L + 0.62, (B + T) / 2 - 0.38)] if (y_hi - y_lo) >= 2 else [(L + 0.55, (B + T) / 2)]
    for hx, hy in hatches:
        cx, cy = P(hx, hy)
        d.rectangle([cx - 10, cy - 10, cx + 10, cy + 10], fill=shade(STEEL, 0.75), outline=shade(STEEL, 0.45))
        d.rectangle([cx - 7, cy - 7, cx + 7, cy + 7], outline=shade(STEEL, 1.25))
        d.rectangle([cx - 10, cy - 3, cx - 8, cy + 3], fill=shade(STEEL, 0.35))
    if (x_hi - x_lo) >= 1:
        vx, vy = P(L + 0.25 + (1.0 if (y_hi - y_lo) >= 2 else 0.9), (B + T) / 2)
        for i in range(4):
            d.line([(vx - 9, vy - 6 + i * 4), (vx + 9, vy - 6 + i * 4)], fill=shade(STEEL, 0.45), width=2)
    # Lit edges up-left, dark edges down-right: the bevel that makes it
    # solid -- an outer line and a softer one just inside it.
    for i in range(len(px)):
        a, b = px[i], px[(i + 1) % len(px)]
        lit = (b[0] - a[0]) - (b[1] - a[1]) < 0
        d.line([a, b], fill=shade(STEEL, 1.6 if lit else 0.3), width=3)
    inner = [P(x + (0.07 if x < (L + R) / 2 else -0.07), y + (0.07 if y < (B + T) / 2 else -0.07)) for x, y in outline]
    for i in range(len(inner)):
        a, b = inner[i], inner[(i + 1) % len(inner)]
        lit = (b[0] - a[0]) - (b[1] - a[1]) < 0
        d.line([a, b], fill=shade(STEEL, 1.25 if lit else 0.55) + (150,), width=2)
    base.alpha_composite(top)


def turret(parts, size, style, label):
    span = math.ceil(size / 2 + 1.05) + 2
    reach = max(max(abs(x), abs(y)) for _, (x, y), _ in parts) + 1
    half = max(span, reach)
    W = (2 * half + 1) * CELL
    img = Image.new("RGBA", (W, W + 26), (14, 15, 20, 255))
    origin = (W // 2, W // 2)
    floor = tinted(sprite("turret_plating"), TINT)
    armour = tinted(sprite("turret_armour"), TINT)
    cells = deck_cells(size)
    for (x, y) in cells:
        edge = any((x + dx, y + dy) not in cells for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))
        img.alpha_composite(armour if edge else floor, (origin[0] + x * CELL - CELL // 2, origin[1] - y * CELL - CELL // 2))
    for (x, y), d in deck_wedges(size):
        turn = {(-1, -1): 0.0, (1, -1): math.pi / 2, (1, 1): math.pi, (-1, 1): -math.pi / 2}[d]
        w = tinted(sprite("turret_armour_wedge", turn), TINT)
        img.alpha_composite(w, (origin[0] + x * CELL - CELL // 2, origin[1] - y * CELL - CELL // 2))
    order = sorted(parts, key=lambda p: p[0] in OUT_FRONT)
    for t, (x, y), r in order:
        if t in SPRITES:
            img.alpha_composite(sprite(SPRITES[t], ROT[r]), (origin[0] + x * CELL - CELL // 2, origin[1] - y * CELL - CELL // 2))
    under = [p for p in parts if p[0] not in OUT_FRONT]
    draw_house(img, under, origin, style)
    # Barrels and mantlets come out through the front, over the house.
    for t, (x, y), r in parts:
        if t in OUT_FRONT and t in SPRITES:
            img.alpha_composite(sprite(SPRITES[t], ROT[r]), (origin[0] + x * CELL - CELL // 2, origin[1] - y * CELL - CELL // 2))
    # Crop to what was drawn, then label it underneath.
    bg = Image.new("RGBA", img.size, (14, 15, 20, 255))
    box = ImageChops.difference(img.convert("RGB"), bg.convert("RGB")).getbbox()
    img = img.crop((box[0] - 16, box[1] - 16, box[2] + 16, box[3] + 16))
    out = Image.new("RGBA", (img.width, img.height + 28), (14, 15, 20, 255))
    out.alpha_composite(img)
    ImageDraw.Draw(out).text((10, img.height + 2), label, fill=(200, 205, 215), font=SMALL)
    return out


def main():
    d = json.load(open(os.path.join(ROOT, "designs/halyard_mk2.json")))
    by_ring = {}
    for m in d["modules"]:
        ring = (m.get("extras") or {}).get("turret")
        if ring:
            by_ring.setdefault(tuple(ring), []).append((m["module_type"], tuple(m["grid_pos"]), m["rotation"]))
    ciws = json.load(open(os.path.join(ROOT, "designs/guns/20_mm_ciws.json")))
    turrets = [
        ("twin 225 mm (large ring)", by_ring[(-4, -2)], 5),
        ("170 mm (medium ring)", by_ring[(8, 4)], 3),
        ("20 mm CIWS (small ring)", [(p["module_type"], tuple(p["offset"]), p["rotation"]) for p in ciws["parts"]], 1),
    ]
    styles = [("current", "NOW: one stretched picture over every part"),
              ("plated", "D: same cover, built from the ship's armour plates"),
              ("roof", "E: same cover, a detailed steel roof")]
    rows = []
    for key, title in styles:
        tiles = [turret(parts, size, key, name) for name, parts, size in turrets]
        h = max(t.height for t in tiles)
        row = Image.new("RGBA", (sum(t.width for t in tiles) + 20 * (len(tiles) - 1), h + 40), (9, 10, 13, 255))
        ImageDraw.Draw(row).text((10, 6), title, fill=(235, 200, 120), font=FONT)
        x = 0
        for t in tiles:
            row.alpha_composite(t, (x, 40 + (h - t.height) // 2))
            x += t.width + 20
        row.save(os.path.join(HERE, f"style_{key}.png"))
        rows.append(row)
    W = max(r.width for r in rows)
    sheet = Image.new("RGBA", (W, sum(r.height for r in rows) + 10 * len(rows)), (9, 10, 13, 255))
    y = 0
    for r in rows:
        sheet.alpha_composite(r, (0, y))
        y += r.height + 10
    sheet.save(os.path.join(HERE, "gun_houses_full.png"))
    print(os.path.join(HERE, "gun_houses_full.png"), sheet.size)


if __name__ == "__main__":
    main()


# ---- the roof the game lays (combat::gun_house::default_roof), style D ------
ROOF_TYPES = {"ArmorPlate", "AngledArmorPlate", "Rangefinder", "RoofVent", "Periscope", "ArmouredHatch",
              "BevelledArmorPlate", "SlopeArmorPlateLow", "SlopeArmorPlateHigh", "SlopeArmorPlateLowMirror",
              "SlopeArmorPlateHighMirror", "HalfArmorPlate"}


def default_roof(parts, size):
    """A line-for-line copy of combat::gun_house::default_roof."""
    housed = [c for t, c, _ in parts if t not in OUT_FRONT and t not in ROOF_TYPES]
    x0, x1 = min(x for x, _ in housed), max(x for x, _ in housed)
    y0, y1 = min(y for _, y in housed), max(y for _, y in housed)
    lanes = [c[1] for t, c, _ in parts if t == "FiringPiece"]
    port = lambda c: c[0] == x1 and c[1] in lanes
    deck = deck_cells(size)
    roof = {(x, y): ("ArmorPlate", "North") for x in range(x0, x1 + 1) for y in range(y0, y1 + 1) if (x, y) in deck}
    big = x1 - x0 >= 4 and y1 - y0 >= 4
    facing = {(1, 1): "North", (1, -1): "East", (-1, 1): "West", (-1, -1): "South"}
    free = lambda c: c in roof and not port(c)
    for corner, out in [((x0, y1), (-1, 1)), ((x0, y0), (-1, -1))]:
        sides = [(corner[0] - out[0], corner[1]), (corner[0], corner[1] - out[1])]
        wedge = ("AngledArmorPlate", facing[out])
        if big and all(free(s) for s in sides):
            roof.pop(corner, None)
            for s in sides:
                roof[s] = wedge
        elif free(corner):
            roof[corner] = wedge
    for y, out in [(y1, (1, 1)), (y0, (1, -1))]:
        back, front = (x1 - 1, y), (x1, y)
        if big and free(back) and free(front):
            if out[1] > 0:
                roof[back], roof[front] = ("SlopeArmorPlateHighMirror", "North"), ("SlopeArmorPlateLowMirror", "North")
            else:
                roof[back], roof[front] = ("SlopeArmorPlateHigh", "South"), ("SlopeArmorPlateLow", "South")
        elif free(front):
            roof[front] = ("BevelledArmorPlate", facing[out])
    mid = (y0 + y1) // 2
    def fit(t, cands):
        for c in cands:
            if not port(c) and roof.get(c, ("",))[0] == "ArmorPlate":
                roof[c] = (t, "North")
                return
    w, h = x1 - x0 + 1, y1 - y0 + 1
    fit("ArmouredHatch", [(x0, mid), (x0, mid + 1), (x0 + 1, mid)])
    if w >= 3:
        fit("RoofVent", [(x0 + 1, mid), (x0 + 1, mid + 1)])
    if h >= 3:
        for edge, inward in [(y1, -1), (y0, 1)]:
            fit("Periscope", [(x1 - 1, edge), (x1 - 1, edge + inward), (x1 - 2, edge)])
    if w >= 4 and h >= 3:
        for edge, inward in [(y1, -1), (y0, 1)]:
            fit("Rangefinder", [(x0 + 1, edge), (x0 + 2, edge), (x0 + 1, edge + inward)])
    return roof


ROOF_ART = {"ArmorPlate": "roof_plate", "AngledArmorPlate": "roof_wedge", "Rangefinder": "rangefinder",
            "RoofVent": "roof_vent", "Periscope": "periscope", "ArmouredHatch": "armoured_hatch",
            "BevelledArmorPlate": "roof_bevelled_plate", "SlopeArmorPlateLow": "roof_slope_low",
            "SlopeArmorPlateHigh": "roof_slope_high", "SlopeArmorPlateLowMirror": "roof_slope_low_mirror",
            "SlopeArmorPlateHighMirror": "roof_slope_high_mirror", "HalfArmorPlate": "roof_half_plate"}
SHAPED = {"AngledArmorPlate", "BevelledArmorPlate", "SlopeArmorPlateLow", "SlopeArmorPlateHigh",
          "SlopeArmorPlateLowMirror", "SlopeArmorPlateHighMirror", "HalfArmorPlate"}


def draw_roof(base, parts, origin, size):
    roof = default_roof(parts, size)
    ox, oy = origin
    # The roof stands a level up: its shadow falls down-right on the deck.
    pieces = [((x, y), sprite(ROOF_ART[t], ROT[r] if t in SHAPED else 0.0)) for (x, y), (t, r) in sorted(roof.items())]
    sh = Image.new("RGBA", base.size)
    for (x, y), art in pieces:
        black = Image.new("RGBA", art.size, (0, 0, 0, 150))
        black.putalpha(ImageChops.multiply(art.getchannel("A"), Image.new("L", art.size, 150)))
        sh.alpha_composite(black, (ox + x * CELL - CELL // 2 + 8, oy - y * CELL - CELL // 2 + 9))
    base.alpha_composite(sh.filter(ImageFilter.GaussianBlur(4)))
    for (x, y), art in pieces:
        base.alpha_composite(art, (ox + x * CELL - CELL // 2, oy - y * CELL - CELL // 2))


def roof_sheet():
    d = json.load(open(os.path.join(ROOT, "designs/halyard_mk2.json")))
    by_ring = {}
    for m in d["modules"]:
        ring = (m.get("extras") or {}).get("turret")
        if ring:
            by_ring.setdefault(tuple(ring), []).append((m["module_type"], tuple(m["grid_pos"]), m["rotation"]))
    ciws = json.load(open(os.path.join(ROOT, "designs/guns/20_mm_ciws.json")))
    turrets = [
        ("twin 225 mm (large ring)", by_ring[(-4, -2)], 5),
        ("170 mm (medium ring)", by_ring[(8, 4)], 3),
        ("20 mm CIWS (small ring)", [(p["module_type"], tuple(p["offset"]), p["rotation"]) for p in ciws["parts"]], 1),
    ]
    global draw_house
    tiles = []
    for name, parts, size in turrets:
        saved = draw_house
        draw_house = lambda base, under, origin, style, _p=parts, _s=size: draw_roof(base, _p, origin, _s)
        tiles.append(turret(parts, size, "roof_blocks", name))
        draw_house = saved
    h = max(t.height for t in tiles)
    row = Image.new("RGBA", (sum(t.width for t in tiles) + 20 * (len(tiles) - 1), h + 40), (9, 10, 13, 255))
    ImageDraw.Draw(row).text((10, 6), "D in game: a roof of real blocks, angle pieces and fittings", fill=(235, 200, 120), font=FONT)
    x = 0
    for t in tiles:
        row.alpha_composite(t, (x, 40 + (h - t.height) // 2))
        x += t.width + 20
    out = os.path.join(HERE, "roof_blocks.png")
    row.save(out)
    print(out)


if __name__ == "__main__" and os.environ.get("ROOF_SHEET"):
    roof_sheet()
