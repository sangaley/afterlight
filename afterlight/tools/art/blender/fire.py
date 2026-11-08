"""Mantaflow fire -- fireball, motor flame, muzzle flash.

The companion to smoke.py, and it differs in one way that matters. Smoke is
shapeless, so its texture ships as pure white with the whole shape in alpha.
Fire's visual interest IS its blackbody gradient -- white-hot core, orange
shoulder, dark edge -- and Sprite.color multiplies, so shipping that gradient
fights whatever colour the call site picked.

The compromise is post.py's `whiten --keep-warm`: value-normalise so only hue
survives, then lerp most of the way to white. Enough warmth stays to read as
fire, not enough to fight a tint.

    Blender -b -P fire.py -- fireball tools/art/preview/fire/ball 256
    Blender -b -P fire.py -- muzzle   tools/art/preview/fire/mz  256

`muzzle` is a directional jet rather than a burst: a muzzle flash points
where the gun does, and the sprite is rotated to the firing angle in game.
"""

import glob
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import ortho_camera, configure, render_to, argv_after_ddash, new_scene  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
CACHE_ROOT = os.path.abspath(os.path.join(HERE, "..", "cache"))

DOMAIN_SIZE = 4.0


def _common_domain(kind, res, last):
    bpy.ops.mesh.primitive_cube_add(size=DOMAIN_SIZE, location=(0.0, 0.0, 0.0))
    dom = bpy.context.object
    dom.name = "domain"
    bpy.ops.object.modifier_add(type="FLUID")
    dom.modifiers["Fluid"].fluid_type = "DOMAIN"
    ds = dom.modifiers["Fluid"].domain_settings
    ds.domain_type = "GAS"
    ds.resolution_max = res
    ds.use_adaptive_domain = False
    ds.use_noise = True
    ds.noise_scale = 2
    ds.noise_strength = 1.4
    ds.vorticity = 0.35
    ds.alpha = 0.0
    ds.beta = 0.6
    # Fire needs fuel to burn and a temperature ceiling to map emission
    # against. burning_rate high = the flame is short and violent, which is
    # what a muzzle flash and a warhead both are.
    ds.burning_rate = 0.9
    ds.flame_smoke = 0.6
    ds.flame_vorticity = 0.6
    ds.flame_max_temp = 2.2
    ds.use_dissolve_smoke = True
    ds.dissolve_speed = 26
    ds.cache_directory = os.path.join(CACHE_ROOT, kind)
    ds.cache_type = "ALL"
    ds.cache_frame_start, ds.cache_frame_end = 1, last
    ds.cache_data_format = "OPENVDB"
    ds.cache_noise_format = "OPENVDB"
    return dom, ds


def build_fireball(kind, res, last):
    """A warhead: one violent spherical burst of burning fuel."""
    scene = new_scene()
    scene.gravity = (0.0, 0.0, 0.0)
    scene.frame_start, scene.frame_end = 1, last

    bpy.ops.mesh.primitive_ico_sphere_add(radius=0.42, subdivisions=2, location=(0, 0, 0))
    em = bpy.context.object
    em.name = "emitter"
    em.hide_render = True
    bpy.ops.object.modifier_add(type="FLUID")
    em.modifiers["Fluid"].fluid_type = "FLOW"
    fs = em.modifiers["Fluid"].flow_settings
    fs.flow_type = "BOTH"          # smoke AND fire
    fs.flow_behavior = "INFLOW"
    fs.smoke_color = (1.0, 1.0, 1.0)
    fs.density = 0.9
    fs.fuel_amount = 1.8
    fs.temperature = 2.0
    fs.surface_distance = 1.4
    fs.subframes = 2
    fs.use_initial_velocity = True     # defaults False; without it nothing moves
    fs.velocity_normal = 2.6
    fs.velocity_random = 2.2

    fs.use_inflow = True
    fs.keyframe_insert("use_inflow", frame=1)
    fs.use_inflow = True
    fs.keyframe_insert("use_inflow", frame=4)
    fs.use_inflow = False
    fs.keyframe_insert("use_inflow", frame=5)

    dom, ds = _common_domain(kind, res, last)
    return scene, dom, ds


def build_muzzle(kind, res, last):
    """A gun: a short directional jet down +X, not a burst.

    Rendered pointing along +X because the engine rotates the sprite to the
    firing angle, and every other sprite in this project is authored on that
    convention.
    """
    scene = new_scene()
    scene.gravity = (0.0, 0.0, 0.0)
    scene.frame_start, scene.frame_end = 1, last

    bpy.ops.mesh.primitive_cone_add(
        radius1=0.16, depth=0.34, location=(-0.9, 0.0, 0.0),
        rotation=(0.0, 1.5708, 0.0),          # nose down +X
    )
    em = bpy.context.object
    em.name = "emitter"
    em.hide_render = True
    bpy.ops.object.modifier_add(type="FLUID")
    em.modifiers["Fluid"].fluid_type = "FLOW"
    fs = em.modifiers["Fluid"].flow_settings
    fs.flow_type = "BOTH"
    fs.flow_behavior = "INFLOW"
    fs.smoke_color = (1.0, 1.0, 1.0)
    fs.density = 0.7
    fs.fuel_amount = 2.2
    fs.temperature = 2.2
    fs.surface_distance = 1.2
    fs.subframes = 2
    fs.use_initial_velocity = True
    # Mostly along the barrel, with just enough scatter to avoid a clean cone.
    fs.velocity_normal = 1.0
    fs.velocity_coord = (7.0, 0.0, 0.0)
    fs.velocity_random = 1.1

    fs.use_inflow = True
    fs.keyframe_insert("use_inflow", frame=1)
    fs.use_inflow = True
    fs.keyframe_insert("use_inflow", frame=3)
    fs.use_inflow = False
    fs.keyframe_insert("use_inflow", frame=4)

    dom, ds = _common_domain(kind, res, last)
    return scene, dom, ds


def flame_material(dom, density=5.0, emit=6.0):
    """Density for body, the `flame` attribute for light.

    Emission strength is driven by flame rather than the shader's own
    blackbody so the hot core lands in LUMINANCE, which is the channel
    post.py can fold into alpha. Blackbody would put it in hue, which is the
    one channel we have to give up.
    """
    mat = bpy.data.materials.new("flame")
    mat.use_nodes = True
    nt = mat.node_tree
    for n in list(nt.nodes):
        if n.type != "OUTPUT_MATERIAL":
            nt.nodes.remove(n)
    out = nt.nodes["Material Output"]

    vol = nt.nodes.new("ShaderNodeVolumePrincipled")
    vol.inputs["Color"].default_value = (1.0, 1.0, 1.0, 1.0)
    vol.inputs["Density"].default_value = density
    vol.inputs["Density Attribute"].default_value = "density"
    vol.inputs["Anisotropy"].default_value = 0.0
    vol.inputs["Emission Color"].default_value = (1.0, 0.72, 0.34, 1.0)
    # A little blackbody for the core-to-rim hue, kept low because whiten
    # will mostly take it out again; it is here so `--keep-warm` has
    # something real to preserve.
    vol.inputs["Blackbody Intensity"].default_value = 0.35
    vol.inputs["Temperature Attribute"].default_value = "temperature"

    attr = nt.nodes.new("ShaderNodeAttribute")
    attr.attribute_name = "flame"
    rng = nt.nodes.new("ShaderNodeMapRange")
    rng.inputs["From Min"].default_value = 0.0
    rng.inputs["From Max"].default_value = 1.0
    rng.inputs["To Min"].default_value = 0.0
    rng.inputs["To Max"].default_value = emit
    nt.links.new(attr.outputs["Fac"], rng.inputs["Value"])
    nt.links.new(rng.outputs["Result"], vol.inputs["Emission Strength"])
    nt.links.new(vol.outputs["Volume"], out.inputs["Volume"])

    dom.data.materials.clear()
    dom.data.materials.append(mat)
    return mat


def light_volume(scene):
    """Isotropic ambient. lib.light_rig's area lamp would bake a directional
    gradient across the volume, and the engine free-rotates these sprites."""
    world = bpy.data.worlds.new("w")
    scene.world = world
    if world.node_tree is None:
        world.use_nodes = True
    bg = world.node_tree.nodes["Background"]
    bg.inputs["Color"].default_value = (1.0, 1.0, 1.0, 1.0)
    bg.inputs["Strength"].default_value = 0.7


def bake(scene, dom, ds, last):
    bpy.context.view_layer.objects.active = dom
    dom.select_set(True)
    with bpy.context.temp_override(scene=scene, object=dom,
                                   active_object=dom, selected_objects=[dom]):
        res = bpy.ops.fluid.bake_all()
    print("bake_all ->", res)
    n = len(glob.glob(os.path.join(ds.cache_directory, "data", "*.vdb")))
    # An unbaked domain renders a clean, fully transparent PNG with no error,
    # so this has to assert rather than trust the operator's return value.
    if n < last - 2:
        raise SystemExit("BAKE FAILED: %d vdb frames in %s" % (n, ds.cache_directory))
    print("cache OK: %d vdb frames" % n)


def blend_path(kind):
    return os.path.join(CACHE_ROOT, "fire_%s.blend" % kind)


def save_blend(kind):
    """Snapshot the baked scene so look iterations skip the sim.

    Rebuilding the domain resets the cache directory, so re-rendering from a
    bare VDB cache silently produces empty frames. A saved .blend keeps the
    modifier pointed at what it already wrote.
    """
    bpy.ops.wm.save_as_mainfile(filepath=blend_path(kind))
    print("SAVED", blend_path(kind))


def load_blend(kind):
    p = blend_path(kind)
    if not os.path.exists(p):
        raise SystemExit("no baked blend at %s -- run once without --from-blend" % p)
    bpy.ops.wm.open_mainfile(filepath=p)
    print("LOADED", p)
    return bpy.context.scene, bpy.data.objects["domain"]


BUILDERS = {"fireball": build_fireball, "muzzle": build_muzzle}


def main():
    args = argv_after_ddash()
    flags = [a for a in args if a.startswith("--")]
    pos = [a for a in args if not a.startswith("--")]

    kind = pos[0] if pos else "fireball"
    out = pos[1] if len(pos) > 1 else "/tmp/%s" % kind
    res = int(pos[2]) if len(pos) > 2 else 256

    span = 1.7
    camx = None
    density = 5.0
    emit = 6.0
    simres = 112
    last = 30
    frames = [4, 6, 8, 10, 12, 14, 16, 20]
    for a in flags:
        if a.startswith("--span="):
            span = float(a.split("=", 1)[1])
        if a.startswith("--camx="):
            camx = float(a.split("=", 1)[1])
        if a.startswith("--density="):
            density = float(a.split("=", 1)[1])
        if a.startswith("--emit="):
            emit = float(a.split("=", 1)[1])
        if a.startswith("--simres="):
            simres = int(a.split("=", 1)[1])
        if a.startswith("--frames="):
            frames = [int(x) for x in a.split("=", 1)[1].split(",")]
            last = max(last, max(frames))

    os.makedirs(os.path.dirname(os.path.abspath(out)) or ".", exist_ok=True)
    os.makedirs(CACHE_ROOT, exist_ok=True)

    if "--from-blend" in flags:
        scene, dom = load_blend(kind)
    else:
        scene, dom, ds = BUILDERS[kind](kind, simres, last)
        bake(scene, dom, ds, last)
        save_blend(kind)

    flame_material(dom, density=density, emit=emit)
    light_volume(scene)
    cam = ortho_camera(scene, span)
    if camx is not None:
        # The muzzle jet grows down +X from an emitter behind the origin, so
        # the frame has to include the emitter or the flash is cut off square
        # at the breech end -- which reads as a rectangle, not a flash.
        cam.location = (camx, 0.0, 6.0)
    configure(scene, res, samples=200)
    scene.cycles.volume_step_rate = 0.25
    scene.cycles.volume_max_steps = 1024
    if "--cpu" in flags:
        scene.cycles.device = "CPU"

    for f in frames:
        scene.frame_set(f)
        render_to(scene, "%s_f%02d.png" % (out, f))
    print("DONE", len(frames), "frames ->", out)


if __name__ == "__main__":
    main()
