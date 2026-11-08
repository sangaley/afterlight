"""Shared Blender scene rig for Afterlight sprite rendering.

Every sprite is rendered the same way: orthographic straight down, flat
overhead key light, no directional shadows (ART_BRIEF requires this -- the
engine rotates sprites freely, so baked directional light breaks them).

Run headless:
    /Applications/Blender.app/Contents/MacOS/Blender -b -P <script>.py -- [args]
"""

import bpy

# ---------------------------------------------------------------- palette
# Sampled from the existing sprite set. Everything lives in one tight
# desaturated blue-grey band; gold is the only accent and is reserved for
# hazard striping. Keep new art inside this or it will not sit with the
# ~100 sprites we are not regenerating.
PALETTE = {
    "recess":    "#1d2129",  # panel lines, deep cavities
    "dark":      "#2f3945",  # shaded plate
    "body":      "#3c4652",  # dominant hull tone
    "body_warm": "#404754",  # dominant module tone
    "light":     "#4a515f",  # raised faces
    "highlight": "#5a5e66",  # top edges, bevel catches
    "gold":      "#917244",  # hazard striping

    # Accent colours. ART_BRIEF: "Color = information, not decoration." So the
    # accent a weapon carries encodes its DAMAGE TYPE, which doubles as combat
    # readability -- you can tell what is shooting at you by its colour.
    "brass":     "#a87a3c",  # kinetic: shells, feeds, ammunition
    "brass_lit": "#c99a52",
    "ion":       "#3f86c8",  # electromagnetic / EMP / ion
    "ion_lit":   "#63aee6",
    "plasma":    "#bf5730",  # plasma, thermal, fire
    "plasma_lit":"#e08a46",
    "danger":    "#a8443a",  # warheads, explosives
    "danger_lit":"#c9645a",
    "utility":   "#4f9a68",  # tractor / support / non-lethal
    "utility_lit":"#6fbc88",
    "amber":     "#c8963c",  # industrial: drills, cutting gear

}


def srgb_to_linear(c):
    """Blender wants linear; our palette is sRGB hex."""
    c = c / 255.0
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def rgba(hex_or_key, alpha=1.0):
    h = PALETTE.get(hex_or_key, hex_or_key).lstrip("#")
    r, g, b = (int(h[i:i + 2], 16) for i in (0, 2, 4))
    return (srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b), alpha)


# ---------------------------------------------------------------- scene
def new_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    return bpy.context.scene


def metal(name, color, metallic=0.15, roughness=0.55):
    mat = bpy.data.materials.new(name)
    if mat.node_tree is None:          # Blender <6 needs the nudge
        mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = rgba(color)
    bsdf.inputs["Metallic"].default_value = metallic
    bsdf.inputs["Roughness"].default_value = roughness
    return mat


def emissive(name, color, strength=3.0, base=None):
    """A material that EMITS light rather than merely reflecting it.

    Reactor cores and engine plumes are light sources. Rendering them as
    ordinary coloured surfaces under the flat key gives dead flat discs --
    the existing hand-painted art beat the first Blender pass precisely
    because it faked this glow and the render did not.
    """
    mat = bpy.data.materials.new(name)
    if mat.node_tree is None:
        mat.use_nodes = True
    bsdf = mat.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = rgba(base or color)
    bsdf.inputs["Metallic"].default_value = 0.0
    bsdf.inputs["Roughness"].default_value = 0.65
    bsdf.inputs["Emission Color"].default_value = rgba(color)
    bsdf.inputs["Emission Strength"].default_value = strength
    return mat


def radial_glow(name, stops, strength=6.0, radius=0.5):
    """Emissive material with a true RADIAL gradient across a flat disc.

    Uses OBJECT coordinates with the Z scale zeroed, so the falloff is purely
    radial. Generated coordinates normalise each axis independently, which on
    a disc 0.6 wide and 0.05 thick stretches Z enormously and flattens the
    gradient to a dead constant -- that bug produced duller cores than the
    hand-painted art it was meant to beat.

    `stops` runs OUTER (0.0) to CENTRE (1.0). `radius` is the disc radius in
    object space, so the ramp lands exactly on its rim.
    """
    mat = bpy.data.materials.new(name)
    if mat.node_tree is None:
        mat.use_nodes = True
    nt = mat.node_tree
    bsdf = nt.nodes["Principled BSDF"]
    bsdf.inputs["Metallic"].default_value = 0.0
    bsdf.inputs["Roughness"].default_value = 0.7
    bsdf.inputs["Emission Strength"].default_value = strength

    coord = nt.nodes.new("ShaderNodeTexCoord")
    mapping = nt.nodes.new("ShaderNodeMapping")
    mapping.inputs["Scale"].default_value = (1.0 / radius, 1.0 / radius, 0.0)
    grad = nt.nodes.new("ShaderNodeTexGradient")
    grad.gradient_type = "SPHERICAL"
    ramp = nt.nodes.new("ShaderNodeValToRGB")

    nt.links.new(mapping.inputs["Vector"], coord.outputs["Object"])
    nt.links.new(grad.inputs["Vector"], mapping.outputs["Vector"])
    nt.links.new(ramp.inputs["Fac"], grad.outputs["Color"])

    el = ramp.color_ramp.elements
    while len(el) > 1:
        el.remove(el[-1])
    el[0].position = stops[0][0]
    el[0].color = rgba(stops[0][1])
    for pos, col in stops[1:]:
        e = el.new(pos)
        e.color = rgba(col)

    nt.links.new(bsdf.inputs["Base Color"], ramp.outputs["Color"])
    nt.links.new(bsdf.inputs["Emission Color"], ramp.outputs["Color"])
    return mat


ION_STOPS = [(0.00, "#0b1526"), (0.38, "ion"), (0.70, "ion_lit"),
             (0.90, "#d6ecff"), (1.00, "#ffffff")]
PLASMA_STOPS = [(0.00, "#24100a"), (0.36, "plasma"), (0.72, "plasma_lit"),
                (0.93, "#ffdcb0"), (1.00, "#fffaf2")]


def bell(name, r_top, r_bot, depth, location, material, verts=32):
    """A real flared nozzle. Opens toward -Y (engine art vents downward)."""
    bpy.ops.mesh.primitive_cone_add(radius1=r_top, radius2=r_bot, depth=depth,
                                    location=location, vertices=verts)
    ob = bpy.context.object
    ob.name = name
    ob.rotation_euler = (1.5708, 0.0, 0.0)
    ob.data.materials.append(material)
    return ob


def light_rig(scene, energy=600.0, fill=0.16):
    """Flat overhead key + ambient fill. Deliberately shadow-free laterally.

    energy=600 is calibrated: it reproduces PALETTE['body'] (#3c4652) as
    #3e434b on film, ~1.5% per channel. Change it and every sprite shifts
    tone, so recalibrate with calibrate.py if you do.
    """
    ld = bpy.data.lights.new("key", type="AREA")
    ld.energy = energy
    # Slightly cool key. A neutral-white key desaturates the blue-grey palette
    # via specular; this holds the hue through the render.
    ld.color = (0.88, 0.93, 1.0)
    ld.size = 14.0                     # very broad = very soft = no hard shadow
    key = bpy.data.objects.new("key", ld)
    scene.collection.objects.link(key)
    key.location = (0.0, 0.0, 7.0)

    # World fill keeps recesses readable instead of crushing them to black.
    world = bpy.data.worlds.new("w")
    scene.world = world
    if world.node_tree is None:
        world.use_nodes = True
    bg = world.node_tree.nodes["Background"]
    bg.inputs["Color"].default_value = rgba("body")
    bg.inputs["Strength"].default_value = fill
    return key


def ortho_camera(scene, span):
    """Frame exactly `span` world units across. One cell = 1.0."""
    cd = bpy.data.cameras.new("cam")
    cd.type = "ORTHO"
    cd.ortho_scale = span
    cam = bpy.data.objects.new("cam", cd)
    scene.collection.objects.link(cam)
    cam.location = (0.0, 0.0, 6.0)
    cam.rotation_euler = (0.0, 0.0, 0.0)
    scene.camera = cam
    return cam


def configure(scene, res, samples=160):
    scene.render.engine = "CYCLES"
    try:
        scene.cycles.device = "GPU"
        prefs = bpy.context.preferences.addons["cycles"].preferences
        prefs.compute_device_type = "METAL"
        prefs.get_devices()
    except Exception as exc:
        print("GPU unavailable, falling back to CPU:", exc)
    scene.cycles.samples = samples
    scene.cycles.use_denoising = True
    scene.render.resolution_x = res
    scene.render.resolution_y = res
    scene.render.film_transparent = True
    # AgX/Filmic (Blender's default) tone-maps like film stock and darkens
    # authored colour badly. Sprite art must come out as authored.
    scene.view_settings.view_transform = "Standard"
    scene.view_settings.look = "None"
    scene.view_settings.exposure = 0.0
    scene.view_settings.gamma = 1.0
    scene.render.image_settings.file_format = "PNG"
    scene.render.image_settings.color_mode = "RGBA"


def render_to(scene, path):
    scene.render.filepath = path
    bpy.ops.render.render(write_still=True)
    print("WROTE", path)


# ---------------------------------------------------------------- helpers
def box(name, size, location, material, bevel=0.0, segments=3):
    """Axis-aligned box. `size` is full extent (x, y, z), not half."""
    bpy.ops.mesh.primitive_cube_add(size=2.0, location=location)
    ob = bpy.context.object
    ob.name = name
    ob.scale = (size[0] / 2.0, size[1] / 2.0, size[2] / 2.0)
    if bevel > 0.0:
        m = ob.modifiers.new("bev", "BEVEL")
        m.width = bevel
        m.segments = segments
        m.limit_method = "ANGLE"
        m.harden_normals = True
    ob.data.materials.append(material)
    return ob


def cyl(name, radius, depth, location, material, bevel=0.0, verts=24):
    bpy.ops.mesh.primitive_cylinder_add(
        radius=radius, depth=depth, location=location, vertices=verts
    )
    ob = bpy.context.object
    ob.name = name
    if bevel > 0.0:
        m = ob.modifiers.new("bev", "BEVEL")
        m.width = bevel
        m.segments = 2
    ob.data.materials.append(material)
    return ob


def cone(name, radius, depth, location, material, verts=20):
    """Nose cones for missiles. Points +Y once rotated."""
    bpy.ops.mesh.primitive_cone_add(radius1=radius, radius2=0.0, depth=depth,
                                    location=location, vertices=verts)
    ob = bpy.context.object
    ob.name = name
    ob.rotation_euler = (-1.5708, 0.0, 0.0)   # tip toward +Y
    ob.data.materials.append(material)
    return ob


def materials():
    """The shared material set. Every sprite in the game draws from this so a
    reactor and a railgun read as built in the same yard."""
    return {
        "recess": metal("recess", "recess", metallic=0.10, roughness=0.80),
        "dark": metal("dark", "dark", metallic=0.15, roughness=0.60),
        "body": metal("body", "body_warm", metallic=0.15, roughness=0.55),
        "light": metal("light", "light", metallic=0.20, roughness=0.45),
        "highlight": metal("highlight", "highlight", metallic=0.25, roughness=0.38),
        "gold": metal("gold", "gold", metallic=0.35, roughness=0.45),
        "brass": metal("brass", "brass", metallic=0.55, roughness=0.42),
        "brass_lit": metal("brass_lit", "brass_lit", metallic=0.55, roughness=0.35),
        "ion": metal("ion", "ion", metallic=0.30, roughness=0.35),
        "ion_lit": metal("ion_lit", "ion_lit", metallic=0.25, roughness=0.28),
        "plasma": metal("plasma", "plasma", metallic=0.25, roughness=0.40),
        "plasma_lit": metal("plasma_lit", "plasma_lit", metallic=0.20, roughness=0.32),
        "danger": metal("danger", "danger", metallic=0.20, roughness=0.48),
        "danger_lit": metal("danger_lit", "danger_lit", metallic=0.20, roughness=0.40),
        "utility": metal("utility", "utility", metallic=0.28, roughness=0.40),
        "utility_lit": metal("utility_lit", "utility_lit", metallic=0.24, roughness=0.33),
        "amber": metal("amber", "amber", metallic=0.45, roughness=0.42),
        # Light SOURCES, not surfaces.
        "glow_ion": emissive("glow_ion", "ion_lit", 3.2, base="ion"),
        "glow_ion_hot": emissive("glow_ion_hot", "ion_lit", 7.0, base="ion_lit"),
        "glow_plasma": emissive("glow_plasma", "plasma_lit", 3.4, base="plasma"),
        "glow_plasma_hot": emissive("glow_plasma_hot", "plasma_lit", 8.0,
                                    base="plasma_lit"),
        "glow_utility": emissive("glow_utility", "utility_lit", 3.0, base="utility"),
    }


# How far the plate is pushed past the camera frame. The bevel rounds every
# edge including the vertical corners, and rounded corners cannot tile -- they
# leave a diamond of empty space where four blocks meet. Overscanning crops the
# rounding away, so what survives is a square block that butts against its
# neighbour with no seam.
PLATE_BLEED = 1.07


def armour_base(m, hazard=True, w=1.0, h=1.0):
    """The chassis every module sits on: bevelled plate, corner bolts, and an
    optional hazard strip along the mounting edge.

    `w`/`h` are in CELLS. The plate is drawn past the frame (see PLATE_BLEED)
    so the sprite is full-bleed: blocks in game sit on a 66-unit grid and are
    drawn at exactly 66 units, so any transparent margin here becomes a
    visible gap between the blocks of a ship -- and since only about half of a
    hull's modules sit over a hull plate, most of those gaps show open space.
    """
    box("plate", (w * PLATE_BLEED, h * PLATE_BLEED, 0.08), (0.0, 0.0, 0.04),
        m["body"], bevel=0.013)
    bx, by = w / 2.0 - 0.075, h / 2.0 - 0.075
    for sx in (-1, 1):
        for sy in (-1, 1):
            cyl("bolt", 0.021, 0.024, (sx * bx, sy * by, 0.086), m["light"],
                bevel=0.006)
    if hazard:
        hw = w - 0.22
        box("hazbed", (hw, 0.070, 0.018), (0.0, -by + 0.010, 0.083), m["recess"])
        n = int(hw / 0.060) + 1
        for i in range(n):
            x = -(n - 1) * 0.030 + i * 0.060
            ch = box("haz", (0.024, 0.062, 0.010), (x, -by + 0.010, 0.094), m["gold"])
            ch.rotation_euler = (0.0, 0.0, -0.7854)


def frame(scene, res, cells_w=1, cells_h=1, overhang=0.0, protrude=1.0):
    """Camera + resolution for a module's real drawn size.

    spawner.rs draws a module at exactly `cells * 66` world units per axis --
    the full grid cell, so blocks touch. `overhang` lengthens the vertical
    axis and `protrude` says which end it hangs off: +1 art-top (gun barrels),
    -1 art-bottom (engine nozzles). One frame unit = 66 world units = 1 cell.
    """
    cw = 66.0 * cells_w
    ch = 66.0 * cells_h + overhang
    span_w, span_h = cw / 66.0, ch / 66.0
    cam = ortho_camera(scene, max(span_w, span_h))
    # Keep the housing centred on its cell; the overhang hangs off one end.
    cam.location = (0.0, protrude * (overhang / 132.0), 6.0)
    if span_w >= span_h:
        scene.render.resolution_x = res
        scene.render.resolution_y = int(round(res * span_h / span_w))
    else:
        scene.render.resolution_y = res
        scene.render.resolution_x = int(round(res * span_w / span_h))
    return cam


def glow_disc(name, radius, depth, location, stops=None, strength=5.5, verts=64):
    """A disc that is a light source with a smooth radial falloff.

    The material is built per disc because the gradient has to be scaled to
    that disc's own radius.
    """
    mat = radial_glow("glow_" + name, stops or ION_STOPS, strength, radius)
    return cyl(name, radius, depth, location, mat, verts=verts)


def argv_after_ddash():
    import sys
    return sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
