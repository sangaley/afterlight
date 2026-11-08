#!/usr/bin/env python3
"""Render the crew states in Blender and pack each into a single-row strip.

Blender's bundled Python has no Pillow, so it writes one PNG per frame and
this packs them. Strips are horizontal single-row sheets at 64px per frame,
matching the creature convention (stalker.png is 384x64 = 6 frames of 64).

    python3 tools/art/pixel/pack_crew.py
"""

import os
import subprocess
import sys
from PIL import Image

BLENDER = "/Applications/Blender.app/Contents/MacOS/Blender"
SCRIPT = "tools/art/blender/crew.py"
OUT_DIR = "assets/sprites/crew"
TMP = "/private/tmp/claude-502/-Users-shhh-afterlight/a3544012-f31a-4a85-8b14-cf83e6046942/scratchpad/crew_build"
FRAME = 64
STATES = {"walk": 6, "idle": 3, "work": 3, "dead": 1}


def main():
    os.makedirs(OUT_DIR, exist_ok=True)
    os.makedirs(TMP, exist_ok=True)
    for state, n in STATES.items():
        base = os.path.join(TMP, state + ".png")
        subprocess.run([BLENDER, "-b", "-P", SCRIPT, "--", state, base],
                       check=True, capture_output=True)
        sheet = Image.new("RGBA", (FRAME * n, FRAME), (0, 0, 0, 0))
        for i in range(n):
            f = "%s_%02d.png" % (base[:-4], i)
            im = Image.open(f).convert("RGBA").resize((FRAME, FRAME), Image.LANCZOS)
            sheet.paste(im, (i * FRAME, 0), im)
        path = os.path.join(OUT_DIR, "crew_%s.png" % state)
        sheet.save(path)
        print("%-18s %d frames  %dx%d" % (os.path.basename(path), n, *sheet.size))


if __name__ == "__main__":
    main()
