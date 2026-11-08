"""Doubles every ship in the game and arms it with the new weapons.

Each ship is grown to the new scale by drawing it twice as big: every cell
of its hand-drawn hull becomes a 2x2 block, so its silhouette, rooms,
corridors and armour belts stay exactly where they were drawn, and every
system scales together (four reactors where there was one, four bunks,
four engines). A 45-degree armour plate becomes a full plate and two angled
ones along the same line, so angled faces stay straight.

The old one-block guns, missiles and lasers are taken out and their cells
become slots for the new weapons, placed as the builder's pre-made weapons
(designs/kits) in each faction's style:

- Gatling           -> Light Autocannon turrets
- Cannon, Railgun   -> 170 mm turrets on a 3x3 ring where there's room,
                       else the Compact 170 mm; a big enough block of them
                       takes a Twin 225 mm turret
- missiles          -> VLS silos with the faction's missile
- Laser             -> laser units firing the way the old laser faced
- PD drone          -> CIWS turrets

The exotics -- Ion Disruptor, Plasma Caster, EMP Pulse, Tractor Beam,
Mining Drill -- are faction identity with no new version; they scale like
everything else. An Ammo Bay scales to one bay and three Supply Crates, so
cook-offs don't grow fourfold. Every armed ship gets point defence (CIWS)
and Missile Warners with a decoy and chaff beside one.

Sources: each ship as approved (git 957d6bb, before any weapon swaps); the
Eternal Vigil as built (it came later). Writes designs/factions/*/*.json
and the player's starter, pincer, crab and trident.

Run from afterlight/: python3 tools/ship-scale/scale.py [--dry]
"""
import json, os, subprocess, sys
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
sys.path.insert(0, os.path.join(ROOT, "tools/faction-arming"))
import kit  # noqa: E402  (module sizes from the registry)

SOURCE_REV = "957d6bb"
# Ships that came after it (the Eternal Vigil): as they stood before any
# scaling. Never the working file -- that's already doubled, and running
# this again would double it again.
LATER_REV = "25d6467"
# Faction stations are drawn at ship scale and built three times as big
# (they should dwarf the ships round them); the files now hold them built.
STATIONS = {("terran_hegemony", "bastion"), ("eternal_hegemony", "citadel"), ("gilded_throne", "exchange"),
            ("broken_choir", "broken_spire"), ("synthesis_collective", "growth_node"), ("corpse_stars", "reliquary"),
            ("stellar_preserve", "sanctuary"), ("the_silence", "monastery"), ("shepherd", "the_fold"),
            ("merchant_princes", "trade_post"), ("free_haulers_compact", "depot")}
PLAYER = ["starter", "pincer", "crab", "trident"]

SWAP = {"Gatling": "gun", "Cannon": "cannon", "Railgun": "cannon", "HeavyMissile": "missile",
        "ClusterRocket": "missile", "GuidedMissile": "missile", "Laser": "laser", "PointDefenseDrone": "pd"}
# The Vigil was built with the new weapons: they are its slots.
NEW_SYSTEMS = {
    "TurretRingSmall": "pd", "TurretRing": "cannon", "TurretRingLarge": "cannon", "FiringPiece": "cannon",
    "GaugeIncreaser": "cannon", "GaugeReducer": "cannon", "Loader": "cannon", "ClipRack": "cannon",
    "ShellFeeder": "cannon", "BreechCooler": "cannon", "RecoilAbsorber": "cannon", "GunBarrel": "cannon",
    "MuzzleBrake": "cannon", "Mantlet": "cannon", "RailMagnet": "cannon", "TraverseMotor": "cannon",
    "AmmoHoist": "cannon", "Magazine": "cannon", "AntiFlashDoor": "cannon", "BeltConveyor": "cannon",
    "LaunchPad": "missile", "RailLauncher": "missile", "VlsSilo": "missile",
    "MissileWarner": "defence", "DecoyLauncher": "defence", "ChaffDispenser": "defence",
}
# The Vigil's installs, by what stood at their heart: one kit each.
ANCHOR_KIT = {"TurretRingSmall": "ciws", "TurretRing": "turret", "TurretRingLarge": "twin",
              "LaunchPad": "vls", "RailLauncher": "vls", "VlsSilo": "vls", "FiringPiece": "spinal"}
# What crew walk on: never built over.
WALKED = {"Hallway", "BulkheadDoor"}
EXOTIC = {"IonDisruptor", "PlasmaCaster", "EMPPulse", "TractorBeam", "MiningDrill"}
# One stays where there was one; the rest of its 2x2 becomes supplies.
ONCE = {"AmmoBay": "SupplyCrates"}

MISSILES = {
    "broken_choir": {"motor": "Booster", "body": ["Frag", "Frag"], "head": "Heat"},
    "corpse_stars": {"motor": "Booster", "body": ["Emp", "Fuel"], "head": "Radar"},
    "merchant_princes": {"motor": "Booster", "body": ["Fins", "He"], "head": "Radar"},
}
# Faction guns where they have their own (designs/guns), else the kits'.
FACTION_GUNS = {
    "eternal_hegemony": {"ciws": "eternal_ciws.json", "turret": "eternal_170_mm_lance.json"},
    "terran_hegemony": {"ciws": "terran_ciws.json"},
}
# Two modifiers on a full laser bank's core, by faction.
LASER_FLAVOUR = {
    "gilded_throne": ("QSwitch", "FrequencyDoubler"),
    "synthesis_collective": ("FrequencyDoubler", "LaserDestabiliser"),
    "shepherd": ("QSwitch", "LaserDestabiliser"),
}

ROT_CW = {"North": "East", "East": "South", "South": "West", "West": "North"}
K_OF = {"East": 0, "South": 1, "West": 2, "North": 3}
# The corner an angled plate leaves open, by its rotation (read off the
# drawn ships: every one agrees).
OPEN_CORNER = {"North": (1, 1), "East": (1, -1), "South": (-1, -1), "West": (-1, 1)}


def turn(v, k):
    x, y = v
    for _ in range(k % 4):
        x, y = y, -x
    return x, y


def turn_rot(r, k):
    for _ in range(k % 4):
        r = ROT_CW[r]
    return r


def git_json(path, rev):
    out = subprocess.run(["git", "show", f"{rev}:afterlight/{path}"], cwd=ROOT, capture_output=True, text=True)
    return json.loads(out.stdout) if out.returncode == 0 else None


def mod(t, at, r="North", extras=None):
    return {"module_type": t, "grid_pos": [at[0], at[1]], "rotation": r, "custom_name": None, "subcomponents": None, "extras": extras}


# ---------------------------------------------------------------- kits ---

def load_kit(name):
    for f in sorted(os.listdir(os.path.join(ROOT, "designs/kits"))):
        d = json.load(open(os.path.join(ROOT, "designs/kits", f)))
        if d["name"] == name:
            return d
    raise SystemExit(f"no kit {name}")


def with_gun(kit_design, gun_file):
    """A turret kit carrying another saved gun design on its turret."""
    if gun_file is None:
        return kit_design
    g = json.load(open(os.path.join(ROOT, "designs/guns", gun_file)))
    hull = [p for p in kit_design["parts"] if not p.get("turret")]
    turret = [dict(p, turret=True) for p in g["parts"]]
    return dict(kit_design, parts=hull + turret, shell=g.get("shell"), point_defence=g.get("point_defence", kit_design.get("point_defence", False)))


def laser_kit(rows, flavour):
    T = {"C": "LaserCore", "V": "LaserCavity", "P": "LaserPump", "E": "LaserEmitter",
         "1": flavour[0] if flavour else None, "2": flavour[1] if flavour else None}
    cy = next(i for i, r in enumerate(rows) if "C" in r)
    cx = rows[cy].index("C")
    parts = [{"module_type": T[ch], "offset": [x - cx, cy - y], "rotation": "East"}
             for y, r in enumerate(rows) for x, ch in enumerate(r) if ch in T and T[ch]]
    return {"name": "laser", "parts": parts}


RING_SIZE = {"TurretRingSmall": 1, "TurretRing": 3, "TurretRingLarge": 5}


def deck(size):
    r = size / 2 + 1.05
    span = int(-(-r // 1))
    return {(x, y) for x in range(-span, span + 1) for y in range(-span, span + 1) if (x * x + y * y) ** 0.5 <= r}


def install(design, origin, facing):
    """Every part laid down (GunDesign::install): (type, cell, hull_cell,
    rotation, ring_at or None, part)."""
    k = K_OF[facing]
    placed = []
    for p in design["parts"]:
        dx, dy = turn(p["offset"], k)
        placed.append((p, (origin[0] + dx, origin[1] + dy), turn_rot(p["rotation"], k)))
    def centre(t, at, r):
        cells = kit.cells_of(t, at, r)
        return (sum(c[0] for c in cells) // len(cells), sum(c[1] for c in cells) // len(cells))
    ring = None
    for p, at, r in placed:
        if not p.get("turret") and p["module_type"] in RING_SIZE:
            drawn = centre(p["module_type"], tuple(p["offset"]), p["rotation"])
            ring = (at, centre(p["module_type"], at, r), RING_SIZE[p["module_type"]], drawn)
    out = []
    for p, at, r in placed:
        if p.get("turret"):
            # Turning the kit turns its hull parts only: what stands on the
            # turret keeps the way it was drawn, its gun along the turret's
            # +x, which the turret swings onto the target.
            cell = (p["offset"][0] - ring[3][0], p["offset"][1] - ring[3][1])
            out.append((p["module_type"], cell, (ring[1][0] + cell[0], ring[1][1] + cell[1]), p["rotation"], ring, p))
        else:
            out.append((p["module_type"], at, at, r, None, p))
    return out, ring


# ---------------------------------------------------------------- ship ---

class Scaled:
    def __init__(self, design, faction, k=2):
        self.src = design
        self.faction = faction
        self.k = k
        self.hull = {}          # cell -> (layer, material)
        self.modules = []
        self.taken = {}         # cell -> module index (hull deck only)
        self.slots = {}         # cell -> (kind, facing)
        self.turret_decks = set()
        self.report = Counter()
        self.ring_label = {}    # ring cell -> kit name
        self.anchors = []       # (kit key, doubled centre, rotation): the Vigil's installs
        self.exotic = False
        k = self.k
        # Hull guns fitted by name (the Vigil's spinal Throne Lance): (gun
        # design name, doubled firing piece cell, facing).
        self.spinals = [(g["design"], (k * g["at"][0] + k // 2, k * g["at"][1]), g.get("facing", "East"))
                        for g in design.get("guns", []) if not g.get("turret")]

    def put(self, t, at, r="North", extras=None):
        cells = kit.cells_of(t, at, r)
        assert not any(c in self.taken for c in cells), (t, at, [c for c in cells if c in self.taken])
        self.modules.append(mod(t, at, r, extras))
        for c in cells:
            self.taken[c] = len(self.modules) - 1
            self.slots.pop(c, None)

    def scale(self):
        src_hull = {tuple(h["grid_pos"]): (h["layer"], h["material"]) for h in self.src["hull_cells"]}
        k = self.k
        big = {}
        for (x, y), (layer, mat) in src_hull.items():
            for u in range(k):
                for v in range(k):
                    big[(k * x + u, k * y + v)] = (layer, mat)
        # The skin stays one cell thick: what was outer hull is outer only
        # where the doubled hull meets space.
        for c, (layer, mat) in big.items():
            edge = any((c[0] + dx, c[1] + dy) not in big for dx in (-1, 0, 1) for dy in (-1, 0, 1))
            self.hull[c] = ("Outer" if edge and layer == "Outer" else ("Inner" if layer == "Outer" else layer), mat)

        for m in self.src["modules"]:
            t, r = m["module_type"], m.get("rotation", "North")
            extras = dict(m.get("extras") or {})
            if extras.get("turret"):
                continue  # the Vigil's turret parts: its rings are slots now
            cells = kit.cells_of(t, tuple(m["grid_pos"]), r)
            self.exotic |= t in EXOTIC
            if self.faction == "eternal_hegemony" and t in NEW_SYSTEMS:
                # A built install (the Vigil): its cells are room for one kit
                # of the same kind, centred where it stood.
                if t in ANCHOR_KIT:
                    cx = sum(c[0] for c in cells) / len(cells)
                    cy = sum(c[1] for c in cells) / len(cells)
                    self.anchors.append((ANCHOR_KIT[t], (round(k * cx + (k - 1) / 2), round(k * cy + (k - 1) / 2)), r))
                for (x, y) in cells:
                    for u in range(k):
                        for v in range(k):
                            self.slots[(k * x + u, k * y + v)] = ("support", r)
                continue
            kind = SWAP.get(t)
            if kind:
                for (x, y) in cells:
                    for u in range(k):
                        for v in range(k):
                            self.slots[(k * x + u, k * y + v)] = (kind, r)
                continue
            if t == "AngledArmorPlate":
                # Cells across the diagonal from the open corner are angled,
                # the open side stays open, the rest is full plate: the
                # angled face stays one straight line.
                (x, y), = cells
                ox, oy = OPEN_CORNER[r]
                for u in range(k):
                    for v in range(k):
                        a = u if ox > 0 else k - 1 - u
                        b = v if oy > 0 else k - 1 - v
                        c = (k * x + u, k * y + v)
                        if a + b < k - 1:
                            self.put("ArmorPlate", c)
                        elif a + b == k - 1:
                            self.put("AngledArmorPlate", c, r)
                continue
            # Four copies tiling the doubled footprint.
            w = max(cx for cx, _ in cells) - min(cx for cx, _ in cells) + 1
            h = max(cy for _, cy in cells) - min(cy for _, cy in cells) + 1
            lo = (k * min(cx for cx, _ in cells), k * min(cy for _, cy in cells))
            f0 = kit.cells_of(t, (0, 0), r)
            flo = (min(c[0] for c in f0), min(c[1] for c in f0))
            # A station keeps one of each exotic weapon, as it always has been
            # built, so its guns don't multiply ninefold.
            once_here = self.k > 2 and t in EXOTIC
            for i in range(k):
                for j in range(k):
                    at = (lo[0] + i * w - flo[0], lo[1] + j * h - flo[1])
                    first = (i, j) == (k // 2, k // 2) if once_here else (i, j) == (0, 0)
                    if t in ONCE and not first:
                        self.put(ONCE[t], at, "North")
                    elif once_here and not first:
                        continue
                    else:
                        self.put(t, at, r, extras or None)

    # ------------------------------------------------------- weapons ---

    def regions(self):
        seen, out = set(), []
        for c in sorted(self.slots):
            if c in seen:
                continue
            kinds = Counter()
            facings = Counter()
            stack, cells = [c], []
            seen.add(c)
            while stack:
                p = stack.pop()
                cells.append(p)
                kinds[self.slots[p][0]] += 1
                facings[self.slots[p][1]] += 1
                for d in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    n = (p[0] + d[0], p[1] + d[1])
                    if n in self.slots and n not in seen and self.slots[n][0] == self.slots[c][0]:
                        seen.add(n)
                        stack.append(n)
            out.append((kinds.most_common(1)[0][0], facings.most_common(1)[0][0], set(cells)))
        return out

    def try_kit(self, design, free, facings, label, extras_for=None, order=None, room=None):
        """Place `design` at the first spot in `free` it fits (in `order` if
        given), trying each facing; True if it went down. Its blocks may
        also use `room` (free deck beside the slot)."""
        room = free if room is None else room
        for c in (order if order is not None else sorted(free)):
            for facing in facings:
                parts, ring = install(design, c, facing)
                hull_cells = []
                ok = True
                for t, cell, _, r, rg, _ in parts:
                    if rg is None:
                        cs = kit.cells_of(t, cell, r)
                        if not all((x in free or x in room) and x not in self.taken for x in cs):
                            ok = False
                            break
                        hull_cells += cs
                if not ok or len(set(hull_cells)) != len(hull_cells):
                    continue
                if ring:
                    d = {(ring[1][0] + x, ring[1][1] + y) for x, y in deck(ring[2])}
                    if d & self.turret_decks:
                        continue
                    self.turret_decks |= d
                for t, cell, _, r, rg, p in parts:
                    extras = {}
                    if rg is not None:
                        extras["turret"] = list(rg[0])
                    if t == "FiringPiece" and design.get("shell"):
                        extras["shell"] = design["shell"]
                    if design.get("point_defence") and t in ("FiringPiece", "LaserEmitter"):
                        extras["point_defence"] = True
                    if extras_for:
                        extras.update(extras_for(t))
                    if rg is None:
                        self.put(t, cell, r, extras or None)
                    else:
                        self.modules.append(mod(t, cell, r, extras))
                for x in hull_cells:
                    free.discard(x)
                if ring:
                    self.ring_label[tuple(ring[0])] = label
                self.report[label] += 1
                return True
        return False

    def arm(self, kits):
        missile = MISSILES.get(self.faction)
        flavour = LASER_FLAVOUR.get(self.faction)
        # A full bank (emitter, core, two modifiers, three cavities, three
        # pumps) where the faction flies one, else a 2x2 unit per old laser.
        full_laser = laser_kit(["PV1.", "PVCE", "PV2."], flavour) if flavour else None
        mini_laser = laser_kit(["VP", "CE"], None)
        # The Compact 170 as roomy as the slot allows: three motors, then
        # two in a 2x3 block, then one in its 2x2 (slow, but it fits where a
        # single old cannon stood on the hull's edge).
        guns_on_top = [p for p in kits["compact"]["parts"] if p.get("turret")]
        ring_at = {"module_type": "TurretRingSmall", "offset": [0, 0], "rotation": "North"}
        def hull_kit(cells):
            return [ring_at] + [{"module_type": t, "offset": list(c), "rotation": "North"} for t, c in cells]
        compact2 = dict(kits["compact"], parts=hull_kit([("TraverseMotor", (1, 0)), ("TraverseMotor", (1, 1)), ("AmmoHoist", (0, -1)), ("Magazine", (1, -1))]) + guns_on_top)
        compact1 = dict(kits["compact"], parts=hull_kit([("TraverseMotor", (1, 0)), ("AmmoHoist", (0, -1)), ("Magazine", (1, -1))]) + guns_on_top)
        turrets = {"gun": [kits["autocannon"]], "cannon": [kits["twin"], kits["turret"], kits["compact"], compact2, compact1],
                   "pd": [kits["ciws"]]}
        all4 = ["East", "South", "West", "North"]
        # The Vigil's installs first: each where it stood, from its own cells.
        support = {c for c, (k, _) in self.slots.items() if k == "support"}
        for key, at, r in self.anchors:
            order = sorted(support, key=lambda c: (c[0] - at[0]) ** 2 + (c[1] - at[1]) ** 2)
            facings = [r if r != "North" else "East"] if key == "spinal" else all4
            extras = (lambda t: {"missile": missile} if t == "VlsSilo" else {}) if key == "vls" and missile else None
            if not self.try_kit(kits[key], support, facings, kits[key]["name"], extras, order=order[:60]):
                self.report["(no room: " + kits[key]["name"] + ")"] += 1
        for name, at, facing in self.spinals:
            g = next(json.load(open(os.path.join(ROOT, "designs/guns", f))) for f in sorted(os.listdir(os.path.join(ROOT, "designs/guns")))
                     if json.load(open(os.path.join(ROOT, "designs/guns", f)))["name"] == name)
            feeder = next((p["offset"] for p in g["parts"] if p["module_type"] == "ShellFeeder"), None)
            mags = [{"module_type": "Magazine", "offset": [feeder[0] - 1, feeder[1] + dy], "rotation": "North"} for dy in (-1, 0, 1)] if feeder else []
            spinal = dict(g, parts=g["parts"] + mags)
            room = {c for c in ((at[0] + x, at[1] + y) for x in range(-14, 15) for y in range(-8, 9))
                    if c not in self.taken and self.hull.get(c, ("",))[0] not in WALKED}
            order = sorted(room, key=lambda c: (c[0] - at[0]) ** 2 + (c[1] - at[1]) ** 2)[:80]
            if not self.try_kit(spinal, room, [facing], name, order=order):
                self.report["(no room: " + name + ")"] += 1
        # Interior deck a turret's motors may spill onto (never a corridor).
        deck = {c for c, (layer, _) in self.hull.items() if layer not in WALKED}
        for kind, facing, cells in self.regions():
            free = {c for c in cells if c not in self.taken}
            if kind in turrets:
                room = free | deck
                for design in turrets[kind]:
                    big = design is kits["twin"]
                    if big and len(cells) < 60:
                        continue
                    while self.try_kit(design, free, all4, design["name"], order=sorted(free), room=room):
                        pass
                # Room left over takes the next size down.
                if kind == "cannon":
                    while self.try_kit(kits["autocannon"], free, all4, kits["autocannon"]["name"], order=sorted(free), room=room):
                        pass
            elif kind == "missile":
                extras = (lambda t: {"missile": missile} if t == "VlsSilo" else {}) if missile else None
                while self.try_kit(kits["vls"], free, all4, "VLS Missile Silo", extras):
                    pass
            elif kind == "laser":
                for design in ([full_laser] if full_laser else []) + [mini_laser]:
                    while self.try_kit(design, free, [facing], "laser unit"):
                        pass
            # What's left of a weapon slot outside the hull is plated, so the
            # silhouette stays whole; inside it is deck again.
            for c in free:
                if c not in self.taken and c not in self.hull:
                    self.put("ArmorPlate", c)
        self.slots.clear()

    def point_defence(self, kits, want):
        """At least `want` CIWS: autocannons become CIWS first, spread from
        end to end; then new ones go on free deck (never a corridor) out
        toward bow, stern and both sides."""
        have = self.report["CIWS Turret"]
        rings = sorted((r for r, label in self.ring_label.items() if label == "Light Autocannon"), key=lambda c: (c[0], c[1]))
        ciws = kits["ciws"]
        while have < want and rings:
            ring_at = list(rings.pop(0 if have % 2 == 0 else -1))
            self.modules = [m for m in self.modules if (m.get("extras") or {}).get("turret") != ring_at]
            for p in ciws["parts"]:
                if p.get("turret"):
                    extras = {"turret": ring_at}
                    if p["module_type"] == "FiringPiece":
                        extras.update({"shell": ciws.get("shell"), "point_defence": True})
                    self.modules.append(mod(p["module_type"], tuple(p["offset"]), p["rotation"], extras))
            self.ring_label[tuple(ring_at)] = "CIWS Turret"
            self.report["CIWS Turret"] += 1
            self.report["Light Autocannon"] -= 1
            have += 1
        self.retake()
        dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)]
        tries = 0
        while have < want and tries < 8:
            dx, dy = dirs[tries % 4]
            tries += 1
            deckfree = {c for c, (layer, _) in self.hull.items() if layer not in WALKED and c not in self.taken}
            order = sorted(deckfree, key=lambda c: -(c[0] * dx + c[1] * dy))[:300]
            if self.try_kit(ciws, deckfree, ["East", "South", "West", "North"], "CIWS Turret", order=order):
                have += 1

    def retake(self):
        """What's taken on the hull deck, from the modules as they stand."""
        self.taken = {}
        for i, m in enumerate(self.modules):
            if not (m.get("extras") or {}).get("turret"):
                for c in kit.cells_of(m["module_type"], tuple(m["grid_pos"]), m["rotation"]):
                    self.taken[c] = i

    def pd_laser_sponson(self, design):
        """A PD laser turret as near the middle as it fits: on free deck, or
        hanging off the flank as a sponson -- the Halyard Mk II's way -- with
        hull laid under whatever stands past the old edge. Never over a
        corridor or a door."""
        deck = {c for c, (layer, _) in self.hull.items() if layer not in WALKED and c not in self.taken}
        near = {(x + dx, y + dy) for (x, y) in self.hull for dx in range(-6, 7) for dy in range(-6, 7)}
        off = {c for c in near if c not in self.hull and c not in self.taken}
        room = deck | off
        mid = (sum(c[0] for c in self.hull) / len(self.hull), sum(c[1] for c in self.hull) / len(self.hull))
        # Inside the hull first, then the flank nearest the middle.
        order = sorted(room, key=lambda c: (c in off, (c[0] - mid[0]) ** 2 + (c[1] - mid[1]) ** 2))
        before = set(self.taken)
        if not self.try_kit(design, room, ["North", "South", "East", "West"], "PD Laser Turret", order=order):
            self.report["(no room: PD Laser Turret)"] += 1
            return
        for c in set(self.taken) - before:
            if c not in self.hull:
                self.hull[c] = ("Inner", "Steel")
        # The skin is wherever the hull now meets space.
        for c, (layer, mat) in list(self.hull.items()):
            if layer in WALKED:
                continue
            edge = any((c[0] + dx, c[1] + dy) not in self.hull for dx in (-1, 0, 1) for dy in (-1, 0, 1))
            self.hull[c] = ("Outer" if edge else ("Inner" if layer == "Outer" else layer), mat)

    def warners(self, count):
        """Missile Warners out toward bow, stern, port and starboard, on
        free deck (never a corridor); a decoy and chaff beside the first."""
        spots = [c for c, (layer, _) in self.hull.items() if layer not in WALKED and c not in self.taken]
        if not spots:
            return
        dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)]
        for i in range(count):
            dx, dy = dirs[i % 4]
            spots = [c for c in spots if c not in self.taken]
            if not spots:
                break
            c = max(spots, key=lambda s: s[0] * dx + s[1] * dy)
            self.put("MissileWarner", c)
            self.report["Missile Warner"] += 1
            if i == 0:
                for t, n in (("DecoyLauncher", (c[0] - dx, c[1] - dy)), ("ChaffDispenser", (c[0] - 2 * dx, c[1] - 2 * dy))):
                    if n in self.hull and n not in self.taken and self.hull[n][0] not in WALKED:
                        self.put(t, n)
                        self.report[t] += 1

    def design(self):
        d = {k: v for k, v in self.src.items() if k not in ("guns", "hull_cells", "modules")}
        d["hull_cells"] = [{"grid_pos": [c[0], c[1]], "layer": l, "material": m} for c, (l, m) in sorted(self.hull.items())]
        d["modules"] = self.modules
        d["created_at"] = "tools/ship-scale/scale.py (doubled from the drawn ship, armed with designs/kits)"
        return d


def main():
    dry = "--dry" in sys.argv
    kits = {
        "autocannon": load_kit("Light Autocannon"),
        "ciws": load_kit("CIWS Turret"),
        "compact": load_kit("Compact 170 mm Turret"),
        "turret": load_kit("170 mm Turret"),
        "twin": load_kit("Twin 225 mm Turret"),
        "vls": load_kit("VLS Missile Silo"),
        "spinal": load_kit("280 mm Spinal Gun"),
    }
    jobs = []
    fdir = os.path.join(ROOT, "designs/factions")
    for entry in sorted(os.listdir(fdir)):
        # Each faction's own fallback ship (designs/factions/<faction>.json,
        # flown when a spawn has no class), then its classes.
        if entry.endswith(".json"):
            rels = [(f"designs/factions/{entry}", entry[:-5])]
        else:
            rels = [(f"designs/factions/{entry}/{f}", entry) for f in sorted(os.listdir(os.path.join(fdir, entry)))]
        for rel, faction in rels:
            src = git_json(rel, SOURCE_REV) or git_json(rel, LATER_REV)
            jobs.append((rel, faction, src))
    for name in PLAYER:
        rel = f"designs/{name}.json"
        jobs.append((rel, "player", git_json(rel, SOURCE_REV) or git_json(rel, LATER_REV)))

    for rel, faction, src in jobs:
        own = dict(kits)
        guns = FACTION_GUNS.get(faction, {})
        if "ciws" in guns:
            own["ciws"] = with_gun(kits["ciws"], guns["ciws"])
        if "turret" in guns:
            own["turret"] = with_gun(kits["turret"], guns["turret"])
        station = (faction, os.path.basename(rel)[:-5]) in STATIONS
        s = Scaled(src, faction, 3 if station else 2)
        s.scale()
        armed = bool(s.slots) or s.exotic
        s.arm(own)
        cells = len(src["hull_cells"])
        # The player's ships also carry a PD laser turret (the Halyard Mk
        # II's unit): a CIWS or two downed 1 of a Choir swarm's 28 missiles,
        # the Halyard's lasers 63 of 72 (combat lab).
        if faction == "player":
            s.pd_laser_sponson(load_kit("PD Laser Turret"))
        if armed and faction != "the_silence":
            # About one CIWS per 400 cells of the doubled hull, 1 to 8: one
            # turret downed 2 of a Choir swarm's 35 missiles (combat lab).
            s.point_defence(own, 8 if station else max(1, min(8, round(len(s.hull) / 400))))
            s.warners(2 if cells < 300 else 4)
        out = s.design()
        line = ", ".join(f"{n} {k}" for k, n in sorted(s.report.items()) if n)
        print(f"{rel}: {cells} -> {len(out['hull_cells'])} cells, {len(out['modules'])} blocks | {line or 'unarmed'}")
        if not dry:
            json.dump(out, open(os.path.join(ROOT, rel), "w"), indent=1)


if __name__ == "__main__":
    main()
