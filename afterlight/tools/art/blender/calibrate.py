"""Solve for the key-light energy that reproduces an authored colour.

Renders a flat swatch of PALETTE['body'] at several light energies. The one
whose output matches the input hex is the exposure every sprite should use,
so authored palette values survive the render untouched.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import (  # noqa: E402
    new_scene, metal, light_rig, ortho_camera, configure, render_to,
    box, PALETTE, argv_after_ddash,
)

OUT = argv_after_ddash()[0] if argv_after_ddash() else "/tmp/cal"

for energy in (200, 400, 600, 800, 1100, 1500):
    scene = new_scene()
    mat = metal("swatch", "body")
    box("swatch", (4.0, 4.0, 0.2), (0.0, 0.0, 0.0), mat)
    light_rig(scene, energy=float(energy))
    ortho_camera(scene, 1.0)
    configure(scene, 64, samples=24)
    render_to(scene, "%s_%d.png" % (OUT, energy))
