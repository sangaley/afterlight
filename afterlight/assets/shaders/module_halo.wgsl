// The modules' light halos, added to what's under them (vfx::module_glow):
// light that brightens the deck rather than paint laid over it. Every halo
// shares this one material and one unit quad, so a ship's halos draw
// together; which mask, colour and breath phase each has rides on its
// MeshTag (art in bits 0-7, colour 8-15, phase 16-18).
#import bevy_sprite::{
    mesh2d_functions as mesh_functions,
    mesh2d_view_bindings::{view, globals},
}
#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping
#endif

struct Halo {
    // Each art's place in the packed masks: uv offset xy, size zw.
    rects: array<vec4<f32>, 128>,
    // Linear colour times strength.
    colours: array<vec4<f32>, 64>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> halo: Halo;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var mask: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var mask_sampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) light: vec4<f32>,
};

@vertex
fn vertex(v: Vertex) -> Out {
    let tag = mesh_functions::get_tag(v.instance_index);
    let rect = halo.rects[tag & 0xffu];
    let colour = halo.colours[(tag >> 8u) & 0xffu];
    // Breathes slowly, on one of eight phases.
    let phase = f32((tag >> 16u) & 7u) * 0.785;
    let breath = 1.0 - 0.22 * (0.5 + 0.5 * sin(globals.time * 1.3 + phase));
    var out: Out;
    let world = mesh_functions::mesh2d_position_local_to_world(
        mesh_functions::get_world_from_local(v.instance_index),
        vec4<f32>(v.position, 1.0),
    );
    out.position = mesh_functions::mesh2d_position_world_to_clip(world);
    out.uv = rect.xy + v.uv * rect.zw;
    out.light = vec4(colour.rgb * breath, 1.0);
    return out;
}

@fragment
fn fragment(in: Out) -> @location(0) vec4<f32> {
    let m = textureSample(mask, mask_sampler, in.uv).a;
    var c = vec4(in.light.rgb * m, 1.0);
#ifdef TONEMAP_IN_SHADER
    c = tonemapping::tone_mapping(c, view.color_grading);
#endif
    // Additive: the blend adds rgb and leaves the alpha under it alone.
    return vec4(c.rgb, 0.0);
}
