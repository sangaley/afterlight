"""Shared pieces for the fleet concept tools: the game's module data, footprints,
and a renderer that draws a Blueprint the way the game does.

Everything here is read out of the Rust source at import time, so a module's
footprint or sprite can't drift from what the game spawns:
  - sprite per ModuleType          src/sprite_map.rs  module_sprite_path
  - turret barrel sprites          src/sprite_map.rs  turret_barrel_sprite
  - footprint size per ModuleType  src/building/registry.rs  ModuleDef.size

Conventions (same as the game): +x is the bow, +y is up, one cell is 66 world
units, Rotation::to_radians is N=0, E=-pi/2, S=pi, W=+pi/2.
"""
import math
import os
import re

from PIL import Image, ImageDraw

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", ".."))
ASSETS = os.path.join(ROOT, "assets")
CELL = 66.0
HULL_TONE = (59, 69, 81)

ROT_RAD = {"North": 0.0, "East": -math.pi / 2, "South": math.pi, "West": math.pi / 2}


def _src(path):
    with open(os.path.join(ROOT, path)) as f:
        return f.read()


def _load_sprites():
    text = _src("src/sprite_map.rs")
    body = text[text.index("pub fn module_sprite_path"):]
    body = body[: body.index("\n}\n")]
    sprites = {}
    for arm in re.finditer(r"((?:ModuleType::\w+\s*\|?\s*)+)=>\s*\"([^\"]+)\"", body):
        for name in re.findall(r"ModuleType::(\w+)", arm.group(1)):
            sprites[name] = arm.group(2)
    barrels = {}
    body = text[text.index("pub fn turret_barrel_sprite"):]
    body = body[: body.index("\n}\n")]
    for a, b in re.findall(r'Some\("([^"]+)"\)\s*=>\s*Some\("([^"]+)"\)', body):
        barrels[a] = b
    return sprites, barrels


def _load_sizes():
    text = _src("src/building/registry.rs")
    sizes = {}
    for m in re.finditer(r"defs\.insert\(ModuleType::(\w+),\s*ModuleDef\s*\{(.*?)\n    \}\);", text, re.S):
        s = re.search(r"size:\s*IVec2::new\((\d+),\s*(\d+)\)", m.group(2))
        if s:
            sizes[m.group(1)] = (int(s.group(1)), int(s.group(2)))
    return sizes


SPRITES, BARRELS = _load_sprites()
SIZES = _load_sizes()

ENGINES = {"SmallEngine", "StandardEngine", "LargeEngine", "SilentDrive", "ManeuveringThruster",
           "JetDrive", "EmergencyThruster", "RudderAssembly", "ManeuverThruster"}
OVERHANG = {
    "sprites/modules/standard_engine.png": (30.0, -1.0),
    "sprites/modules/silent_drive.png": (24.0, -1.0),
    "sprites/modules/torpedo_tube.png": (26.0, 1.0),
    "sprites/modules/heavy_missile.png": (26.0, 1.0),
    "sprites/modules/guided_missile.png": (26.0, 1.0),
    "sprites/modules/cluster_rocket.png": (26.0, 1.0),
    "sprites/modules/salvage_arm.png": (34.0, 1.0),
}
WEDGES = {"AngledArmorPlate", "AngledHullPlate"}
LAUNCHERS = {"HeavyMissile", "GuidedMissile", "ClusterRocket", "TorpedoTube"}
POSTS = {  # crew stations (go ON the nav map as posts)
    "StandardReactor", "SmallReactor", "LargeReactor", "FusionReactor", "StandardEngine",
    "LargeEngine", "BridgeWing", "HelmStation", "NavigationConsole", "Cannon", "Gatling",
    "Railgun", "Coilgun", "Laser", "PlasmaCaster", "IonDisruptor", "HeavyMissile",
    "GuidedMissile", "ClusterRocket", "BasicQuarters", "Barracks", "OfficerQuarters",
    "MedBay", "SurgicalBay", "GalleyMess", "MessHall", "RadarArray", "OxygenScrubber",
    "RepairBay", "SalvageArm", "TractorBeam", "EMPPulse", "PlasmaCaster", "MiningDrill",
    "ResearchLab", "CreatureContainment", "AdvancedRepairBay", "AICombatCore",
}


def rotate_offset(x, y, rot):
    return {"North": (x, y), "East": (y, -x), "South": (-x, -y), "West": (-y, x)}[rot]


def footprint(kind, origin, rot):
    w, h = SIZES.get(kind, (1, 1))
    ox, oy = origin
    out = []
    for x in range(w):
        for y in range(h):
            dx, dy = rotate_offset(x, y, rot)
            out.append((ox + dx, oy + dy))
    return out


def base_rotation(kind):
    return -math.pi if kind in ENGINES else 0.0


# --------------------------------------------------------------------------- render

_img_cache = {}


def _img(rel):
    if rel not in _img_cache:
        _img_cache[rel] = Image.open(os.path.join(ASSETS, rel)).convert("RGBA")
    return _img_cache[rel]


def hull_sprite(cell):
    if cell["layer"] == "Hallway":
        return "sprites/hull/hull_hallway"
    return "sprites/hull/hull_" + {"Steel": "steel", "Titanium": "titanium",
                                   "Composite": "composite", "AbyssalAlloy": "abyssal"}[cell["material"]]


def bounds(design, pad=1):
    pts = [tuple(c["grid_pos"]) for c in design["hull_cells"]]
    for m in design["modules"]:
        pts += footprint(m["module_type"], tuple(m["grid_pos"]), m["rotation"])
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    return min(xs) - pad, max(xs) + pad, min(ys) - pad, max(ys) + pad


def render(design, px=24, pad=1, background=None):
    """Draw a Blueprint dict. px = pixels per cell."""
    x0, x1, y0, y1 = bounds(design, pad)
    W = int((x1 - x0 + 1) * px)
    H = int((y1 - y0 + 1) * px)
    img = Image.new("RGBA", (W, H), background or (0, 0, 0, 0))
    s = px / CELL

    def to_px(wx, wy):  # world units (cell centre = x*66, y*66) -> image px
        return (wx - (x0 - 0.5) * CELL) * s, ((y1 + 0.5) * CELL - wy) * s

    # Hull, with seam variants chosen by matching neighbours.
    face = {tuple(c["grid_pos"]): hull_sprite(c) for c in design["hull_cells"]}
    for (x, y), base in face.items():
        mask = 0
        for bit, (dx, dy) in ((1, (0, 1)), (2, (0, -1)), (4, (-1, 0)), (8, (1, 0))):
            if face.get((x + dx, y + dy)) == base:
                mask |= bit
        tile = _img(f"{base}_{mask:02d}.png").resize((px + 1, px + 1), Image.LANCZOS)
        cx, cy = to_px(x * CELL, y * CELL)
        img.alpha_composite(tile, (int(round(cx - px / 2)), int(round(cy - px / 2))))

    def blit(sprite, w, h, angle, wx, wy):
        w_px, h_px = max(1, int(round(w * s))), max(1, int(round(h * s)))
        im = sprite.resize((w_px, h_px), Image.LANCZOS).rotate(math.degrees(angle), Image.BICUBIC, expand=True)
        cx, cy = to_px(wx, wy)
        img.alpha_composite(im, (int(round(cx - im.width / 2)), int(round(cy - im.height / 2))))

    # Modules: flat ones first, then protruding parts, then barrels (the game's z order).
    layers = ([], [], [])
    for m in design["modules"]:
        kind, rot = m["module_type"], m["rotation"]
        cells = footprint(kind, tuple(m["grid_pos"]), rot)
        mnx, mxx = min(c[0] for c in cells), max(c[0] for c in cells)
        mny, mxy = min(c[1] for c in cells), max(c[1] for c in cells)
        cx, cy = (mnx + mxx) / 2 * CELL, (mny + mxy) / 2 * CELL
        angle = ROT_RAD[rot] + base_rotation(kind)
        if kind in WEDGES:
            layers[0].append(("wedge", cx, cy, ROT_RAD[rot]))
            continue
        path = SPRITES.get(kind, "sprites/modules/small_reactor.png")
        lw, lh = CELL + (mxx - mnx) * CELL, CELL + (mxy - mny) * CELL
        quarter = round(angle / (math.pi / 2)) % 4
        fw, fh = (lh, lw) if quarter % 2 == 1 else (lw, lh)
        over, protrude = OVERHANG.get(path, (0.0, 0.0))
        off = protrude * over * 0.5
        ox, oy = -math.sin(angle) * off, math.cos(angle) * off
        layers[1 if over else 0].append(("sprite", path, fw, fh + over, angle, cx + ox, cy + oy))
        if path in BARRELS:
            layers[2].append(("sprite", BARRELS[path], 132.0, 132.0, angle, cx, cy))

    draw = ImageDraw.Draw(img)
    for layer in layers:
        for item in layer:
            if item[0] == "sprite":
                _, path, w, h, angle, wx, wy = item
                blit(_img(path), w, h, angle, wx, wy)
            else:
                _, wx, wy, angle = item
                hh = CELL / 2
                ca, sa = math.cos(angle), math.sin(angle)
                tri = [(-hh, hh), (-hh, -hh), (hh, -hh)]  # material half: x + y <= 0
                pts = [to_px(wx + px_ * ca - py_ * sa, wy + px_ * sa + py_ * ca) for px_, py_ in tri]
                draw.polygon(pts, fill=HULL_TONE + (255,))
                lit = tuple(int(c + (255 - c) * 0.30) for c in HULL_TONE)
                draw.line([pts[0], pts[2]], fill=lit + (255,), width=max(1, int(round(3 * s))))
    return img


def load(path):
    import json
    with open(path) as f:
        return json.load(f)
