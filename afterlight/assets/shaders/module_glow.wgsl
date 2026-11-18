// A module's light halo, added to what's under it (vfx::module_glow): light
// that brightens the deck rather than paint laid over it. It breathes slowly
// on its own phase.
#import bevy_sprite::{
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::{view, globals},
}
#ifdef TONEMAP_IN_SHADER
#import bevy_core_pipeline::tonemapping
#endif

struct Glow {
    // Linear colour times strength.
    colour: vec4<f32>,
    // x: phase, y: breath depth, z: breath speed.
    breath: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> glow: Glow;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var mask: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var mask_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let m = textureSample(mask, mask_sampler, mesh.uv).a;
    let breath = 1.0 - glow.breath.y * (0.5 + 0.5 * sin(globals.time * glow.breath.z + glow.breath.x));
    var c = vec4(glow.colour.rgb * m * breath, 1.0);
#ifdef TONEMAP_IN_SHADER
    c = tonemapping::tone_mapping(c, view.color_grading);
#endif
    // Additive: the blend adds rgb and leaves the alpha under it alone.
    return vec4(c.rgb, 0.0);
}
