"""Faction liveries for the reworked fleet: more colour, laid by structure, and
small details on the shell.

The game's paint (vfx/ship_roof.rs ShipPaint) gives every ship three colours
(main, second, trim), any colour per tile (Slot::Custom) and decals of twelve
shapes in any colour on three layers. This lays all of that from a faction's
palette over a refit design:

  - the hull in the faction's main colour, its livery pattern in the second;
  - rooms tinted by what they are (guns, engines, power, the bridge, sensors)
    from the same palette, so colour follows the ship's structure;
  - the faction's motif in decals (Terran navy stripes and a roundel, the
    Choir's gold kintsugi seams, the Compact's hazard hatching...);
  - details: panel seams across big bare plating, running lights round the
    hull's edge, hatching ahead of the engines.

Everything is laid mirror-wise port and starboard, like paint_gen.rs, whose
decal conventions this follows (Stripe turn 0 runs bow to stern, Edge turn 1
is a cell's bow side, Chevron turn 1 points at the bow, four Arcs round a
corner make a roundel).
"""
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(ROOT, "tools", "fleet-concepts"))
import fleet  # noqa: E402  (module footprints, from the game's registry)


def _categories():
    """ModuleType -> ModuleCategory, read from components.rs's category()."""
    text = open(os.path.join(ROOT, "src", "components.rs")).read()
    body = text[text.index("pub fn category(&self) -> ModuleCategory"):]
    body = body[:body.index("\n    }\n")]
    out = {}
    for arm in re.finditer(r"((?:ModuleType::\w+\s*\|?\s*)+)=>\s*ModuleCategory::(\w+)", body):
        for t in re.findall(r"ModuleType::(\w+)", arm.group(1)):
            out[t] = arm.group(2)
    return out


CATEGORY = _categories()


def mix(a, b, t):
    return [round(a[i] + (b[i] - a[i]) * t) for i in range(3)]


def shade(a, k):
    return [max(0, min(255, round(c * k))) for c in a]


# A faction's paint: main hull, second (its livery pattern and panels), three
# accents (the first is the trim), a light colour for running lights, its
# livery pattern, its motif, and how strongly rooms take their tint.
THEMES = {
    # The navy: light grey plate, navy panels, red and blue stripes down the
    # spine, a yellow roundel (the developer asked for red, blue and yellow).
    "terran_hegemony": dict(main=[178, 188, 200], second=[38, 56, 104], accents=[[206, 44, 48], [44, 104, 206], [238, 192, 52]],
                            light=[250, 244, 220], pattern="Bands", motif="navy", room=0.6),
    # Crimson and gold, an empire dressed for parade: gold chevrons down the
    # nave, ivory bands.
    "eternal_hegemony": dict(main=[156, 38, 46], second=[226, 172, 64], accents=[[236, 188, 74], [238, 228, 206], [116, 64, 156]],
                             light=[255, 214, 120], pattern="Chevrons", motif="regal", room=0.62),
    # Royal slate and gold, jewel green: a gilded nose and a roundel.
    "gilded_throne": dict(main=[96, 104, 156], second=[42, 44, 76], accents=[[242, 200, 82], [240, 232, 214], [44, 176, 124]],
                          light=[255, 232, 160], pattern="Bow", motif="gilt", room=0.6),
    # Guild green and cream with burgundy and copper bands.
    "merchant_princes": dict(main=[124, 168, 108], second=[54, 76, 50], accents=[[238, 226, 186], [138, 38, 52], [200, 116, 58]],
                             light=[255, 236, 190], pattern="Bands", motif="guild", room=0.6),
    # Rust orange work paint, charcoal, hazard yellow and black on the push
    # bars, a teal patch here and there.
    "free_haulers_compact": dict(main=[196, 116, 72], second=[238, 192, 46], accents=[[242, 198, 40], [26, 26, 30], [60, 166, 164]],
                                 light=[255, 220, 120], pattern="Stripe", motif="hazard", room=0.6),
    # Violet flesh, teal veins, pink and lime spots.
    "synthesis_collective": dict(main=[152, 86, 194], second=[58, 30, 76], accents=[[72, 222, 200], [236, 128, 150], [176, 232, 84]],
                                 light=[150, 255, 230], pattern="Stripe", motif="spots", room=0.62),
    # Pale leaf-white with teal flanks, leaf green, sky blue and amber.
    "stellar_preserve": dict(main=[214, 232, 222], second=[40, 146, 128], accents=[[98, 184, 82], [82, 172, 222], [232, 172, 62]],
                             light=[230, 255, 210], pattern="Split", motif="flanks", room=0.6),
    # Pale jade, broken and mended in gold (kintsugi), a bruise violet.
    "broken_choir": dict(main=[164, 200, 174], second=[52, 70, 60], accents=[[230, 184, 78], [238, 236, 222], [96, 64, 146]],
                         light=[255, 226, 150], pattern="Checker", motif="kintsugi", room=0.62),
    # Bone, with violet ribs and the ember of a dying star.
    "corpse_stars": dict(main=[216, 206, 184], second=[66, 44, 104], accents=[[156, 100, 232], [204, 74, 44], [118, 136, 100]],
                         light=[200, 150, 255], pattern="Chevrons", motif="ribs", room=0.62),
    # White and charcoal, one pale gold pinstripe: restraint.
    "the_silence": dict(main=[232, 234, 238], second=[56, 58, 64], accents=[[214, 194, 140], [40, 42, 46], [126, 146, 176]],
                        light=[230, 236, 255], pattern="Bands", motif="quiet", room=0.3),
    # Wheat gold, crook brown, wool cream and a red mark.
    "shepherd": dict(main=[218, 184, 82], second=[74, 50, 30], accents=[[132, 86, 44], [242, 234, 212], [176, 42, 36]],
                     light=[255, 240, 190], pattern="Bow", motif="crook", room=0.6),
    # Orange and charcoal chevrons; the copies' generations in teal, cream
    # and red.
    "recursive_kingdom": dict(main=[232, 142, 62], second=[48, 44, 42], accents=[[62, 172, 182], [238, 222, 190], [204, 62, 42]],
                              light=[255, 220, 160], pattern="Chevrons", motif="generations", room=0.6),
}

# Which palette colour a room takes, by what it is.
ROOM_TINT = {
    "Weapons": 0, "Guns": 0, "Lasers": 0,      # first accent
    "Propulsion": 1,                           # second accent
    "Power": 2, "Detection": 2,                # third accent
    "Control": "second", "Utility": "second",  # panel colour
}
SKIP = {"Structural"}  # armour and frames keep the hull's paint


def accent_at(pattern, cell, lo, hi):
    """Livery::accent_at."""
    x, y = cell
    from_bow = hi[0] - x
    off = y - (lo[1] + hi[1]) * 0.5
    if pattern == "Bands":
        return x % 6 < 2
    if pattern == "Chevrons":
        return (x + int(abs(off))) % 7 < 2
    if pattern == "Bow":
        return from_bow < max((hi[0] - lo[0] + 1) // 5, 2)
    if pattern == "Split":
        return off > 0
    if pattern == "Checker":
        return (x // 2 + y // 2) % 2 == 0
    if pattern == "Stripe":
        return abs(off) < 1.5
    return False


# ---------------------------------------------------------------------------
# Schemes: every faction keeps its main colour, but its ships don't all wear
# the same livery. Each faction has several schemes, dealt out across its
# ships in turn, and each ship's own seed nudges where things sit, so two
# ships on one scheme still differ. A scheme is a list of (motif, args);
# colours are named: a0 a1 a2 (the accents), second, light, dark (the main
# shaded), pale (the main lightened). A (lo, hi) pair is picked per ship.
# "pattern" sets which Livery pattern the hull's second colour follows.
# ---------------------------------------------------------------------------
SCHEMES = {
    "terran_hegemony": [
        # Line navy: red and blue spine stripes, a yellow roundel.
        [("pattern", "Bands"), ("spine_stripes", dict(colours=["a0", "a1"])), ("roundel", dict(colour="a2", at=(0.25, 0.4))),
         ("spine_chevrons", dict(colour="a2", step=4)), ("contour", dict(colour="second", depths=[1]))],
        # Carrier deck: a navy stern, deck rungs, red wingtips.
        [("pattern", "Stripe"), ("zone", dict(colour="second", start=(0.62, 0.72), stop=1.0)), ("ladder", dict(colour="a2", step=3, w=2)),
         ("tips", dict(colour="a0", depth=1)), ("roundel", dict(colour="a1", at=(0.3, 0.45))), ("contour", dict(colour="a0", depths=[2]))],
        # Strike sweep: swept red and blue bands, a navy edge.
        [("pattern", "Bow"), ("sweep", dict(colours=["a0", "a1"], at=(0.18, 0.3), w=2)), ("contour", dict(colour="second", depths=[0])),
         ("ticks", dict(colour="a2")), ("spine_stripes", dict(colours=["a2"], start=0.45, stop=0.85, dashed=True))],
        # Yellow and blue: a blue nose, yellow stripes, yellow tips.
        [("pattern", "Bands"), ("nose", dict(colour="a1", frac=(0.12, 0.18))), ("spine_stripes", dict(colours=["a2", "a1"], start=0.2)),
         ("tips", dict(colour="a2", depth=1)), ("roundel", dict(colour="a2", at=(0.5, 0.6))), ("contour", dict(colour="a1", depths=[2]))],
        # Panelled: navy flanks, red pinstripes, blue bands.
        [("pattern", "Bands"), ("flanks", dict(colour="second", depth=1)), ("contour", dict(colour="a0", depths=[2])),
         ("bands", dict(colour="a1", line="a2", n=2, w=1)), ("roundel", dict(colour="a2", at=(0.3, 0.4)))],
    ],
    "eternal_hegemony": [
        # Parade: gold chevrons, ivory bands, gold edging.
        [("pattern", "Chevrons"), ("spine_chevrons", dict(colour="a0", step=3)), ("bands", dict(colour="a1", line="a0", n=2, w=1)),
         ("contour", dict(colour="a0", depths=[0]))],
        # Sunburst: gold rays round a violet heart, a gilded stern.
        [("pattern", "Bow"), ("sunburst", dict(colour="a0", at=(0.35, 0.5), rays=8)), ("disc", dict(colour="a2", at=None, ring="a0")),
         ("stern", dict(colour="a0", frac=(0.06, 0.1))), ("contour", dict(colour="a0", depths=[1]))],
        # Vestments: a violet stole down the spine, gold borders, a gold nose.
        [("pattern", "Bands"), ("belly", dict(colour="a2", frac=(0.22, 0.32), border="a0")), ("nose", dict(colour="a0", frac=(0.1, 0.15))),
         ("contour", dict(colour="a1", depths=[1]))],
        # Heraldic: swept gold bars, ivory rungs, violet tips.
        [("pattern", "Chevrons"), ("sweep", dict(colours=["a0", "a0", "a0"], at=(0.15, 0.25), w=1)), ("ladder", dict(colour="a1", step=4, w=1)),
         ("tips", dict(colour="a2", depth=1)), ("contour", dict(colour="a0", depths=[0]))],
        # Imperial: a gold bow, ivory rings amidships, gold pinstripes.
        [("pattern", "Bands"), ("zone", dict(colour="second", start=0.0, stop=(0.14, 0.2))), ("rings", dict(colour="a1", at=(0.45, 0.6), n=3)),
         ("contour", dict(colour="a0", depths=[2])), ("spine_chevrons", dict(colour="a0", step=5))],
    ],
    "gilded_throne": [
        [("pattern", "Bow"), ("nose", dict(colour="a0")), ("roundel", dict(colour="a2", at=0.45)), ("spine_stripes", dict(colours=["a1"], start=0.25)),
         ("contour", dict(colour="a0", depths=[0]))],
        # Filigree: double gold pinstripes, an emerald jewel, ivory ticks.
        [("pattern", "Bands"), ("contour", dict(colour="a0", depths=[1, 3])), ("disc", dict(colour="a2", at=(0.4, 0.55), ring="a0")),
         ("ticks", dict(colour="a1"))],
        # Banknote: ivory bars ruled in gold, a dark spine.
        [("pattern", "Bands"), ("belly", dict(colour="second", frac=(0.16, 0.24), border="a0")), ("bands", dict(colour="a1", line="a0", n=4, w=1)),
         ("roundel", dict(colour="a2", at=(0.2, 0.3)))],
        # Crown: a gold sweep, gold tips, emerald chevrons.
        [("pattern", "Bow"), ("sweep", dict(colours=["a0"], at=(0.15, 0.25), w=3)), ("tips", dict(colour="a0", depth=1)),
         ("spine_chevrons", dict(colour="a2", step=3)), ("contour", dict(colour="a1", depths=[2]))],
        # Inlay: a checkered gold stern, gold border, an emerald disc.
        [("pattern", "Bow"), ("stern", dict(colour="a0", frac=(0.12, 0.18), checker=True)), ("contour", dict(colour="a0", depths=[0, 2])),
         ("disc", dict(colour="a2", at=(0.3, 0.45), ring="a1"))],
    ],
    "merchant_princes": [
        [("pattern", "Bands"), ("bands", dict(colour="a0", line="a1", n=3, w=2)), ("spine_stripes", dict(colours=["a2"], start=0.2, stop=0.8))],
        # House colours: a cream spine band, burgundy borders, a copper stern.
        [("pattern", "Stripe"), ("belly", dict(colour="a0", frac=(0.2, 0.3), border="a1")), ("stern", dict(colour="a2", frac=(0.08, 0.12))),
         ("contour", dict(colour="a1", depths=[0]))],
        # Awning: burgundy and cream stripes amidships.
        [("pattern", "Bow"), ("awning", dict(colours=["a1", "a0"], start=(0.3, 0.4), stop=(0.65, 0.75))), ("nose", dict(colour="a2", frac=0.1)),
         ("contour", dict(colour="a0", depths=[1]))],
        # Ledger: copper rungs, a cream nose, burgundy tips.
        [("pattern", "Bands"), ("ladder", dict(colour="a2", step=2, w=2)), ("nose", dict(colour="a0", frac=(0.12, 0.18))),
         ("tips", dict(colour="a1", depth=1)), ("roundel", dict(colour="a1", at=(0.4, 0.55)))],
        # Seal: burgundy flanks, cream bars, a copper roundel.
        [("pattern", "Bow"), ("flanks", dict(colour="a1", depth=1)), ("bands", dict(colour="a0", line="a2", n=2, w=1)),
         ("disc", dict(colour="a2", at=(0.3, 0.45), ring="a0"))],
    ],
    "free_haulers_compact": [
        [("pattern", "Stripe"), ("hazard", dict(yellow="a0", black="a1")), ("flanks", dict(colour="a2", depth=1))],
        # Work lights: hazard tips, yellow pinstripe, black ticks, a teal stern.
        [("pattern", "Bands"), ("tips", dict(colour="a0", depth=1, hatch="a1")), ("contour", dict(colour="a0", depths=[2])),
         ("ticks", dict(colour="a1")), ("stern", dict(colour="a2", frac=(0.08, 0.14)))],
        # Patchwork: teal and grey patches welded on over the years.
        [("pattern", "Stripe"), ("patches", dict(colours=["a2", "dark", "pale"], every=4)), ("hazard", dict(yellow="a0", black="a1", frac=0.07)),
         ("contour", dict(colour="a1", depths=[0]))],
        # Striped barge: yellow bars with black edges, teal flanks.
        [("pattern", "Bands"), ("bands", dict(colour="a0", line="a1", n=3, w=1)), ("flanks", dict(colour="a2", depth=1)),
         ("stern", dict(colour="a1", frac=0.06, checker=True))],
        # Union: a charcoal belly, yellow borders, hazard bow.
        [("pattern", "Bow"), ("belly", dict(colour="a1", frac=(0.18, 0.26), border="a0")), ("hazard", dict(yellow="a0", black="a1", frac=0.06)),
         ("tips", dict(colour="a2", depth=1))],
    ],
    "synthesis_collective": [
        [("pattern", "Stripe"), ("flanks", dict(colour="a0", depth=1)), ("spots", dict(colours=["a1", "a2"]))],
        # Veins: teal veins curving out from the spine, pink spots.
        [("pattern", "Bow"), ("ribs", dict(colour="a0", every=(5, 7), curve=1.4)), ("spots", dict(colours=["a1"], every=11)),
         ("contour", dict(colour="a2", depths=[0]))],
        # Membrane: the hull grown in cells, teal walls.
        [("pattern", "Stripe"), ("membrane", dict(line="a0", fills=["second", None, "a1", None, None, None], size=(5, 7))),
         ("contour", dict(colour="a2", depths=[1]))],
        # Bioluminescent: lime rings round the core, teal flanks.
        [("pattern", "Bands"), ("rings", dict(colour="a2", at=(0.4, 0.55), n=4)), ("flanks", dict(colour="a0", depth=1)),
         ("spots", dict(colours=["a1"], every=13))],
        # Mottled: flesh blotches in pink and dark, lime veins.
        [("pattern", "Bands"), ("patches", dict(colours=["second", "a1", "pale"], every=3)), ("ribs", dict(colour="a2", every=(8, 10), curve=1.0)),
         ("contour", dict(colour="a0", depths=[0]))],
    ],
    "stellar_preserve": [
        [("pattern", "Split"), ("flanks", dict(colour="second", depth=2)), ("spine_stripes", dict(colours=["a0", "a1"], start=0.15, stop=0.7)),
         ("roundel", dict(colour="a2", at=0.5))],
        # Leaf: green swept leaves, a teal edge, an amber roundel.
        [("pattern", "Bow"), ("sweep", dict(colours=["a0", "a0"], at=(0.2, 0.3), w=2)), ("contour", dict(colour="second", depths=[0])),
         ("roundel", dict(colour="a2", at=(0.5, 0.6)))],
        # Canopy: a sky-blue spine band, leaf pinstripes, teal tips.
        [("pattern", "Stripe"), ("belly", dict(colour="a1", frac=(0.18, 0.26), border="a0")), ("tips", dict(colour="second", depth=1)),
         ("ticks", dict(colour="a2"))],
        # Growth rings: green rings round the heart, teal flanks.
        [("pattern", "Bands"), ("rings", dict(colour="a0", at=(0.4, 0.5), n=4)), ("flanks", dict(colour="second", depth=1)),
         ("contour", dict(colour="a1", depths=[2]))],
        # Dappled: leaf and sky patches, a teal nose.
        [("pattern", "Bands"), ("patches", dict(colours=["a0", "a1", "second"], every=4)), ("nose", dict(colour="second", frac=(0.1, 0.15))),
         ("contour", dict(colour="a2", depths=[1]))],
    ],
    "broken_choir": [
        [("pattern", "Checker"), ("flanks", dict(colour="a2", depth=1)), ("seams_gold", dict(colour="a0")),
         ("spine_stripes", dict(colours=["a1"], start=0.3, stop=0.7))],
        # Stained glass: panes of violet, ivory and jade leaded in gold.
        [("pattern", "Bands"), ("membrane", dict(line="a0", fills=["a2", None, "a1", None, "second", None, None], size=(4, 6))),
         ("contour", dict(colour="a0", depths=[0]))],
        # Broken halo: gold rings with gaps, violet flanks.
        [("pattern", "Bow"), ("rings", dict(colour="a0", at=(0.35, 0.5), n=3, broken=True)), ("flanks", dict(colour="a2", depth=1)),
         ("spine_chevrons", dict(colour="a1", step=4))],
        # Shards: gold cracks swept back, violet tips.
        [("pattern", "Checker"), ("ribs", dict(colour="a0", every=(6, 8), curve=0.4)), ("tips", dict(colour="a2", depth=1)),
         ("contour", dict(colour="a1", depths=[1]))],
        # Organ: ivory pipes down the spine, a violet nose, gold seams.
        [("pattern", "Stripe"), ("ladder", dict(colour="a1", step=2, w=2)), ("nose", dict(colour="a2", frac=(0.1, 0.16))),
         ("seams_gold", dict(colour="a0", every=13))],
    ],
    "corpse_stars": [
        [("pattern", "Chevrons"), ("bands", dict(colour="a0", line="dark", n=4, w=1)), ("spots", dict(colours=["a1"], every=11))],
        # Ribcage: violet ribs curving off a dark spine.
        [("pattern", "Stripe"), ("belly", dict(colour="second", frac=(0.08, 0.12))), ("ribs", dict(colour="a0", every=(4, 5), curve=1.6)),
         ("contour", dict(colour="a1", depths=[0]))],
        # Decay: grey-green rot and ember spots.
        [("pattern", "Bands"), ("patches", dict(colours=["a2", "dark", "second"], every=3)), ("spots", dict(colours=["a1"], every=9)),
         ("contour", dict(colour="a0", depths=[1]))],
        # Shroud: a violet stern, ember sweep, violet edge.
        [("pattern", "Chevrons"), ("zone", dict(colour="second", start=(0.6, 0.7), stop=1.0)), ("sweep", dict(colours=["a1"], at=(0.2, 0.3), w=1)),
         ("contour", dict(colour="a0", depths=[0])), ("ladder", dict(colour="a0", step=3, w=1))],
        # Ossuary: violet rings of bone, a checkered stern.
        [("pattern", "Bow"), ("rings", dict(colour="a0", at=(0.35, 0.5), n=3)), ("stern", dict(colour="second", frac=(0.1, 0.15), checker=True)),
         ("contour", dict(colour="a2", depths=[2]))],
    ],
    "the_silence": [
        # Pinstripe: one pale gold line, now with a gold hull line and quiet
        # slate ticks.
        [("pattern", "Bands"), ("spine_stripes", dict(colours=["a0"], start=0.15, stop=0.9)), ("contour", dict(colour="a0", depths=[2])),
         ("ticks", dict(colour="a2"))],
        # Habit: a charcoal stern, ruled off in gold.
        [("pattern", "Stripe"), ("zone", dict(colour="second", start=(0.58, 0.7), stop=1.0, border="a0")), ("contour", dict(colour="a2", depths=[1])),
         ("roundel", dict(colour="a0", at=(0.25, 0.35)))],
        # Cowl: a charcoal hood over the bow, slate tips, a gold hull line.
        [("pattern", "Bands"), ("nose", dict(colour="second", frac=(0.14, 0.2))), ("contour", dict(colour="a0", depths=[1])),
         ("tips", dict(colour="a2", depth=1))],
        # Cloister: gold rungs down the spine, a slate border.
        [("pattern", "Bands"), ("ladder", dict(colour="a0", step=3, w=1)), ("contour", dict(colour="a2", depths=[0, 3])),
         ("disc", dict(colour="second", at=(0.5, 0.6), ring="a0"))],
        # Vow: a charcoal band down the spine, gold either side.
        [("pattern", "Bow"), ("belly", dict(colour="second", frac=(0.12, 0.2), border="a0")), ("rings", dict(colour="a2", at=(0.3, 0.4), n=2)),
         ("contour", dict(colour="a0", depths=[0]))],
    ],
    "shepherd": [
        [("pattern", "Bow"), ("nose", dict(colour="a0", frac=0.14)), ("spine_stripes", dict(colours=["a1"], start=0.2)), ("roundel", dict(colour="a2", at=0.55))],
        # Fleece: cream wool in cells, a brown nose.
        [("pattern", "Bow"), ("membrane", dict(line="a0", fills=["a1", None, None, None], size=(4, 5))), ("nose", dict(colour="a0", frac=(0.1, 0.14))),
         ("contour", dict(colour="second", depths=[0]))],
        # Brand: a big red mark, brown rungs.
        [("pattern", "Bands"), ("disc", dict(colour="a2", at=(0.4, 0.55), ring="a1")), ("ladder", dict(colour="a0", step=3, w=2)),
         ("contour", dict(colour="a0", depths=[0]))],
        # Hurdle: fence rails across the hull, red tips.
        [("pattern", "Bands"), ("bands", dict(colour="a0", line="a1", n=4, w=1)), ("contour", dict(colour="a0", depths=[2])),
         ("tips", dict(colour="a2", depth=1))],
        # Pasture: a brown spine band, cream flanks, a red roundel.
        [("pattern", "Stripe"), ("belly", dict(colour="second", frac=(0.14, 0.2), border="a1")), ("flanks", dict(colour="a1", depth=1)),
         ("roundel", dict(colour="a2", at=(0.3, 0.45)))],
    ],
    "recursive_kingdom": [
        [("pattern", "Chevrons"), ("generations", dict(colours=["a0", "a1", "a2"]))],
        # Nested: the ship's own outline repeated inward, copy within copy.
        [("pattern", "Stripe"), ("contour", dict(colour="a0", depths=[1, 4, 7, 10, 13], cycle=["a0", "a1", "a2"]))],
        # Halving: three sweeps, each half the one before.
        [("pattern", "Bands"), ("sweep", dict(colours=["a0", "a1", "a2"], at=(0.15, 0.2), w=4, halve=True)), ("contour", dict(colour="second", depths=[0]))],
        # Sierpinski: the triangle that is three of itself.
        [("pattern", "Bands"), ("sierpinski", dict(colour="a0", line="second")), ("tips", dict(colour="a2", depth=1))],
        # Copy marks: a checkered stern, teal rungs, cream tips.
        [("pattern", "Chevrons"), ("stern", dict(colour="second", frac=(0.1, 0.16), checker=True)), ("ladder", dict(colour="a0", step=2, w=2)),
         ("tips", dict(colour="a1", depth=1)), ("contour", dict(colour="a2", depths=[1]))],
    ],
}

# Ships the developer asked for by colour.
OVERRIDES = {
    ("terran_hegemony", "claymore"): 0,   # red and blue
    ("terran_hegemony", "unifier"): 2,    # red and blue
    ("terran_hegemony", "sovereign"): 3,  # yellow and blue
}


def _seed(text):
    h = 2166136261
    for ch in text.encode():
        h = ((h ^ ch) * 16777619) & 0xFFFFFFFF
    return h


class Rng:
    def __init__(self, seed):
        self.s = seed or 1

    def next(self):
        self.s = (self.s * 1103515245 + 12345) & 0x7FFFFFFF
        return self.s / 0x7FFFFFFF


class Painter:
    def __init__(self, design, theme, seed=1):
        self.t = theme
        self.rng = Rng(seed)
        self.cells = {tuple(c["grid_pos"]) for c in design["hull_cells"]}
        xs = [c[0] for c in self.cells]
        ys = [c[1] for c in self.cells]
        self.lo, self.hi = (min(xs), min(ys)), (max(xs), max(ys))
        self.s = self.lo[1] + self.hi[1]          # row y mirrors to s - y
        self.len = self.hi[0] - self.lo[0] + 1
        self.wide = (self.hi[1] - self.lo[1] + 2) // 2
        # Motifs are drawn in units that grow with the ship, so a flagship's
        # stripes read as stripes and not as pinstripes.
        self.u = max(1, round(self.len / 42))
        self.pattern = theme["pattern"]
        # What stands on each cell.
        self.room = {}
        self.engines = set()
        for m in design["modules"]:
            cat = CATEGORY.get(m["module_type"])
            for c in fleet.footprint(m["module_type"], tuple(m["grid_pos"]), m.get("rotation") or "North"):
                if cat and cat not in SKIP:
                    self.room[c] = cat
                if cat == "Propulsion" and "Engine" in m["module_type"]:
                    self.engines.add(c)
        self.edge = {c for c in self.cells if any((c[0] + dx, c[1] + dy) not in self.cells for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))}
        # How far each cell lies inside the hull (0 on its edge).
        self.depth = {c: 0 for c in self.edge}
        ring = list(self.edge)
        while ring:
            nxt = []
            for (x, y) in ring:
                for d in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    n = (x + d[0], y + d[1])
                    if n in self.cells and n not in self.depth:
                        self.depth[n] = self.depth[(x, y)] + 1
                        nxt.append(n)
            ring = nxt
        # Each column's widest reach from the spine.
        self.reach = {}
        for c in self.cells:
            self.reach[c[0]] = max(self.reach.get(c[0], 0), self.dy(c))
        # Each column's spine body: how far the hull runs unbroken out from
        # the middle row (arms and pods beyond a gap don't count).
        self.body = {}
        mid = (self.s + 1) // 2
        for x in range(self.lo[0], self.hi[0] + 1):
            k = 0
            while (x, mid + k) in self.cells:
                k += 1
            self.body[x] = k
        self.decals = []
        self.seen = set()
        self.fill = {}   # cell -> (layer, colour, soft): painted plating, no decal

    # ---------------------------------------------------------------- helpers
    def pick(self, v):
        """A (lo, hi) pair is chosen per ship; anything else is itself."""
        if isinstance(v, tuple) and len(v) == 2:
            a, b = v
            r = self.rng.next()
            if isinstance(a, int) and isinstance(b, int):
                return a + int(r * (b - a + 1)) if b > a else a
            return a + (b - a) * r
        return v

    def col(self, ref):
        if ref is None or isinstance(ref, list):
            return ref
        t, a = self.t, self.t["accents"]
        return {"a0": a[0], "a1": a[1], "a2": a[2], "second": t["second"], "light": t["light"],
                "main": t["main"], "dark": shade(t["main"], 0.62), "pale": mix(t["main"], [255, 255, 255], 0.35)}[ref]

    def dy(self, c):
        return abs(2 * c[1] - self.s) // 2

    def upper(self):
        return sorted((c for c in self.cells if 2 * c[1] >= self.s), key=lambda c: (-c[0], c[1]))

    def mirror(self, c):
        return (c[0], self.s - c[1])

    def paint(self, c, colour, layer=1, soft=False):
        """Paint a cell's plating (and its mirror). Higher layers win; soft
        paint leaves rooms their own tint."""
        for cell in (c, self.mirror(c)):
            if cell in self.cells:
                old = self.fill.get(cell)
                if old is None or old[0] <= layer:
                    self.fill[cell] = (layer, list(colour), soft)

    def put(self, c, shape, turn, colour, layer):
        """A decal and its mirror across the ship's middle (Decal::mirrored).
        A Full decal is just paint, so it goes on the plating instead."""
        if c not in self.cells:
            return
        if shape == "Full":
            return self.paint(c, colour, layer)
        for cell, tn, flip in ((c, turn, False), (self.mirror(c), (4 - turn % 4) % 4, True)):
            if cell not in self.cells or (cell == c and flip):
                continue
            key = (cell, shape, tn, flip, layer)
            if key in self.seen:
                continue
            self.seen.add(key)
            self.decals.append({"cell": list(cell), "shape": shape, "turn": tn, "flip": flip, "colour": list(colour), "layer": layer})

    def line(self, c, colour, layer=2):
        """A thin line through a cell on a small ship, a painted cell on a
        big one (where a pinstripe would vanish)."""
        if self.u == 1:
            self.put(c, "Stripe", 0, colour, layer)
        else:
            self.paint(c, colour, layer)

    # ------------------------------------------------------------------ slots
    def slots(self):
        t = self.t
        out = []
        pw, ph = 4 * self.u, 3 * self.u
        for c in sorted(self.cells):
            cat = self.room.get(c)
            f = self.fill.get(c)
            if f and not (f[2] and cat in ROOM_TINT):
                out.append([list(c), {"Custom": f[1]}])
                continue
            scaled = (c[0] // self.u, c[1] // self.u)
            lo, hi = (self.lo[0] // self.u, self.lo[1] // self.u), (self.hi[0] // self.u, self.hi[1] // self.u)
            if accent_at(self.pattern, scaled, lo, hi):
                out.append([list(c), "Second"])
            elif cat in ROOM_TINT:
                k = ROOM_TINT[cat]
                tint = t["second"] if k == "second" else t["accents"][k]
                out.append([list(c), {"Custom": mix(t["main"], tint, t["room"])}])
            else:
                # Plating laid in panels, each a shade off its neighbours.
                h = _seed(f"{(self.hi[0] - c[0]) // pw},{self.dy(c) // ph}") % 5
                k = (0.93, 1.0, 1.0, 1.05, 0.97)[h]
                out.append([list(c), "Main" if k == 1.0 else {"Custom": shade(t["main"], k)}])
        return out

    # --------------------------------------------------------------- details
    def seams(self):
        """Panel seams across big bare plating: every sixth column and row."""
        dark = shade(self.t["main"], 0.62)
        for c in self.upper():
            if c in self.room or c in self.edge:
                continue
            if (self.hi[0] - c[0]) % (6 * self.u) == 3:
                self.put(c, "Edge", 1, dark, 1)
            if self.dy(c) % (6 * self.u) == 3:
                self.put(c, "Edge", 0, dark, 1)

    def lights(self):
        """Running lights round the hull's edge, about every fifth cell."""
        n = 0
        for c in self.upper():
            if c in self.edge and c not in self.room:
                if n % 5 == 0:
                    self.put(c, "Dot", 0, self.t["light"], 3)
                n += 1

    def engine_hatching(self):
        """Hazard hatching on the plating just ahead of the engines."""
        a = self.t["accents"]
        for (x, y) in self.engines:
            for k in (1, 2):
                c = (x + k, y)
                if c in self.cells and c not in self.room and 2 * c[1] >= self.s:
                    self.put(c, "Slash", 0, shade(a[1], 0.8), 2)

    # ---------------------------------------------------------------- motifs
    def spine_stripes(self, colours, start=0.1, stop=0.85, dashed=False):
        """Stripes bow to stern either side of the spine, one per colour."""
        x0 = self.hi[0] - int(self.len * stop)
        x1 = self.hi[0] - int(self.len * start)
        u = self.u
        for k, col in enumerate(colours):
            for c in self.upper():
                if not x0 <= c[0] <= x1:
                    continue
                if dashed and ((self.hi[0] - c[0]) // (3 * u)) % 2:
                    continue
                d = self.dy(c) - 1 - k * u
                if u == 1 and d == 0:
                    self.put(c, "Stripe", 0, col, 2)
                elif u > 1 and 0 <= d < u:
                    self.paint(c, col, 2)

    def roundel(self, colour, at=0.3, ring=None):
        if self.u > 1:
            return self.disc(colour, at, ring)
        x = self.hi[0] - int(self.len * at)
        y = (self.s + 1) // 2 + (0 if self.s % 2 else max(2, self.wide // 2))
        for dx, dyy, turn in ((0, 0, 0), (-1, 0, 1), (-1, -1, 2), (0, -1, 3)):
            c = (x + dx, y + dyy)
            if 2 * c[1] >= self.s:
                self.put(c, "Arc", turn, colour, 3)

    def disc(self, colour, at, ring=None):
        """A painted roundel 2-3 units across on each flank (or the spine).
        `at` None puts it on the spine at the ship's middle."""
        r = 1.6 * self.u
        on_spine = at is None or self.wide < 6 * self.u
        cx = self.hi[0] - int(self.len * (0.5 if at is None else at))
        cy = (self.s / 2) + (0 if on_spine else self.wide / 2)
        for c in self.upper():
            d = ((c[0] - cx) ** 2 + (c[1] - cy) ** 2) ** 0.5
            if d <= r:
                self.paint(c, colour, 3)
            elif ring and d <= r + 0.9:
                self.paint(c, ring, 3)

    def spine_chevrons(self, colour, step=3):
        for c in self.upper():
            if self.dy(c) < self.u and self.lo[0] + self.len // 5 <= c[0] <= self.hi[0] - 2 and (self.hi[0] - c[0]) % (step * self.u) < self.u:
                self.put(c, "Chevron", 1, colour, 3)

    def nose(self, colour, frac=0.18):
        n = max(2, int(self.len * frac))
        for c in self.upper():
            u = self.hi[0] - c[0] + self.dy(c)
            if u < n:
                self.paint(c, colour, 1)
            elif u == n:
                self.put(c, "Diagonal", 1, colour, 1)

    def flanks(self, colour, depth=2):
        for c in self.upper():
            if self.dy(c) > 0 and any((c[0], c[1] + k) not in self.cells for k in range(1, depth * self.u + 1)):
                self.paint(c, colour, 1)

    def bands(self, colour, line, n=3, w=2):
        w *= self.u
        gap = max(self.len // (n + 1), w + 1)
        for k in range(1, n + 1):
            x0 = self.hi[0] - k * gap
            for c in self.upper():
                if x0 - w + 1 <= c[0] <= x0:
                    self.paint(c, colour, 1)
                    if c[0] == x0:
                        self.put(c, "Edge", 1, line, 2)
                    if c[0] == x0 - w + 1:
                        self.put(c, "Edge", 3, line, 2)

    def seams_gold(self, colour, every=9):
        """Kintsugi: gold cracks running slantwise over the plating, mended."""
        for c in self.upper():
            if c in self.room:
                continue
            zig = (self.dy(c) // (2 * self.u)) % 2
            v = (self.hi[0] - c[0]) + (self.dy(c) % (2 * self.u)) * (1 if zig else -1)
            if v % (every * self.u) < self.u:
                self.paint(c, colour, 2)

    def spots(self, colours, every=7):
        for c in self.upper():
            if c in self.room or c in self.edge:
                continue
            u = self.u
            h = ((c[0] // u) * 7 + (self.dy(c) // u) * 13) % every
            if h == 0:
                colour = colours[(c[0] // (every * u)) % len(colours)]
                if u == 1:
                    self.put(c, "Circle", 0, colour, 2)
                else:
                    self.paint(c, colour, 2)

    def hazard(self, yellow, black, frac=0.1):
        """Yellow-and-black hatching across the bow (the push bar)."""
        n = max(2, int(self.len * frac))
        for c in self.upper():
            if self.hi[0] - c[0] < n:
                self.paint(c, yellow, 1)
                self.put(c, "Slash", 0, black, 2)

    def generations(self, colours):
        """The Recursive Kingdom's chevrons, each copy's band in its own
        generation's colour."""
        for c in self.upper():
            if c in self.room:
                continue
            v = (c[0] + self.dy(c)) // self.u
            if v % 7 < 2:
                self.paint(c, colours[(v // 7) % len(colours)], 1)

    # -- shapes that follow the hull -------------------------------------
    def contour(self, colour, depths, cycle=None):
        """Lines that follow the hull's own outline, `depths` cells in from
        its edge (scaled with the ship): a border, a pinstripe, or the
        ship's shape repeated inward."""
        for i, d in enumerate(depths):
            col = cycle[i % len(cycle)] if cycle else colour
            want = d * self.u
            for c in self.upper():
                dc = self.depth.get(c, -9)
                if not (dc == want or (self.u > 1 and d > 0 and want <= dc < want + max(1, self.u // 2))):
                    continue
                # Inside lines only where the hull is wide enough to hold
                # them: a thin arm's middle row would take the line whole.
                # A border only round plating with an inside to it, so thin
                # spars and arms keep the hull colour.
                if not any(self.depth.get((c[0] + a, c[1] + b), -1) > dc + max(1, self.u // 2)
                           for a in (-2, 0, 2) for b in (-2, 0, 2)):
                    continue
                self.paint(c, self.col(col), 2 if d else 1)

    def tips(self, colour, depth=1, hatch=None):
        """The ship's farthest reaches port and starboard painted."""
        far = max(self.reach.values())
        for c in self.upper():
            if self.dy(c) >= far - depth * self.u - (far // 8):
                self.paint(c, colour, 2)
                if hatch:
                    self.put(c, "Slash", 0, self.col(hatch), 3)

    def zone(self, colour, start, stop, border=None):
        """A whole section of hull in another colour, bow fraction `start`
        to `stop`; its bowward edge ruled in `border`."""
        x1 = self.hi[0] - int(self.len * start)
        x0 = self.hi[0] - int(self.len * stop)
        for c in self.upper():
            if x0 <= c[0] <= x1:
                self.paint(c, colour, 1, soft=True)
                if border and x1 - self.u < c[0] <= x1 and start > 0:
                    self.paint(c, self.col(border), 2)

    def stern(self, colour, frac=0.1, checker=False):
        n = max(2, int(self.len * frac))
        for c in self.upper():
            if c[0] < self.lo[0] + n:
                if not checker or ((c[0] // self.u) + (c[1] // self.u)) % 2 == 0:
                    self.paint(c, colour, 1)

    def belly(self, colour, frac=0.2, border=None):
        """A broad band down the spine."""
        x0, x1 = self.lo[0] + self.len // 10, self.hi[0] - self.len // 12
        for c in self.upper():
            if not x0 <= c[0] <= x1:
                continue
            # As a share of the spine body there, so a band down a slim
            # hull with long arms doesn't swallow the hull.
            w = max(1, round(min(self.wide, 2 * self.body.get(c[0], 0)) * frac))
            d = self.dy(c)
            if d < w:
                self.paint(c, colour, 1, soft=True)
            elif border and d < w + self.u:
                self.paint(c, self.col(border), 2)

    def sweep(self, colours, at=0.2, w=2, halve=False):
        """Bands swept back from the spine, pointing at the bow."""
        a = int(self.len * at)
        w = w * self.u
        for col in colours:
            for c in self.upper():
                v = self.hi[0] - c[0] - self.dy(c)
                if a <= v < a + w:
                    self.paint(c, self.col(col), 1)
                elif v == a - 1 and self.dy(c) > 0 and self.u == 1:
                    self.put(c, "Diagonal", 0, self.col(col), 1)
                elif v == a + w and self.dy(c) > 0 and self.u == 1:
                    self.put(c, "Diagonal", 2, self.col(col), 1)
            a += w + max(2, w // 2 if halve else self.u * 2)
            if halve:
                w = max(self.u, w // 2)

    def ladder(self, colour, step=3, w=2):
        """Rungs across the spine: deck markings, vertebrae, organ pipes."""
        x0, x1 = self.lo[0] + self.len // 6, self.hi[0] - self.len // 8
        for c in self.upper():
            if x0 <= c[0] <= x1 and self.dy(c) < w * self.u and (self.hi[0] - c[0]) % (step * self.u) < max(1, self.u // 2):
                self.paint(c, colour, 2)

    def ticks(self, colour):
        """A short row of marks on each flank near the bow: hull numbers."""
        far = self.reach.get(self.hi[0] - self.len // 5, self.wide)
        y = max(1, far // 2)
        x1 = self.hi[0] - self.len // 6
        for k in range(4):
            x = x1 - k * 2 * self.u
            for j in range(self.u):
                for i in range(self.u):
                    c = (x - i, (self.s + 1) // 2 + y + j)
                    if c in self.cells and c not in self.room:
                        self.paint(c, colour, 3)

    def ribs(self, colour, every=6, curve=1.0):
        """Lines curving back from the spine to the flanks: ribs, veins."""
        x0, x1 = self.lo[0] + self.len // 8, self.hi[0] - self.len // 8
        for c in self.upper():
            if not x0 <= c[0] <= x1 or self.dy(c) == 0:
                continue
            v = (self.hi[0] - c[0]) - curve * self.dy(c) ** 2 / max(1, self.wide)
            if int(v // self.u) % every == 0:
                self.paint(c, colour, 2)

    def rings(self, colour, at=0.5, n=3, broken=False):
        """Concentric rings round a point on the spine."""
        cx = self.hi[0] - self.len * at
        step = 3 * self.u
        for c in self.upper():
            d = ((c[0] - cx) ** 2 + self.dy(c) ** 2) ** 0.5
            k = int(d // step)
            if 1 <= k <= n and d % step < self.u:
                if broken and (int(d) + c[0]) % 5 == 0:
                    continue
                self.paint(c, colour, 2)

    def sunburst(self, colour, at=0.4, rays=8):
        import math
        cx = self.hi[0] - self.len * at
        r = max(self.wide * 1.4, 6)
        for c in self.upper():
            dx, dyy = c[0] - cx, self.dy(c)
            d = (dx * dx + dyy * dyy) ** 0.5
            if 2 * self.u < d <= r:
                ang = math.atan2(dyy, dx) / math.pi * rays
                if ang % 1.0 < 0.3:
                    self.paint(c, colour, 1)

    def awning(self, colours, start=0.3, stop=0.7):
        x1 = self.hi[0] - int(self.len * start)
        x0 = self.hi[0] - int(self.len * stop)
        for c in self.upper():
            if x0 <= c[0] <= x1:
                k = ((x1 - c[0]) // max(1, self.u)) % (2 * len(colours))
                if k % 2 == 0:
                    self.paint(c, self.col(colours[k // 2]), 1, soft=True)

    def patches(self, colours, every=4):
        """Odd plates here and there in other colours: patched, mottled."""
        pw, ph = 3 * self.u, 2 * self.u
        for c in self.upper():
            bx, by = (self.hi[0] - c[0]) // pw, self.dy(c) // ph
            h = _seed(f"p{bx},{by},{self.len}")
            if h % every == 0:
                self.paint(c, self.col(colours[(h // every) % len(colours)]), 1, soft=True)

    def membrane(self, line, fills, size=5):
        """The hull divided into irregular cells (a membrane, stained glass,
        a fleece): each cell its own colour, the walls in `line`."""
        step = size * self.u
        seeds = []
        for gx in range(0, self.len + step, step):
            for gy in range(0, self.wide + step, step):
                h = _seed(f"c{gx},{gy},{self.len}")
                seeds.append((self.hi[0] - gx - (h % step), gy + ((h >> 8) % step), (h >> 16) % len(fills)))

        def owner(c):
            best, bi = None, 0
            for i, (sx, sy, _) in enumerate(seeds):
                d = (c[0] - sx) ** 2 + (self.dy(c) - sy) ** 2
                if best is None or d < best:
                    best, bi = d, i
            return bi

        own = {c: owner(c) for c in self.upper()}
        for c, i in own.items():
            f = fills[seeds[i][2]]
            n = own.get((c[0] + 1, c[1]))
            m = own.get((c[0], c[1] + 1))
            if (n is not None and n != i) or (m is not None and m != i):
                self.paint(c, self.col(line), 2)
            elif f:
                self.paint(c, self.col(f), 1, soft=True)

    def sierpinski(self, colour, line=None):
        """The Sierpinski triangle laid from the bow."""
        for c in self.upper():
            x = (self.hi[0] - c[0]) // self.u
            y = self.dy(c) // self.u
            if y <= x and (x - y) & y == 0:
                self.paint(c, colour, 1)

    # ------------------------------------------------------------- schemes
    def scheme(self, steps):
        for name, args in steps:
            if name == "pattern":
                self.pattern = args
                continue
            kw = {}
            for k, v in args.items():
                v = self.pick(v)
                if k in ("colour", "yellow", "black", "line", "ring", "border") and isinstance(v, str):
                    v = self.col(v) if k not in ("border",) else v
                elif k in ("colours",):
                    v = [self.col(x) if isinstance(x, str) else x for x in v]
                kw[k] = v
            getattr(self, name)(**kw)


def scheme_for(slug, key):
    schemes = SCHEMES[slug]
    if key is None:
        return 0
    if (slug, key) in OVERRIDES:
        return OVERRIDES[(slug, key)]
    folder = os.path.join(ROOT, "tools", "fleet-concepts", "concepts", slug)
    keys = sorted(f[:-5] for f in os.listdir(folder) if f.endswith(".json"))
    pinned = {k for (s, k) in OVERRIDES if s == slug}
    free = [k for k in keys if k not in pinned]
    i = free.index(key) if key in free else _seed(key)
    return i % len(schemes)


def paint(slug, design, key=None):
    """A refit design's ShipPaint (serde form) in its faction's livery: the
    faction's main colour always, one of its schemes over it."""
    t = THEMES[slug]
    p = Painter(design, t, _seed(f"{slug}/{key}"))
    p.seams()
    p.scheme(SCHEMES[slug][scheme_for(slug, key)])
    p.engine_hatching()
    p.lights()
    # The faction's main colour must still be what the ship is: where the
    # scheme's motifs already cover much of the hull, the livery pattern
    # underneath steps aside.
    slots = p.slots()
    if main_share(slots, t["main"]) < 0.42:
        p.pattern = None
        slots = p.slots()
    s = lambda c: [round(v / 255, 4) for v in c]  # noqa: E731
    return {"main": s(t["main"]), "second": s(t["second"]), "trim": s(t["accents"][0]), "warm": False,
            "slots": slots, "decals": p.decals}


def main_share(slots, main):
    """How much of the hull wears the main colour (or a plating shade of it)."""
    n = 0
    for _, s in slots:
        if s == "Main" or (isinstance(s, dict) and sum(abs(s["Custom"][i] - main[i]) for i in range(3)) < 40):
            n += 1
    return n / max(1, len(slots))
