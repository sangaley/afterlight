#!/usr/bin/env python3
"""Gun parts in the module set's style (darkset.py): dark block-filling plate,
corner bolts, grime, hazard bands, brass where ammunition lives, icy blue for
coolant, copper for rail magnets. Replaces the flat stand-ins in
assets/sprites/gun_parts/ (GUN_PARTS_ART.md lists every part).

Directional parts point UP. Tiling parts (barrel, loader, clip, conveyor,
turret plating) run edge to edge top and bottom so segments join.

    cd afterlight/tools/art && python3 gun_parts_art.py            # previews only
    cd afterlight/tools/art && python3 gun_parts_art.py --install  # into the game
"""
import os
import sys
from darkset import *          # dark palette + fill_block/led/pipe/cyl/small_glow/R/U/P/rr/save/layer/compose
from rooms_lib import Room

GAME = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "assets", "sprites", "gun_parts"))

BRASS = (196, 158, 78); BRASS_L = (236, 206, 128); BRASS_D = (122, 92, 40)
ICE = (128, 206, 236); ICE_D = (52, 112, 146)
COP_C = (184, 112, 58); COP_CL = (232, 164, 98)
PLATE = mix(STEEL, STEEL_D, 0.25)
W = 126.0


def blank():
    """A transparent 1x1 canvas, no backing plate (for parts that aren't a full block)."""
    r = Room(1, 1)
    r.img = Image.new("RGBA", (r.W, r.H), (0, 0, 0, 0))
    r.d = ImageDraw.Draw(r.img)
    return r


def put(r, name):
    """Smooth master -> 128 px pixel sprite, the gun parts' cell size."""
    big = r.img.resize((int(W * FINAL), int(W * FINAL)), Image.LANCZOS)
    small = big.resize((128, 128), Image.BOX)
    rgb = small.convert("RGB").quantize(colors=40, dither=Image.Dither.NONE).convert("RGB")
    a = small.getchannel("A").point(lambda v: min(255, int(round(v / 51)) * 51))
    out = rgb.convert("RGBA")
    out.putalpha(a)
    return out


def tile_plate(r, x0, x1, seed, base=PLATE, bolts=True):
    """A plate running off the top and bottom edges, for parts that tile."""
    r.d.rectangle([U(r, x0), 0, U(r, x1), r.H], fill=base + (255,))
    r.d.line([(U(r, x0), 0), (U(r, x0), r.H)], fill=EDGE + (255,), width=SS * 2)
    r.d.line([(U(r, x1), 0), (U(r, x1), r.H)], fill=EDGE + (255,), width=SS * 2)
    r.d.line([(U(r, x0 + 2), 0), (U(r, x0 + 2), r.H)], fill=A(LIGHT, 110), width=SS)
    r.d.line([(U(r, x1 - 2), 0), (U(r, x1 - 2), r.H)], fill=A(DARK, 130), width=SS)
    metal_noise(r, (x0 + x1) / 2, W / 2, (x1 - x0) / 2, seed)
    if bolts:
        for by in (W * 0.25, W * 0.75):
            bolt(r, x0 + 6, by, 1.7)
            bolt(r, x1 - 6, by, 1.7)


def shell_side(r, cx, y_tip, length, w):
    """A round lying lengthwise, nose up: brass case, dark steel projectile."""
    case_top = y_tip + length * 0.42
    rr(r, [cx - w / 2, case_top, cx + w / 2, y_tip + length], radius=U(r, 1.5), fill=BRASS + (255,), outline=EDGE + (255,), width=SS)
    r.d.line([P(r, cx - w / 2 + 1.3, case_top + 2), P(r, cx - w / 2 + 1.3, y_tip + length - 2)], fill=A(BRASS_L, 220), width=SS)
    r.d.rectangle([U(r, cx - w / 2 - 0.8), U(r, y_tip + length - 3), U(r, cx + w / 2 + 0.8), U(r, y_tip + length)], fill=BRASS_D + (255,))
    nose = [P(r, cx - w / 2 + 0.6, case_top), P(r, cx + w / 2 - 0.6, case_top), P(r, cx + w * 0.18, y_tip + 2), P(r, cx, y_tip), P(r, cx - w * 0.18, y_tip + 2)]
    r.d.polygon(nose, fill=mix(STEEL_D, DARK, 0.2) + (255,), outline=EDGE + (255,))
    r.d.line([P(r, cx - w / 2 + 1.5, case_top - 1), P(r, cx - w * 0.12, y_tip + 3)], fill=A(STEEL_L, 120), width=SS)
    r.d.rectangle([U(r, cx - w / 2 + 0.6), U(r, case_top - 2.2), U(r, cx + w / 2 - 0.6), U(r, case_top)], fill=COP_C + (255,))


def shell_base(r, cx, cy, rad):
    """A round seen end on: brass base, rim, primer."""
    r.d.ellipse([U(r, cx - rad - 1.5), U(r, cy - rad - 1.5), U(r, cx + rad + 1.5), U(r, cy + rad + 1.5)], fill=mix(STEEL_D, DARK, 0.4) + (255,), outline=EDGE + (255,), width=SS)
    r.d.ellipse([U(r, cx - rad), U(r, cy - rad), U(r, cx + rad), U(r, cy + rad)], fill=BRASS + (255,), outline=BRASS_D + (255,), width=SS * 2)
    r.d.arc([U(r, cx - rad + 1.5), U(r, cy - rad + 1.5), U(r, cx + rad - 1.5), U(r, cy + rad - 1.5)], 170, 300, fill=A(BRASS_L, 230), width=SS * 2)
    pr = rad * 0.28
    r.d.ellipse([U(r, cx - pr), U(r, cy - pr), U(r, cx + pr), U(r, cy + pr)], fill=mix(BRASS_D, DARK, 0.3) + (255,))


def bore(r, cx, cy, rad):
    r.d.ellipse([U(r, cx - rad), U(r, cy - rad), U(r, cx + rad), U(r, cy + rad)], fill=(6, 7, 10, 255), outline=EDGE + (255,), width=SS * 2)
    r.d.arc([U(r, cx - rad), U(r, cy - rad), U(r, cx + rad), U(r, cy + rad)], -20, 120, fill=A(STEEL_L, 120), width=SS * 2)


def ring(r, cx, cy, rad, w, col, outline=EDGE):
    r.d.ellipse([U(r, cx - rad), U(r, cy - rad), U(r, cx + rad), U(r, cy + rad)], outline=outline + (255,), width=int(SS * (w + 1.6)))
    r.d.ellipse([U(r, cx - rad + 0.8), U(r, cy - rad + 0.8), U(r, cx + rad - 0.8), U(r, cy + rad - 0.8)], outline=col + (255,), width=int(SS * w))
    rim3d(r, cx, cy, rad + w * 0.25, w=1.2)


# ======================================================================= THE BREECH

def firing_piece():
    r = R(1, 1); c = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "fp", base=PLATE)
    rr(r, [22, 20, W - 22, W - 16], radius=U(r, 6), fill=mix(STEEL_D, DARK, 0.25) + (255,), outline=EDGE + (255,), width=SS * 2)
    # The breech ring with its brass locking collar, interrupted threads.
    ring(r, c, c + 4, 34, 7, STEEL)
    for a0 in range(0, 360, 60):
        r.d.arc([U(r, c - 26), U(r, c + 4 - 26), U(r, c + 26), U(r, c + 4 + 26)], a0 + 6, a0 + 42, fill=BRASS + (255,), width=SS * 7)
    sphere_shade(r, c, c + 4, 19, mix(STEEL_D, DARK, 0.2))
    bore(r, c, c + 4, 9)
    # The bore runs out to the top edge: this is where the barrel or loader joins.
    rr(r, [c - 7, 4, c + 7, c - 18], radius=U(r, 2), fill=mix(STEEL_D, DARK, 0.3) + (255,), outline=EDGE + (255,), width=SS * 2)
    r.d.rectangle([U(r, c - 3), U(r, 4), U(r, c + 3), U(r, c - 18)], fill=(6, 7, 10, 255))
    # Firing lead and a ready light.
    pipe(r, W - 30, W - 22, W - 16, W - 36, w=3, col=COP_C)
    led(r, 28, W - 26, RD, 1.8)
    return put(r, "firing_piece.png")


def gauge_increaser():
    r = R(1, 1); c = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "gi", base=PLATE)
    for rad, col in ((44, STEEL_D), (34, STEEL), (24, mix(STEEL, STEEL_D, 0.5))):
        ring(r, c, c, rad, 6, col)
    r.d.rectangle([U(r, c - 6), U(r, 10), U(r, c + 6), U(r, W - 10)], fill=BRASS + (255,), outline=EDGE + (255,), width=SS * 2)
    r.d.line([P(r, c - 4, 12), P(r, c - 4, W - 12)], fill=A(BRASS_L, 220), width=SS)
    for y in (20, W - 20):
        bolt(r, c, y, 2.3)
    bore(r, c, c, 10)
    return put(r, "gauge_increaser.png")


def gauge_reducer():
    r = R(1, 1); c = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "gr", base=PLATE)
    ring(r, c, c, 40, 6, STEEL_D)
    # A liner sleeve pressed into the bore: brass inside steel, a narrower hole.
    sphere_shade(r, c, c, 30, mix(STEEL, STEEL_D, 0.4))
    ring(r, c, c, 18, 5, BRASS)
    bore(r, c, c, 6)
    for k in range(8):
        a = math.radians(k * 45 + 22.5)
        bolt(r, c + math.cos(a) * 36, c + math.sin(a) * 36, 1.8)
    # Arrows in: the bore gets smaller here.
    for ang in (90, 270):
        a = math.radians(ang); ux, uy = math.cos(a), math.sin(a)
        tip = (c + ux * 22, c + uy * 22); base = (c + ux * 28, c + uy * 28)
        r.d.polygon([P(r, *tip), P(r, base[0] - uy * 4, base[1] + ux * 4), P(r, base[0] + uy * 4, base[1] - ux * 4)], fill=CAUT + (255,))
    return put(r, "gauge_reducer.png")


# ======================================================================= LOADING

def autoloader():
    r = blank(); c = W / 2
    tile_plate(r, 6, W - 6, "al", bolts=False)
    # Left frame rail: a steel channel with lightening holes and a ready light.
    rail(r, 9, 25)
    for y in (W * 0.25, W * 0.75):
        rr(r, [13, y - 10, 21, y + 10], radius=U(r, 3), fill=(12, 13, 18, 255), outline=EDGE + (255,), width=SS)
        r.d.line([P(r, 13.5, y - 8), P(r, 13.5, y + 8)], fill=A(STEEL_L, 90), width=SS)
    led(r, 17, W * 0.5, GRN, 1.9)
    # Right frame: the rammer chain in its guide, and the hydraulic line beside it.
    rail(r, W - 27, W - 13)
    r.d.rectangle([U(r, W - 24), 0, U(r, W - 16), r.H], fill=(14, 15, 20, 255))
    for y in range(0, int(W), 8):
        rr(r, [W - 23.5, y + 0.8, W - 16.5, y + 6.2], radius=U(r, 1.5), fill=mix(STEEL, STEEL_D, 0.25) + (255,), outline=EDGE + (255,), width=SS)
        r.d.ellipse([U(r, W - 21), U(r, y + 2.6), U(r, W - 19), U(r, y + 4.4)], fill=(14, 15, 20, 255))
    r.d.line([P(r, W - 9, 0), P(r, W - 9, W)], fill=EDGE + (255,), width=SS * 5)
    r.d.line([P(r, W - 9, 0), P(r, W - 9, W)], fill=ORGD + (255,), width=SS * 3)
    r.d.line([P(r, W - 9.6, 0), P(r, W - 9.6, W)], fill=A(ORGL, 120), width=SS)
    for y in (W * 0.33, W * 0.83):
        rr(r, [W - 12, y - 2.5, W - 6, y + 2.5], radius=U(r, 1), fill=mix(STEEL, STEEL_L, 0.2) + (255,), outline=EDGE + (255,), width=SS)
    # The trough, with cradle rollers under the round.
    r.d.rectangle([U(r, 28), 0, U(r, W - 30), r.H], fill=mix(STEEL_D, DARK, 0.5) + (255,))
    for x in (28, W - 30):
        r.d.line([P(r, x, 0), P(r, x, W)], fill=EDGE + (255,), width=SS * 3)
        r.d.line([P(r, x + (1.6 if x < c else -1.6), 0), P(r, x + (1.6 if x < c else -1.6), W)], fill=A(STEEL_L, 150), width=SS)
    for y in (24, 52, 80, 108):
        cyl_h(r, 30, W - 32, y, 5)
    shell_side(r, c - 1, 12, W - 22, 28)
    # A clamp strap over the round, bolted to both lips.
    for y in (W * 0.62,):
        rr(r, [c - 20, y - 3, c + 18, y + 3], radius=U(r, 1.5), fill=mix(STEEL, STEEL_D, 0.1) + (255,), outline=EDGE + (255,), width=SS)
        r.d.line([P(r, c - 18, y - 1.6), P(r, c + 16, y - 1.6)], fill=A(LIGHT, 150), width=SS)
        bolt(r, c - 17, y, 1.6); bolt(r, c + 15, y, 1.6)
    # Cross members on the top and bottom edges: two segments make one whole member.
    for y in (0, W):
        rr(r, [6, y - 4, W - 6, y + 4], radius=U(r, 1), fill=mix(STEEL, STEEL_D, 0.2) + (255,), outline=EDGE + (255,), width=SS)
        for x in (12, W - 12):
            bolt(r, x, y + (2 if y == 0 else -2), 1.4)
    return put(r, "autoloader.png")


def rail(r, x0, x1):
    """A steel frame rail running off the top and bottom edges."""
    r.d.rectangle([U(r, x0), 0, U(r, x1), r.H], fill=mix(STEEL, STEEL_D, 0.1) + (255,))
    r.d.line([P(r, x0, 0), P(r, x0, W)], fill=EDGE + (255,), width=SS * 2)
    r.d.line([P(r, x1, 0), P(r, x1, W)], fill=EDGE + (255,), width=SS * 2)
    r.d.line([P(r, x0 + 1.5, 0), P(r, x0 + 1.5, W)], fill=A(LIGHT, 140), width=SS)
    r.d.line([P(r, x1 - 1.5, 0), P(r, x1 - 1.5, W)], fill=A(DARK, 150), width=SS)


def blank_band(r, x0, y0, x1, y1):
    """A vertical hazard strip (hazard_band is horizontal)."""
    r.d.rectangle([U(r, x0), U(r, y0), U(r, x1), U(r, y1)], fill=CAUT + (255,))
    step = 5.0; y = y0 - step
    while y < y1:
        r.d.polygon([P(r, x0, y), P(r, x0, y + step * 0.5), P(r, x1, y + step * 0.5 + (x1 - x0)), P(r, x1, y + (x1 - x0))], fill=(34, 30, 24, 255))
        y += step
    r.d.rectangle([U(r, x0), U(r, y0), U(r, x1), U(r, y1)], outline=EDGE + (255,), width=SS)


def ammo_clip():
    r = blank(); c = W / 2
    tile_plate(r, 10, W - 10, "ac")
    for x in (38, W - 38):
        for y in (W * 0.27, W * 0.73):
            shell_base(r, x, y, 19)
    # Retaining bars across each pair.
    for y in (W * 0.27, W * 0.73):
        rr(r, [22, y - 2, W - 22, y + 2], radius=U(r, 1), fill=mix(STEEL, STEEL_D, 0.2) + (255,), outline=EDGE + (255,), width=SS)
        bolt(r, c, y, 2.2)
    return put(r, "ammo_clip.png")


def ammo_feeder():
    r = R(1, 1); c = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "af", base=PLATE)
    # Rollers across the belt.
    rr(r, [24, 14, W - 24, W - 22], radius=U(r, 4), fill=mix(STEEL_D, DARK, 0.35) + (255,), outline=EDGE + (255,), width=SS * 2)
    for y in range(20, int(W - 24), 10):
        cyl_h(r, 28, W - 28, y, 6)
    # Chevrons up: shells go this way.
    for y in (34, 58, 82):
        r.d.polygon([P(r, c, y - 9), P(r, c + 16, y + 5), P(r, c + 9, y + 5), P(r, c, y - 2), P(r, c - 9, y + 5), P(r, c - 16, y + 5)], fill=CAUT + (255,), outline=EDGE + (255,))
    # The hatch at the bottom where shells come in from the magazine.
    rr(r, [c - 22, W - 26, c + 22, W - 12], radius=U(r, 2), fill=BRASS_D + (255,), outline=EDGE + (255,), width=SS * 2)
    r.d.line([P(r, c - 18, W - 19), P(r, c + 18, W - 19)], fill=A(BRASS_L, 160), width=SS)
    return put(r, "ammo_feeder.png")


def cyl_h(r, x0, x1, y, h):
    rr(r, [x0, y - h / 2, x1, y + h / 2], radius=U(r, h / 2), fill=mix(STEEL, STEEL_D, 0.2) + (255,), outline=EDGE + (255,), width=SS)
    r.d.line([P(r, x0 + 2, y - h / 2 + 1.2), P(r, x1 - 2, y - h / 2 + 1.2)], fill=A(STEEL_L, 170), width=SS)


def conveyor():
    r = blank(); c = W / 2
    tile_plate(r, 26, W - 26, "cv", bolts=False)
    r.d.rectangle([U(r, 34), 0, U(r, W - 34), r.H], fill=mix(STEEL_D, DARK, 0.4) + (255,))
    for y in range(4, int(W), 12):
        cyl_h(r, 36, W - 36, y, 7)
    # A shell riding the belt.
    shell_base(r, c, W * 0.5, 11)
    return put(r, "conveyor.png")


# ======================================================================= HEAT AND RECOIL

def cooler():
    r = R(1, 1); c = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "co", base=PLATE)
    # Fins.
    for x in (28, 46, 64, 82, 100):
        rr(r, [x - 4, 18, x + 4, W - 18], radius=U(r, 2), fill=mix(STEEL, STEEL_L, 0.15) + (255,), outline=EDGE + (255,), width=SS)
        r.d.line([P(r, x - 2.4, 20), P(r, x - 2.4, W - 20)], fill=A(LIGHT, 170), width=SS)
    # Coolant lines between the fins, with a manifold top and bottom.
    for x in (37, 55, 73, 91):
        r.d.line([P(r, x, 22), P(r, x, W - 22)], fill=EDGE + (255,), width=SS * 9)
        r.d.line([P(r, x, 22), P(r, x, W - 22)], fill=ICE + (255,), width=SS * 6)
        r.d.line([P(r, x - 1.5, 22), P(r, x - 1.5, W - 22)], fill=A((230, 248, 255), 200), width=SS * 2)
    for y in (16, W - 16):
        rr(r, [22, y - 6, W - 22, y + 6], radius=U(r, 3), fill=ICE_D + (255,), outline=EDGE + (255,), width=SS)
        r.d.line([P(r, 26, y - 2), P(r, W - 26, y - 2)], fill=A(ICE, 200), width=SS)
    for (x, y) in ((14, 16), (W - 14, W - 16)):
        pipe(r, x, y, x + (10 if x < c else -10), y, w=5, col=ICE_D)
    return put(r, "cooler.png")


def recoil_absorber():
    r = R(1, 1); c = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "ra", base=PLATE)
    # Cylinder body, the spring around the rod, the rod pointing up to the breech.
    rr(r, [c - 16, 52, c + 16, W - 14], radius=U(r, 6), fill=mix(STEEL, STEEL_D, 0.5) + (255,), outline=EDGE + (255,), width=SS * 2)
    r.d.line([P(r, c - 13, 56), P(r, c - 13, W - 18)], fill=A(LIGHT, 140), width=SS)
    rr(r, [c - 4, 8, c + 4, 60], radius=U(r, 2), fill=mix(STEEL_L, STEEL, 0.3) + (255,), outline=EDGE + (255,), width=SS * 2)
    r.d.line([P(r, c - 2, 10), P(r, c - 2, 58)], fill=A((255, 255, 255), 150), width=SS)
    pts = []
    for k in range(9):
        y = 24 + k * 10.0
        x = c - 20 if k % 2 == 0 else c + 20
        pts.append(P(r, x, y))
    r.d.line(pts, fill=EDGE + (255,), width=SS * 10, joint="curve")
    r.d.line(pts, fill=CAUT + (255,), width=SS * 6, joint="curve")
    r.d.line(pts, fill=A((250, 222, 140), 180), width=SS * 2, joint="curve")
    for y in (66, W - 26):
        rr(r, [c - 22, y - 4, c + 22, y + 4], radius=U(r, 2), fill=mix(STEEL, STEEL_D, 0.3) + (255,), outline=EDGE + (255,), width=SS * 2)
        bolt(r, c - 18, y, 1.6); bolt(r, c + 18, y, 1.6)
    # Hydraulic line.
    pipe(r, c + 18, W - 40, W - 16, W - 40, w=3, col=ORGD)
    return put(r, "recoil_absorber.png")


# ======================================================================= THE TUBE

def barrel():
    r = blank(); c = W / 2
    w = 36
    r.d.rectangle([U(r, c - w / 2), 0, U(r, c + w / 2), r.H], fill=mix(STEEL_D, STEEL, 0.25) + (255,))
    r.d.line([P(r, c - w / 2, 0), P(r, c - w / 2, W)], fill=EDGE + (255,), width=SS * 2)
    r.d.line([P(r, c + w / 2, 0), P(r, c + w / 2, W)], fill=EDGE + (255,), width=SS * 2)
    # Round tube shading: light down the left, dark down the right.
    for i, t in enumerate((0.18, 0.28, 0.36)):
        x = c - w / 2 + w * t
        r.d.line([P(r, x, 0), P(r, x, W)], fill=A(STEEL_L, 150 - i * 40), width=SS * 2)
    r.d.line([P(r, c + w * 0.32, 0), P(r, c + w * 0.32, W)], fill=A(DARK, 150), width=SS * 3)
    # One reinforcing band near the top.
    rr(r, [c - w / 2 - 3, 18, c + w / 2 + 3, 28], radius=U(r, 2), fill=mix(STEEL, STEEL_D, 0.2) + (255,), outline=EDGE + (255,), width=SS * 2)
    r.d.line([P(r, c - w / 2 - 1, 20), P(r, c + w / 2 + 1, 20)], fill=A(LIGHT, 150), width=SS)
    return put(r, "barrel.png")


def muzzle_brake():
    r = blank(); c = W / 2
    w = 36
    # The barrel coming in from below.
    r.d.rectangle([U(r, c - w / 2), U(r, 70), U(r, c + w / 2), r.H], fill=mix(STEEL_D, STEEL, 0.25) + (255,), outline=EDGE + (255,), width=SS * 2)
    r.d.line([P(r, c - w / 2 + 5, 70), P(r, c - w / 2 + 5, W)], fill=A(STEEL_L, 140), width=SS * 2)
    # The brake: wider, with three pairs of side vents, bore at the top edge.
    rr(r, [c - 28, 8, c + 28, 78], radius=U(r, 4), fill=mix(STEEL, STEEL_D, 0.35) + (255,), outline=EDGE + (255,), width=SS * 2)
    r.d.line([P(r, c - 25, 12), P(r, c - 25, 74)], fill=A(LIGHT, 150), width=SS)
    for y in (22, 40, 58):
        for sx in (-1, 1):
            x0 = c + sx * 28 - (7 if sx > 0 else 0)
            rr(r, [x0, y - 5, x0 + 7, y + 5], radius=U(r, 1), fill=(10, 10, 14, 255))
            soot(r, c + sx * 30, y, 6, 5)
    r.d.rectangle([U(r, c - 7), U(r, 4), U(r, c + 7), U(r, 10)], fill=(6, 7, 10, 255), outline=EDGE + (255,), width=SS)
    return put(r, "muzzle_brake.png")


def mantlet():
    r = R(1, 1); c = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "mt", base=mix(STEEL_D, STEEL, 0.3))
    # A thick cast shield, sloped: lit face up top, dark face below.
    pts = [P(r, 14, 30), P(r, 30, 12), P(r, W - 30, 12), P(r, W - 14, 30), P(r, W - 14, W - 22), P(r, 14, W - 22)]
    r.d.polygon(pts, fill=mix(STEEL, STEEL_D, 0.2) + (255,), outline=EDGE + (255,))
    r.d.polygon([P(r, 18, 30), P(r, 32, 16), P(r, W - 32, 16), P(r, W - 18, 30)], fill=mix(STEEL, LIGHT, 0.25) + (255,))
    r.d.rectangle([U(r, 16), U(r, W - 44), U(r, W - 16), U(r, W - 24)], fill=mix(STEEL_D, DARK, 0.35) + (255,))
    metal_noise(r, c, c, 50, "mtn")
    # The slot the barrel runs through.
    rr(r, [c - 8, 6, c + 8, W - 36], radius=U(r, 3), fill=(8, 9, 12, 255), outline=EDGE + (255,), width=SS * 2)
    for y in (40, 70):
        bolt(r, 24, y, 2.4); bolt(r, W - 24, y, 2.4)
    return put(r, "mantlet.png")


def rail_magnet():
    r = R(1, 1); c = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "rm", base=PLATE)
    # Two copper coil packs either side of a central rail channel.
    for x0 in (16, W - 46):
        rr(r, [x0, 14, x0 + 30, W - 14], radius=U(r, 4), fill=mix(STEEL_D, DARK, 0.3) + (255,), outline=EDGE + (255,), width=SS * 2)
        y = 18.0; k = 0
        while y < W - 20:
            rr(r, [x0 + 3, y, x0 + 27, y + 4], radius=U(r, 1.5), fill=(COP_CL if k % 2 == 0 else COP_C) + (255,))
            y += 6.5; k += 1
    r.d.rectangle([U(r, c - 6), U(r, 8), U(r, c + 6), U(r, W - 8)], fill=(8, 10, 14, 255), outline=EDGE + (255,), width=SS * 2)
    for y in (30, 63, 96):
        small_glow(r, c, y, 4.5, CY, (220, 246, 255))
    return put(r, "rail_magnet.png")


# ======================================================================= TURRET

def traverse_motor():
    r = R(1, 1); c = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "tm", base=PLATE)
    # Motor drum with cooling ribs, and the pinion gear that walks the ring.
    rr(r, [16, 22, 70, W - 22], radius=U(r, 8), fill=mix(STEEL_D, DARK, 0.15) + (255,), outline=EDGE + (255,), width=SS * 2)
    for y in range(30, int(W - 26), 7):
        r.d.line([P(r, 20, y), P(r, 66, y)], fill=A(LIGHT, 90), width=SS)
        r.d.line([P(r, 20, y + 1.5), P(r, 66, y + 1.5)], fill=A(DARK, 140), width=SS)
    gx, gy, gr = W - 36, c, 24
    teeth = []
    for k in range(32):
        a = math.radians(k * 360 / 32)
        rad = gr if k % 2 == 0 else gr - 5
        teeth.append(P(r, gx + math.cos(a) * rad, gy + math.sin(a) * rad))
    r.d.polygon(teeth, fill=mix(STEEL, STEEL_L, 0.2) + (255,), outline=EDGE + (255,))
    r.d.ellipse([U(r, gx - 12), U(r, gy - 12), U(r, gx + 12), U(r, gy + 12)], fill=mix(STEEL_D, DARK, 0.2) + (255,), outline=EDGE + (255,), width=SS * 2)
    r.d.ellipse([U(r, gx - 5), U(r, gy - 5), U(r, gx + 5), U(r, gy + 5)], fill=BRASS + (255,), outline=EDGE + (255,), width=SS)
    pipe(r, 70, c, gx - 12, c, w=6, col=STEEL)
    led(r, 26, 30, GRN, 1.8)
    return put(r, "traverse_motor.png")


def turret_ring():
    r = R(1, 1); c = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "trg", base=mix(STEEL_D, DARK, 0.2))
    # The race: an outer ring gear, a brass bearing track, the deck seat inside.
    teeth = []
    for k in range(72):
        a = math.radians(k * 5)
        rad = 54 if k % 2 == 0 else 50
        teeth.append(P(r, c + math.cos(a) * rad, c + math.sin(a) * rad))
    r.d.polygon(teeth, fill=mix(STEEL, STEEL_D, 0.3) + (255,), outline=EDGE + (255,))
    ring(r, c, c, 44, 6, BRASS)
    for k in range(24):
        a = math.radians(k * 15)
        x, y = c + math.cos(a) * 44, c + math.sin(a) * 44
        r.d.ellipse([U(r, x - 1.6), U(r, y - 1.6), U(r, x + 1.6), U(r, y + 1.6)], fill=BRASS_L + (255,))
    r.d.ellipse([U(r, c - 37), U(r, c - 37), U(r, c + 37), U(r, c + 37)], fill=PLATE + (255,), outline=EDGE + (255,), width=SS * 2)
    metal_noise(r, c, c, 36, "trn")
    for k in range(8):
        a = math.radians(k * 45)
        bolt(r, c + math.cos(a) * 30, c + math.sin(a) * 30, 2.0)
    # Training marks.
    for k in range(4):
        a = math.radians(k * 90 - 90)
        r.d.line([P(r, c + math.cos(a) * 18, c + math.sin(a) * 18), P(r, c + math.cos(a) * 26, c + math.sin(a) * 26)], fill=CAUT + (255,), width=SS * 2)
    return put(r, "turret_ring.png")


def ammo_hoist():
    r = R(1, 1); c = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "ah", base=PLATE)
    # A round trunk going down, a lift cage in it carrying a round up.
    ring(r, c, c, 42, 6, mix(STEEL, STEEL_D, 0.2))
    r.d.ellipse([U(r, c - 36), U(r, c - 36), U(r, c + 36), U(r, c + 36)], fill=(12, 13, 18, 255))
    for k in range(4):
        a = math.radians(k * 90 + 45)
        r.d.line([P(r, c + math.cos(a) * 14, c + math.sin(a) * 14), P(r, c + math.cos(a) * 36, c + math.sin(a) * 36)], fill=mix(STEEL, STEEL_D, 0.3) + (255,), width=SS * 4)
    rr(r, [c - 20, c - 20, c + 20, c + 20], radius=U(r, 3), outline=BRASS + (255,), width=SS * 3)
    shell_base(r, c, c, 12)
    hazard_band(r, 16, W - 14, W - 16, W - 8)
    return put(r, "ammo_hoist.png")


def anti_flash_door():
    r = R(1, 1); c = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "afd", base=PLATE)
    # Two heavy leaves meeting in the middle, hazard striped, a dog lever each.
    for x0, x1 in ((16, c - 1), (c + 1, W - 16)):
        rr(r, [x0, 16, x1, W - 16], radius=U(r, 3), fill=CAUT + (255,), outline=EDGE + (255,), width=SS * 2)
    g, gd = layer(r)
    step = 14.0; x = -W
    while x < W * 2:
        gd.polygon([P(r, x, W - 16), P(r, x + step * 0.5, W - 16), P(r, x + step * 0.5 + (W - 32), 16), P(r, x + (W - 32), 16)], fill=(34, 30, 24, 255))
        x += step
    mask = Image.new("L", r.img.size, 0)
    ImageDraw.Draw(mask).rectangle([U(r, 17), U(r, 17), U(r, W - 17), U(r, W - 17)], fill=255)
    g.putalpha(Image.composite(g.getchannel("A"), Image.new("L", r.img.size, 0), mask))
    compose(r, g)
    r.d.line([P(r, c, 16), P(r, c, W - 16)], fill=EDGE + (255,), width=SS * 3)
    for x in (c - 14, c + 14):
        rr(r, [x - 4, c - 14, x + 4, c + 14], radius=U(r, 2), fill=mix(STEEL, STEEL_D, 0.2) + (255,), outline=EDGE + (255,), width=SS * 2)
        bolt(r, x, c - 9, 1.8)
    for y in (24, W - 24):
        for x in (20, W - 20):
            bolt(r, x, y, 1.8)
    return put(r, "anti_flash_door.png")


def ammo_store():
    r = R(1, 1); c = W / 2
    fill_block(r, 6, 6, W - 6, W - 6, "as", base=mix(STEEL_D, DARK, 0.15), haz_bottom=True)
    # Racked rounds seen end on, three rows of three.
    for row in range(3):
        for col in range(3):
            shell_base(r, 30 + col * 33, 26 + row * 30, 11)
    for y in (41, 71):
        r.d.line([P(r, 14, y), P(r, W - 14, y)], fill=mix(STEEL, STEEL_D, 0.3) + (255,), width=SS * 3)
    return put(r, "ammo_store.png")


def turret_plating():
    """Deck plate: diamond tread, a bevelled border, countersunk bolts, scuffs.
    Tiles in every direction (the border is the seam between plates)."""
    r = blank(); c = W / 2
    r.d.rectangle([0, 0, r.W, r.H], fill=PLATE + (255,))
    metal_noise(r, c, c, 62, "tp")
    # Tread: raised lozenges at alternating angles, the classic deck plate.
    g, gd = layer(r)
    step = 10.0
    for j in range(13):
        for i in range(13):
            cx_, cy_ = 8 + i * step + (step / 2 if j % 2 else 0), 8 + j * step
            if not (9 < cx_ < W - 9 and 9 < cy_ < W - 9):
                continue
            ang = math.radians(45 if (i + j) % 2 == 0 else -45)
            ux, uy = math.cos(ang) * 3.8, math.sin(ang) * 3.8
            nx, ny = -uy * 0.36, ux * 0.36
            body = [P(r, cx_ - ux, cy_ - uy), P(r, cx_ + nx, cy_ + ny), P(r, cx_ + ux, cy_ + uy), P(r, cx_ - nx, cy_ - ny)]
            gd.polygon(body, fill=mix(PLATE, LIGHT, 0.42) + (255,))
            gd.line([P(r, cx_ - ux, cy_ - uy), P(r, cx_ - nx, cy_ - ny), P(r, cx_ + ux, cy_ + uy)], fill=A(DARK, 230), width=int(SS * 1.6))
    compose(r, g)
    # Scuffs where crews walk: short light scratches at random angles, no streaks.
    rng = random.Random("tps"); g, gd = layer(r)
    for _ in range(14):
        x, y = rng.uniform(16, W - 16), rng.uniform(16, W - 16)
        a = rng.uniform(0, math.pi); L = rng.uniform(3, 9)
        gd.line([P(r, x, y), P(r, x + math.cos(a) * L, y + math.sin(a) * L)], fill=A(LIGHT, rng.choice([40, 60, 80])), width=SS)
    compose(r, blur(g, 0.25))
    # The border: a machined frame with a light top-left and dark bottom-right.
    for inset, col, a in ((0, EDGE, 230), (2.2, LIGHT, 90)):
        r.d.rectangle([U(r, inset), U(r, inset), r.W - 1 - U(r, inset), r.H - 1 - U(r, inset)], outline=A(col, a), width=SS * 2 if inset == 0 else SS)
    r.d.line([P(r, 3, W - 3.5), P(r, W - 3, W - 3.5)], fill=A(DARK, 160), width=SS)
    r.d.line([P(r, W - 3.5, 3), P(r, W - 3.5, W - 3)], fill=A(DARK, 160), width=SS)
    for (x, y) in ((9, 9), (W - 9, 9), (9, W - 9), (W - 9, W - 9), (c, 6), (c, W - 6), (6, c), (W - 6, c)):
        r.d.ellipse([U(r, x - 2.6), U(r, y - 2.6), U(r, x + 2.6), U(r, y + 2.6)], fill=mix(PLATE, DARK, 0.45) + (255,))
        bolt(r, x, y, 1.6)
    return put(r, "turret_plating.png")


def turret_armour():
    """Thick cast plate: a raised slab with chamfered edges, a weld bead round
    the face, heavy bolts with washers and a lifting eye."""
    r = blank(); c = W / 2
    face = mix(STEEL_D, STEEL, 0.3)
    r.d.rectangle([0, 0, r.W, r.H], fill=mix(STEEL_D, DARK, 0.3) + (255,))
    ch = 10
    # Chamfers: light on top and left, dark on the bottom and right.
    for pts, col in (
        ([(1, 1), (W - 1, 1), (W - ch, ch), (ch, ch)], mix(face, LIGHT, 0.35)),
        ([(1, 1), (ch, ch), (ch, W - ch), (1, W - 1)], mix(face, LIGHT, 0.2)),
        ([(1, W - 1), (ch, W - ch), (W - ch, W - ch), (W - 1, W - 1)], mix(face, DARK, 0.5)),
        ([(W - 1, 1), (W - 1, W - 1), (W - ch, W - ch), (W - ch, ch)], mix(face, DARK, 0.38)),
    ):
        r.d.polygon([P(r, x, y) for x, y in pts], fill=col + (255,), outline=EDGE + (255,))
    r.d.rectangle([U(r, ch), U(r, ch), U(r, W - ch), U(r, W - ch)], fill=face + (255,), outline=EDGE + (255,), width=SS)
    # Cast surface: soft mottling plus the usual fine noise.
    rng = random.Random("tam"); g, gd = layer(r)
    for _ in range(30):
        x, y, rad = rng.uniform(ch, W - ch), rng.uniform(ch, W - ch), rng.uniform(4, 11)
        gd.ellipse([U(r, x - rad), U(r, y - rad), U(r, x + rad), U(r, y + rad)], fill=A(rng.choice([DARK, LIGHT]), rng.choice([6, 9, 12])))
    compose(r, blur(g, 3.5))
    metal_noise(r, c, c, W / 2 - ch, "ta")
    # Weld bead just inside the chamfer.
    for k in range(int((W - 2 * ch - 6) / 3.2) + 1):
        t = ch + 3 + k * 3.2
        for (x, y) in ((t, ch + 3), (t, W - ch - 3), (ch + 3, t), (W - ch - 3, t)):
            r.d.ellipse([U(r, x - 1.5), U(r, y - 1.1), U(r, x + 1.5), U(r, y + 1.1)], fill=mix(face, LIGHT, 0.3) + (255,), outline=A(EDGE, 160))
    for (x, y) in ((ch + 12, ch + 12), (W - ch - 12, ch + 12), (ch + 12, W - ch - 12), (W - ch - 12, W - ch - 12)):
        r.d.ellipse([U(r, x - 5), U(r, y - 5), U(r, x + 5), U(r, y + 5)], fill=mix(face, DARK, 0.3) + (255,), outline=EDGE + (255,), width=SS)
        bolt(r, x, y, 3.2)
    # Lifting eye in the middle.
    ring(r, c, c, 7, 2.6, mix(STEEL, STEEL_L, 0.2))
    r.d.ellipse([U(r, c - 4), U(r, c - 4), U(r, c + 4), U(r, c + 4)], fill=mix(face, DARK, 0.5) + (255,))
    return put(r, "turret_armour.png")


def turret_armour_wedge():
    """The armour's 45-degree piece: the bottom-left half of the cast plate,
    with a chamfer along the cut (top-left to bottom-right)."""
    r = blank(); c = W / 2
    face = mix(STEEL_D, STEEL, 0.3)
    tri = [P(r, 1, 1), P(r, W - 1, W - 1), P(r, 1, W - 1)]
    r.d.polygon(tri, fill=face + (255,), outline=EDGE + (255,))
    rng = random.Random("taw"); g, gd = layer(r)
    for _ in range(18):
        x, y, rad = rng.uniform(6, W * 0.6), rng.uniform(W * 0.4, W - 6), rng.uniform(4, 11)
        gd.ellipse([U(r, x - rad), U(r, y - rad), U(r, x + rad), U(r, y + rad)], fill=A(rng.choice([DARK, LIGHT]), rng.choice([6, 9, 12])))
    compose(r, blur(g, 3.5))
    # Chamfers on the two straight edges and a lit bevel along the cut.
    ch = 10
    r.d.polygon([P(r, 1, 1), P(r, ch, ch + 4), P(r, ch, W - ch), P(r, 1, W - 1)], fill=mix(face, LIGHT, 0.2) + (255,), outline=EDGE + (255,))
    r.d.polygon([P(r, 1, W - 1), P(r, ch, W - ch), P(r, W - ch - 4, W - ch), P(r, W - 1, W - 1)], fill=mix(face, DARK, 0.5) + (255,), outline=EDGE + (255,))
    r.d.polygon([P(r, 1, 1), P(r, W - 1, W - 1), P(r, W - ch - 4, W - ch), P(r, ch, ch + 4)], fill=mix(face, LIGHT, 0.4) + (255,), outline=EDGE + (255,))
    for k in range(int((W - 2 * ch - 10) / 3.2)):
        t = ch + 6 + k * 3.2
        for (x, y) in ((ch + 3, t), (t, W - ch - 3)):
            if y > x + 6:
                r.d.ellipse([U(r, x - 1.5), U(r, y - 1.1), U(r, x + 1.5), U(r, y + 1.1)], fill=mix(face, LIGHT, 0.3) + (255,), outline=A(EDGE, 160))
    for (x, y) in ((ch + 12, W - ch - 12), (ch + 12, c + 4), (c - 4, W - ch - 12)):
        r.d.ellipse([U(r, x - 5), U(r, y - 5), U(r, x + 5), U(r, y + 5)], fill=mix(face, DARK, 0.3) + (255,), outline=EDGE + (255,), width=SS)
        bolt(r, x, y, 3.2)
    return put(r, "turret_armour_wedge.png")


PARTS = [
    firing_piece, gauge_increaser, gauge_reducer, autoloader, ammo_clip, ammo_feeder,
    conveyor, cooler, recoil_absorber, barrel, muzzle_brake, mantlet, rail_magnet,
    traverse_motor, turret_ring, ammo_hoist, anti_flash_door, ammo_store,
    turret_plating, turret_armour, turret_armour_wedge,
]


def render_all():
    return {f.__name__ + ".png": f() for f in PARTS}


if __name__ == "__main__":
    out = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else os.path.join(os.path.dirname(os.path.abspath(__file__)), "preview", "gun_parts")
    os.makedirs(out, exist_ok=True)
    sprites = render_all()
    for name, im in sprites.items():
        im.save(os.path.join(out, name))
        if "--install" in sys.argv:
            im.save(os.path.join(GAME, name))
    print(f"{len(sprites)} gun parts -> {out}" + (f" and {GAME}" if "--install" in sys.argv else ""))
