#!/usr/bin/env python3
"""UI icon set, authored as SVG and rasterised by Inkscape.

Flat monochrome vector at small size is the one job in this project where
Blender is the wrong tool and hand-authored SVG is exactly right.

Counts come from the code, not ART_BRIEF: ModuleCategory has 10 variants
(the brief lists 8, missing Detection and Storage) and KineticAmmoType has 15
(9 conventional + 6 exotic), not 9.

Icons are drawn in a single colour so the UI can tint them per state; the
palette accent is applied at render time, not baked in.

    python3 tools/art/icons/gen_icons.py
"""

import os
import subprocess

INKSCAPE = "/Applications/Inkscape.app/Contents/MacOS/inkscape"
SVG_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "svg")
PNG_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "png")
FG = "#c8d2e0"          # neutral light; the UI tints from here
SIZE = 128              # rasterise at 2x the 64px spec for crisp downscale


def svg(body, stroke_w=5.0):
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" '
        'width="64" height="64">'
        '<g fill="none" stroke="%s" stroke-width="%s" stroke-linecap="round" '
        'stroke-linejoin="round">%s</g></svg>' % (FG, stroke_w, body)
    )


def solid(body):
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" '
            'width="64" height="64"><g fill="%s" stroke="none">%s</g></svg>'
            % (FG, body))


# ------------------------------------------------------------- categories
CATEGORIES = {
    # Bolt inside a containment ring.
    "cat_power": svg('<circle cx="32" cy="32" r="22"/>'
                     '<path d="M35 18 L24 34 h9 l-4 12 11-16 h-9 z" fill="%s" '
                     'stroke="none"/>' % FG),
    # Nozzle with exhaust chevrons.
    "cat_propulsion": svg('<path d="M22 12 h20 v18 l8 12 H14 l8-12 z"/>'
                          '<path d="M24 50 l8 8 8-8"/>'
                          '<path d="M28 44 l4 5 4-5"/>'),
    # Circulation loop with an air droplet.
    "cat_lifesupport": svg('<path d="M32 14 a18 18 0 1 1-13 5"/>'
                           '<path d="M19 8 v11 h11"/>'
                           '<path d="M32 26 c6 7 9 11 9 15 a9 9 0 0 1-18 0 '
                           'c0-4 3-8 9-15 z"/>'),
    # Console screen with a cursor reticle.
    "cat_control": svg('<rect x="9" y="14" width="46" height="32" rx="4"/>'
                       '<path d="M24 54 h16"/><path d="M32 46 v8"/>'
                       '<circle cx="32" cy="30" r="6"/>'
                       '<path d="M32 20 v4 M32 36 v4 M22 30 h4 M38 30 h4"/>'),
    # Muzzle and crosshair.
    "cat_weapons": svg('<path d="M10 26 h26 v12 H10 z"/>'
                       '<path d="M36 30 h12"/>'
                       '<circle cx="50" cy="32" r="9"/>'
                       '<path d="M50 20 v5 M50 39 v5 M38 32 h5 M57 32 h5"/>'),
    # Radar sweep.
    "cat_detection": svg('<path d="M32 52 A20 20 0 0 1 32 12"/>'
                         '<path d="M32 52 A20 20 0 0 0 32 12"/>'
                         '<path d="M32 32 L50 20"/>'
                         '<circle cx="32" cy="32" r="3" fill="%s"/>' % FG),
    # Crate with strapping.
    "cat_storage": svg('<rect x="11" y="16" width="42" height="34" rx="3"/>'
                       '<path d="M11 27 h42 M11 39 h42 M32 16 v34"/>'),
    # Crew figure.
    "cat_crew": svg('<circle cx="32" cy="21" r="8"/>'
                    '<path d="M15 52 a17 17 0 0 1 34 0"/>'),
    # Wrench.
    "cat_utility": svg('<path d="M41 12 a12 12 0 1 0 9 20 l-2 2 L26 56 '
                       'a6 6 0 0 1-9-9 L39 25 l2-2 a12 12 0 0 1 0-11 z"/>'
                       '<path d="M41 12 l-7 7 6 6 7-7"/>'),
    # Custom / player-assembled: a module outline with a plus.
    "cat_custom": svg('<rect x="11" y="11" width="42" height="42" rx="4"/>'
                      '<path d="M32 22 v20 M22 32 h20"/>'),
    # I-beam / girder -- framing, bracing, bulkheads.
    "cat_structural": svg('<path d="M14 12 h36 M14 52 h36"/>'
                          '<path d="M22 12 v40 M42 12 v40"/>'
                          '<path d="M22 32 h20"/>'),
    # Hull: staggered plating, the wall you lay rather than the frame inside
    # it. Hull borrowed the girder until Structural got a tab of its own and
    # the two sat side by side wearing the same picture.
    "cat_hull": svg('<rect x="10" y="12" width="44" height="40" rx="3"/>'
                    '<path d="M10 25.3 h44 M10 38.7 h44"/>'
                    '<path d="M25 12 v13.3 M39 25.3 v13.4 M25 38.7 v13.3"/>'),
}

# ------------------------------------------------------- conventional ammo
AMMO = {
    # AP "Solid" -- plain penetrator.
    "ammo_ap": svg('<path d="M32 8 L42 24 v28 H22 V24 z"/><path d="M22 44 h20"/>'),
    # APHE "Cracker" -- penetrator with a burst inside.
    "ammo_aphe": svg('<path d="M32 8 L42 24 v28 H22 V24 z"/>'
                     '<path d="M32 28 l3 6 6-2-4 6 4 6-6-2-3 6-3-6-6 2 4-6-4-6 6 2 z" '
                     'fill="%s" stroke="none"/>' % FG),
    # HEFrag "Shredder" -- surface burst throwing fragments.
    "ammo_hefrag": svg('<circle cx="32" cy="32" r="10"/>'
                       '<path d="M32 14 v-6 M32 50 v6 M14 32 h-6 M50 32 h6 '
                       'M19 19 l-4-4 M45 19 l4-4 M19 45 l-4 4 M45 45 l4 4"/>'),
    # Incendiary "Torch" -- flame.
    "ammo_incendiary": svg('<path d="M32 56 c-10 0-17-7-17-16 0-10 9-14 9-24 '
                           '0 0 12 5 12 16 3-3 4-7 4-7 5 5 9 10 9 15 0 9-7 16-17 16 z"/>'
                           '<path d="M32 48 a6 6 0 0 1-6-6 c0-5 6-7 6-13 '
                           '4 4 6 8 6 13 a6 6 0 0 1-6 6 z"/>'),
    # EMPShell "Blackout" -- round wrapped in arcs.
    "ammo_emp": svg('<path d="M32 10 L40 24 v20 H24 V24 z"/>'
                    '<path d="M14 22 a22 22 0 0 0 0 22 M50 22 a22 22 0 0 1 0 22"/>'
                    '<path d="M34 30 l-6 8 h7 l-5 8"/>'),
    # Flak "Curtain" -- airburst cloud.
    "ammo_flak": svg('<circle cx="32" cy="30" r="7"/>'
                     '<circle cx="16" cy="18" r="3" fill="%s"/>'
                     '<circle cx="48" cy="18" r="3" fill="%s"/>'
                     '<circle cx="14" cy="42" r="3" fill="%s"/>'
                     '<circle cx="50" cy="42" r="3" fill="%s"/>'
                     '<circle cx="32" cy="52" r="3" fill="%s"/>'
                     '<circle cx="32" cy="10" r="3" fill="%s"/>' % ((FG,) * 6)),
    # HEAT "Lance" -- shaped charge cone and jet.
    "ammo_heat": svg('<path d="M18 14 h28 L32 38 z"/>'
                     '<path d="M32 38 v18"/><path d="M27 50 l5 8 5-8"/>'),
    # HESH "Bell" -- squash head with shock rings.
    "ammo_hesh": svg('<path d="M22 12 h20 v14 a10 10 0 0 1-20 0 z"/>'
                     '<path d="M16 38 a22 22 0 0 0 32 0"/>'
                     '<path d="M12 48 a30 30 0 0 0 40 0"/>'),
    # APFSDS "Rod" -- long dart with fins.
    "ammo_apfsds": svg('<path d="M32 6 L36 18 v28 h-8 V18 z"/>'
                       '<path d="M28 46 l-8 12 M36 46 l8 12 M32 46 v12"/>'),
}

# -------------------------------------------------------------- exotic ammo
EXOTIC = {
    "ammo_plasmaslug": svg('<path d="M32 10 L41 26 v18 H23 V26 z"/>'
                           '<circle cx="32" cy="33" r="5" fill="%s"/>'
                           '<path d="M20 52 q6 -5 12 0 t12 0"/>'
                           '<path d="M20 60 q6 -5 12 0 t12 0"/>' % FG),
    "ammo_antimatter": svg('<circle cx="17" cy="32" r="11" fill="%s" stroke="none"/>'
                           '<circle cx="47" cy="32" r="11"/>'
                           '<path d="M32 14 v36"/>'
                           '<path d="M26 20 l-5-6 M38 20 l5-6 '
                           'M26 44 l-5 6 M38 44 l5 6"/>' % FG),
    "ammo_singularity": svg('<circle cx="32" cy="32" r="8" fill="%s" stroke="none"/>'
                            '<ellipse cx="32" cy="32" rx="24" ry="9"/>'
                            '<ellipse cx="32" cy="32" rx="16" ry="22"/>' % FG),
    "ammo_nanite": svg('<path d="M32 10 l8 5 v10 l-8 5-8-5V15 z"/>'
                       '<path d="M14 34 l8 5 v10 l-8 5-8-5V39 z"/>'
                       '<path d="M50 34 l8 5 v10 l-8 5-8-5V39 z"/>'
                       '<path d="M28 27 l-8 6 M36 27 l8 6"/>'),
    "ammo_phaseslug": svg('<path d="M26 10 L36 24 v26 H26 z"/>'
                          '<path d="M38 14 L48 28 v22 H38 z" stroke-dasharray="5 5"/>'),
    "ammo_neutron": svg('<circle cx="32" cy="32" r="6" fill="%s"/>'
                        '<ellipse cx="32" cy="32" rx="24" ry="10"/>'
                        '<ellipse cx="32" cy="32" rx="24" ry="10" '
                        'transform="rotate(60 32 32)"/>'
                        '<ellipse cx="32" cy="32" rx="24" ry="10" '
                        'transform="rotate(-60 32 32)"/>' % FG),
}


def main():
    os.makedirs(SVG_DIR, exist_ok=True)
    os.makedirs(PNG_DIR, exist_ok=True)
    allicons = {}
    allicons.update(CATEGORIES)
    allicons.update(AMMO)
    allicons.update(EXOTIC)

    for name, markup in allicons.items():
        sp = os.path.join(SVG_DIR, name + ".svg")
        with open(sp, "w") as fh:
            fh.write(markup)
        subprocess.run([INKSCAPE, sp, "--export-type=png",
                        "--export-filename=" + os.path.join(PNG_DIR, name + ".png"),
                        "--export-width=%d" % SIZE, "--export-height=%d" % SIZE],
                       check=True, capture_output=True)
    print("wrote %d icons (%d categories, %d conventional ammo, %d exotic)"
          % (len(allicons), len(CATEGORIES), len(AMMO), len(EXOTIC)))


if __name__ == "__main__":
    main()
