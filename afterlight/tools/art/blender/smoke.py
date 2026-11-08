"""Mantaflow smoke sprites -- soft puffs for missile trails and explosions.

Every combat effect in the game is an untextured solid-colour quad. The
procedural animation behind them is fine; the sprites are hard-edged
rectangles. This bakes a real smoke sim and harvests stills from it, so the
puffs carry actual turbulence instead of a radial gradient.

Smoke only -- no fire this pass.

    # bake, then render a sweep of candidate frames
    Blender -b -P smoke.py -- puff tools/art/preview/effects/puff 256

    # retune the look against the bake you already have (seconds, not minutes)
    Blender -b -P smoke.py -- puff <out> 256 --from-blend --span=2.0

The bake is snapshotted to `tools/art/cache/<kind>.blend`, which is what
--from-blend reads. Reusing the bare VDB cache does NOT work: rebuilding the
domain resets the cache directory, and an unbaked domain renders a perfectly
clean, fully transparent PNG with no error at all -- so a naive "skip the
bake" flag fails silently. The saved .blend keeps the modifier pointed at the
VDBs it already wrote.

Output is `<out>_f##.png`. Run these through `post.py whiten` before looking
at them: Bevy's Sprite.color MULTIPLIES the texture and every call site
already sets a meaningful grey, so the shipped art has to be white RGB with
the shape carried entirely in alpha.
"""

import glob
import os
import sys

import bpy

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import (  # noqa: E402
    new_scene, ortho_camera, configure, render_to, argv_after_ddash,
)

HERE = os.path.dirname(os.path.abspath(__file__))
CACHE_ROOT = os.path.abspath(os.path.join(HERE, "..", "cache"))

# The sim runs 1..LAST; the usable window is "rolled but not yet dissipated".
LAST_FRAME = 60

# Domain is a 4.0 cube; the camera frames 4.6 so the soft edge never touches
# the canvas. A puff cut off square at the border shows a straight edge the
# moment the engine rotates the sprite.
DOMAIN_SIZE = 4.0
CAMERA_SPAN = 4.6


# ---------------------------------------------------------------- sim build
def build_sim(kind, quick=False):
    """Emitter first, domain second -- Mantaflow does not pick up a flow
    object that was added after the domain.

    `quick` drops resolution and drops noise to prove the bake path end to end
    in about a minute. The failure this guards against is silent (an unbaked
    domain renders a clean transparent PNG), so it is worth one cheap run
    before committing to the real one.
    """
    global LAST_FRAME
    if quick:
        LAST_FRAME = 24
    scene = new_scene()

    # No "up" in space. Gravity would give the plume a direction, and from
    # straight overhead -- the only view this game has -- a directional plume
    # reads as a lopsided smear rather than a puff.
    scene.gravity = (0.0, 0.0, 0.0)
    scene.frame_start, scene.frame_end = 1, LAST_FRAME

    # ---- emitter
    bpy.ops.mesh.primitive_ico_sphere_add(radius=0.50, subdivisions=2,
                                          location=(0.0, 0.0, 0.0))
    em = bpy.context.object
    em.name = "emitter"
    # The emitter is a real mesh and Cycles will happily render it: a solid
    # lit sphere sitting dead centre, completely hiding the smoke behind it.
    # It only needs to exist for the solver.
    em.hide_render = True
    bpy.ops.object.modifier_add(type="FLUID")
    em.modifiers["Fluid"].fluid_type = "FLOW"
    fs = em.modifiers["Fluid"].flow_settings
    fs.flow_type = "SMOKE"
    # INFLOW keyframed off after a few frames, NOT flow_behavior='GEOMETRY'.
    # GEOMETRY re-emits the mesh volume every frame, which leaves a hard
    # sphere sitting inside the puff for the whole sim.
    fs.flow_behavior = "INFLOW"
    fs.smoke_color = (1.0, 1.0, 1.0)
    fs.density = 1.0
    fs.temperature = 1.4
    fs.fuel_amount = 0.0
    fs.surface_distance = 1.5
    fs.subframes = 2
    # MUST be set, and defaults to False: without it velocity_normal and
    # velocity_random below are silently ignored. With no velocity, no
    # gravity and no buoyancy the smoke simply sits in the emitter volume for
    # the whole sim and renders as a static sphere -- which looks exactly
    # like a correct bake of a boring puff, so it is worth stating outright.
    fs.use_initial_velocity = True
    # Mantaflow's gas solver is incompressible: once inflow stops, the puff
    # does NOT keep expanding the way a real blast does -- it advects along
    # whatever velocity it was given and then sits. All the growth has to be
    # bought here, at the emitter, which is why this is much higher than the
    # value that looks reasonable in the UI.
    fs.velocity_normal = 1.8
    # Load-bearing. Without a random component the sim yields a perfectly
    # round ball -- exactly what a procedural radial gradient gives for free,
    # which would defeat the point of simulating anything.
    fs.velocity_random = 3.0

    # One-shot burst, not a jet. Held to frame 7 rather than 4 so there is
    # enough gas in the domain to still read once it has spread out.
    fs.use_inflow = True
    fs.keyframe_insert("use_inflow", frame=1)
    fs.use_inflow = True
    fs.keyframe_insert("use_inflow", frame=7)
    fs.use_inflow = False
    fs.keyframe_insert("use_inflow", frame=8)

    # ---- domain
    bpy.ops.mesh.primitive_cube_add(size=DOMAIN_SIZE, location=(0.0, 0.0, 0.0))
    dom = bpy.context.object
    dom.name = "domain"
    bpy.ops.object.modifier_add(type="FLUID")
    dom.modifiers["Fluid"].fluid_type = "DOMAIN"
    ds = dom.modifiers["Fluid"].domain_settings
    ds.domain_type = "GAS"
    ds.resolution_max = 48 if quick else 128
    # Adaptive bounds move and resize per frame. Against a fixed ortho camera
    # that makes the puff drift and change apparent scale between the frames
    # we harvest, so the three shipped variants would not sit at one size.
    ds.use_adaptive_domain = False
    ds.use_noise = not quick
    ds.noise_scale = 2
    ds.noise_strength = 1.6
    ds.noise_pos_scale = 1.6
    # Vorticity is what keeps the silhouette from being a sphere. From
    # directly overhead an explosion genuinely is roughly round, so the
    # interest has to come from internal structure -- wisps, holes,
    # filaments -- rather than from an irregular outline.
    ds.vorticity = 0.45
    ds.alpha = 0.0          # density buoyancy off -- zero g
    ds.beta = 0.4           # a little heat rise so it is not inert
    ds.use_dissolve_smoke = True
    ds.dissolve_speed = 42

    ds.cache_directory = os.path.join(CACHE_ROOT, kind)
    ds.cache_type = "ALL"
    ds.cache_frame_start, ds.cache_frame_end = 1, LAST_FRAME
    ds.cache_data_format = "OPENVDB"
    ds.cache_noise_format = "OPENVDB"

    return scene, dom, ds


# ---------------------------------------------------------------- bake
def cache_frames(ds):
    return len(glob.glob(os.path.join(ds.cache_directory, "data", "*.vdb")))


def bake(scene, dom, ds, replay=False):
    """Bake the sim, then PROVE it baked.

    An unbaked domain renders a perfectly clean, perfectly valid, fully
    transparent PNG. There is no error and no warning -- you just get blank
    files. So this asserts on the cache rather than trusting the operator's
    return value.
    """
    if replay:
        # Mantaflow steps the sim during depsgraph evaluation, so walking the
        # frames forward computes it with no job system involved. Must start
        # at frame 1 and must not skip.
        ds.cache_type = "REPLAY"
        print("REPLAY: stepping %d frames" % LAST_FRAME)
        for f in range(scene.frame_start, scene.frame_end + 1):
            scene.frame_set(f)
            bpy.context.view_layer.update()
            bpy.context.evaluated_depsgraph_get()
            if f % 10 == 0:
                print("  frame %d" % f)
        return

    bpy.context.view_layer.objects.active = dom
    dom.select_set(True)
    with bpy.context.temp_override(scene=scene, object=dom,
                                   active_object=dom, selected_objects=[dom]):
        res = bpy.ops.fluid.bake_all()
    print("bake_all ->", res)

    n = cache_frames(ds)
    if n < LAST_FRAME - 2:
        raise SystemExit(
            "BAKE FAILED: %d vdb files, expected ~%d in %s\n"
            "Re-run with --replay." % (n, LAST_FRAME, ds.cache_directory))
    print("cache OK: %d vdb frames" % n)


def blend_path(kind):
    return os.path.join(CACHE_ROOT, "%s.blend" % kind)


def save_blend(kind):
    """Snapshot the baked scene so look iterations skip the sim.

    Rebuilding the domain resets the cache directory, so there is no way to
    re-render from a bare cache -- but a saved .blend keeps the modifier
    pointing at the VDBs it already wrote. Bake once, then tune framing,
    density and frame choice for the cost of a render.
    """
    p = blend_path(kind)
    bpy.ops.wm.save_as_mainfile(filepath=p)
    print("SAVED", p)


def load_blend(kind):
    p = blend_path(kind)
    if not os.path.exists(p):
        raise SystemExit("no baked blend at %s -- run once without --from-blend" % p)
    bpy.ops.wm.open_mainfile(filepath=p)
    scene = bpy.context.scene
    dom = bpy.data.objects["domain"]
    print("LOADED", p)
    return scene, dom


# ---------------------------------------------------------------- material
def smoke_material(dom):
    """White isotropic smoke.

    Colour is white at the source, not just in post: the shipped texture is
    multiplied by whatever grey the spawn site picked, so any hue baked in
    here would fight it.
    """
    mat = bpy.data.materials.new("smoke")
    mat.use_nodes = True
    nt = mat.node_tree
    for n in list(nt.nodes):
        if n.type != "OUTPUT_MATERIAL":
            nt.nodes.remove(n)
    out = nt.nodes["Material Output"]

    vol = nt.nodes.new("ShaderNodeVolumePrincipled")
    vol.inputs["Color"].default_value = (1.0, 1.0, 1.0, 1.0)
    vol.inputs["Density"].default_value = 6.0
    # Isotropic. Forward scattering would make the puff's brightness depend on
    # view direction, which is a baked lighting cue -- and the engine rotates
    # these sprites freely.
    vol.inputs["Anisotropy"].default_value = 0.0
    vol.inputs["Emission Strength"].default_value = 0.0
    vol.inputs["Blackbody Intensity"].default_value = 0.0
    vol.inputs["Density Attribute"].default_value = "density"

    nt.links.new(vol.outputs["Volume"], out.inputs["Volume"])
    # Clear first: on the --from-blend path this would otherwise stack a new
    # slot every run, and only the first one renders.
    dom.data.materials.clear()
    dom.data.materials.append(mat)
    return mat


def light_volume(scene):
    """Isotropic ambient, no lamp.

    lib.light_rig puts a broad AREA light at z=7. On a solid module that is
    correct; on a volume it produces a soft DIRECTIONAL gradient across the
    puff, which is a baked directional shadow -- and ART_BRIEF forbids those
    precisely because the engine free-rotates sprites.
    """
    world = bpy.data.worlds.new("w")
    scene.world = world
    if world.node_tree is None:
        world.use_nodes = True
    bg = world.node_tree.nodes["Background"]
    bg.inputs["Color"].default_value = (1.0, 1.0, 1.0, 1.0)
    bg.inputs["Strength"].default_value = 1.6


# ---------------------------------------------------------------- render
def render_frames(scene, frames, out_prefix, res):
    for f in frames:
        scene.frame_set(f)
        render_to(scene, "%s_f%02d.png" % (out_prefix, f))


BUILDERS = {"puff": build_sim}


def main():
    args = argv_after_ddash()
    flags = [a for a in args if a.startswith("--")]
    pos = [a for a in args if not a.startswith("--")]

    kind = pos[0] if pos else "puff"
    out = pos[1] if len(pos) > 1 else "/tmp/%s" % kind
    res = int(pos[2]) if len(pos) > 2 else 256

    replay = "--replay" in flags
    cpu = "--cpu" in flags
    quick = "--quick" in flags
    if quick:
        kind = kind + "_quick"

    # How many world units the camera frames. The sim fills only a fraction
    # of the 4.0 domain, so the default span leaves the puff small and lost;
    # tune this down until it sits in frame with a little margin.
    span = CAMERA_SPAN
    for a in flags:
        if a.startswith("--span="):
            span = float(a.split("=", 1)[1])

    # The window worth looking at: rolled enough to have structure, not yet
    # eaten by dissolve. Narrowed to three shipped variants off the sheet.
    frames = [10, 18] if quick else [6, 10, 14, 18, 22, 26, 30, 34, 40]
    for a in flags:
        if a.startswith("--frames="):
            frames = [int(x) for x in a.split("=", 1)[1].split(",")]

    os.makedirs(os.path.dirname(os.path.abspath(out)) or ".", exist_ok=True)
    os.makedirs(CACHE_ROOT, exist_ok=True)

    if "--from-blend" in flags:
        scene, dom = load_blend(kind)
    else:
        scene, dom, ds = build_sim(kind, quick=quick)
        bake(scene, dom, ds, replay=replay)
        save_blend(kind)

    smoke_material(dom)
    light_volume(scene)
    ortho_camera(scene, span)
    configure(scene, res, samples=256)
    if cpu:
        scene.cycles.device = "CPU"
    # Volumes need fine stepping or the wisps resolve as banded mush, and more
    # samples than a solid because every ray marches.
    scene.cycles.volume_step_rate = 0.25
    scene.cycles.volume_max_steps = 1024

    render_frames(scene, frames, out, res)
    print("DONE", len(frames), "frames ->", out)


if __name__ == "__main__":
    main()
