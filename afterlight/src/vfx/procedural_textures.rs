use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};

// ============================================================================
// PROCEDURAL CIRCLE TEXTURES
// Celestial bodies (stars, planets, asteroids) and their glow/atmosphere
// layers were plain `Sprite { custom_size, .. }` with no image — Bevy draws
// an untextured Sprite as a flat rectangle, so every "circular" body was
// actually rendering as a solid-color square. Two small textures generated
// once at startup (a hard-edged disc and a soft radial falloff) fix that for
// every existing spawn site with just an `image: Some(handle)` swap — no new
// art assets, no shaders.
// ============================================================================

const TEX_SIZE: u32 = 128;

#[derive(Resource, Clone)]
pub struct CelestialTextures {
    /// Opaque disc, white RGB (tint via Sprite.color) — solid bodies: star
    /// core, planet, asteroid.
    pub solid: Handle<Image>,
    /// Soft radial gradient, white RGB, alpha 1.0 at center fading to 0 at
    /// the edge — glow/corona/atmosphere/shadow layers.
    pub glow: Handle<Image>,
    /// Flat annulus, white RGB, hollow in the middle — planetary rings. Drawn
    /// as a wide thin ellipse by squashing it on Y.
    pub ring: Handle<Image>,
}

fn circle_image(soft: bool) -> Image {
    let size = TEX_SIZE;
    let mut data = vec![0u8; (size * size * 4) as usize];
    let center = size as f32 / 2.0;
    let radius = center;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let t = (dx * dx + dy * dy).sqrt() / radius;

            let alpha = if soft {
                (1.0 - t.min(1.0)).powf(1.6)
            } else {
                // Opaque disc with a ~1.5px anti-aliased edge so it doesn't
                // look jagged at small sizes.
                let edge = (1.5 / radius).max(0.001);
                if t <= 1.0 - edge { 1.0 } else { ((1.0 - t) / edge).clamp(0.0, 1.0) }
            };

            let idx = ((y * size + x) * 4) as usize;
            data[idx] = 255;
            data[idx + 1] = 255;
            data[idx + 2] = 255;
            data[idx + 3] = (alpha * 255.0).round() as u8;
        }
    }

    Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// A flat ring: opaque between `INNER` and the rim, clear inside, with both
/// edges feathered and a couple of darker lanes so it doesn't read as a
/// drawn-on hoop.
fn ring_image() -> Image {
    const INNER: f32 = 0.55;
    let size = TEX_SIZE;
    let mut data = vec![0u8; (size * size * 4) as usize];
    let center = size as f32 / 2.0;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let t = (dx * dx + dy * dy).sqrt() / center;

            // Feather both edges over the same width, so the band has no hard
            // line on either side at the sizes these are drawn at.
            let feather = 0.06;
            let outer_fade = ((1.0 - t) / feather).clamp(0.0, 1.0);
            let inner_fade = ((t - INNER) / feather).clamp(0.0, 1.0);
            let mut alpha = outer_fade.min(inner_fade);

            // Two gaps, placed across the band's own width rather than at
            // fixed radii, so they stay put if INNER is retuned.
            let across = ((t - INNER) / (1.0 - INNER)).clamp(0.0, 1.0);
            for gap in [0.32f32, 0.68] {
                let d = (across - gap).abs();
                if d < 0.07 {
                    alpha *= 0.35 + 0.65 * (d / 0.07);
                }
            }

            let idx = ((y * size + x) * 4) as usize;
            data[idx] = 255;
            data[idx + 1] = 255;
            data[idx + 2] = 255;
            data[idx + 3] = (alpha * 255.0).round() as u8;
        }
    }

    Image::new(
        Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

pub fn generate_celestial_textures(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
) {
    let solid = images.add(circle_image(false));
    let glow = images.add(circle_image(true));
    let ring = images.add(ring_image());
    commands.insert_resource(CelestialTextures { solid, glow, ring });
}
