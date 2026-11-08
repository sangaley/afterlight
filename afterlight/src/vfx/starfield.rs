use bevy::prelude::*;
use rand::Rng;
use crate::camera::MainCamera;

// ============================================================================
// PARALLAX STARFIELD
// Three layers of stars wrap around the camera in a large box. Each layer
// follows the camera by a different fraction (parallax factor) — distant
// stars track the camera closely (appear far away, drift slowly), near
// stars lag behind (rush past) — giving a strong sense of motion in what
// is otherwise a featureless void.
//
// The box grows with the zoom and the stars with it, so the sky looks the same
// however far out you are: they are a very long way off, and pulling the
// camera back doesn't bring them any closer together.
// ============================================================================

/// One star in the wrapping starfield. `parallax` is how much of the camera's
/// movement the star inherits: 0.9 = distant (barely moves past), 0.5 = close.
#[derive(Component)]
pub struct StarfieldStar {
    pub parallax: f32,
    /// Where it sits in the box, from the camera, at the box's own size.
    pub offset: Vec2,
}

/// Marks the sparse decorative background planets (as opposed to star
/// sparkles, which also carry `StarfieldStar` for the shared parallax/wrap
/// system).
#[derive(Component)]
pub struct BackgroundPlanet;

/// Wrap box around the camera at normal zoom (`field_scale` grows it from
/// there). Sized so stars never pop at the edges, at a density that gives
/// about 35 visible stars.
const FIELD_W: f32 = 16_000.0;
const FIELD_H: f32 = 9_000.0;

/// The zoom the box's size and the stars' sizes are drawn for.
const FIELD_ZOOM: f32 = 1.8;

/// How much bigger the box and its stars are at this zoom. Never smaller:
/// zoomed right in, the stars just stay put and look larger.
pub fn field_scale(zoom: f32) -> f32 {
    (zoom / FIELD_ZOOM).max(1.0)
}

/// `offset` after the camera moved `delta` world units, wrapped into the box.
/// The camera carries a star `parallax` of the way along with it, and the rest
/// shows as drift, less of it the further out the camera is.
pub fn drift(offset: Vec2, delta: Vec2, parallax: f32, scale: f32) -> Vec2 {
    let moved = offset - delta * (1.0 - parallax) / scale;
    Vec2::new(
        (moved.x + FIELD_W / 2.0).rem_euclid(FIELD_W) - FIELD_W / 2.0,
        (moved.y + FIELD_H / 2.0).rem_euclid(FIELD_H) - FIELD_H / 2.0,
    )
}

/// Real 4-point sparkle sprites (Kenney "Space Shooter Redux", CC0 — see
/// assets/sprites/celestial/CREDITS.txt) instead of plain colored square
/// dots — mostly matters for the nearest/biggest layer where a shape is
/// actually visible at a few pixels across.
const SPARKLE_SPRITES: [&str; 3] = [
    "sprites/celestial/starfield/star1.png",
    "sprites/celestial/starfield/star2.png",
    "sprites/celestial/starfield/star3.png",
];

/// Same planet art used for real in-system planets (see
/// celestial::spawning::planet_sprite_path) — reused here as small, distant
/// background scenery. A handful scattered far apart, well behind the
/// stars, moving almost not at all (parallax 0.97) so they read as fixed
/// backdrop rather than something to fly to.
const BACKGROUND_PLANET_SPRITES: [&str; 6] = [
    "sprites/celestial/planets/planet00.png",
    "sprites/celestial/planets/planet02.png",
    "sprites/celestial/planets/planet04.png",
    "sprites/celestial/planets/planet06.png",
    "sprites/celestial/planets/planet08.png",
    "sprites/celestial/planets/planet09.png",
];

/// Spawn the starfield once, centered on the camera (guarded by query).
pub fn spawn_starfield(
    mut commands: Commands,
    existing: Query<(), With<StarfieldStar>>,
    camera_query: Query<&Transform, With<MainCamera>>,
    asset_server: Res<AssetServer>,
) {
    if !existing.is_empty() {
        return;
    }
    let Ok(cam) = camera_query.single() else { return };
    let center = cam.translation.truncate();
    let mut rng = rand::thread_rng();

    let sparkle_handles: Vec<Handle<Image>> = SPARKLE_SPRITES.iter()
        .map(|path| asset_server.load(*path))
        .collect();
    let bg_planet_handles: Vec<Handle<Image>> = BACKGROUND_PLANET_SPRITES.iter()
        .map(|path| asset_server.load(*path))
        .collect();

    // (count, parallax, size range, alpha range, tint)
    // Counts scaled 6x to match the OLD density (280 stars over 6000x4000 =
    // 24,000,000 sq units) applied to the new 16,000x9,000 = 144,000,000 sq
    // unit field (exactly 6x the area) — matching density instead of
    // undershooting it like the previous attempt did. 4th layer is a sparse
    // handful of much bigger, brighter stars so the field isn't uniformly
    // tiny — real size variety instead of everything topping out at 4.5px.
    let layers: [(usize, f32, (f32, f32), (f32, f32), Color); 4] = [
        (840, 0.92, (1.0, 2.2), (0.25, 0.45), Color::srgb(0.85, 0.88, 1.0)),
        (540, 0.78, (1.8, 3.2), (0.40, 0.65), Color::srgb(0.95, 0.95, 1.0)),
        (300, 0.58, (2.6, 4.5), (0.60, 0.95), Color::srgb(1.0, 1.0, 1.0)),
        (35,  0.40, (6.0, 11.0), (0.75, 1.0), Color::srgb(1.0, 0.97, 0.9)),
    ];

    for (count, parallax, size, alpha, tint) in layers {
        for _ in 0..count {
            let offset = Vec2::new(
                rng.gen_range(-FIELD_W / 2.0..FIELD_W / 2.0),
                rng.gen_range(-FIELD_H / 2.0..FIELD_H / 2.0),
            );
            let pos = center + offset;
            let s = rng.gen_range(size.0..size.1);
            let a = rng.gen_range(alpha.0..alpha.1);
            let c = tint.to_srgba();
            let image = sparkle_handles[rng.gen_range(0..sparkle_handles.len())].clone();
            commands.spawn((
                Sprite {
                    image,
                    color: Color::srgba(c.red, c.green, c.blue, a),
                    custom_size: Some(Vec2::splat(s)),
                    ..default()
                },
                Transform::from_xyz(pos.x, pos.y, -500.0),
                StarfieldStar { parallax, offset },
            ));
        }
    }

    // Distant background planets — sparse, small, mostly-opaque, barely
    // moving. Same art as real planets, just decorative scenery here rather
    // than an orbiting CelestialBody.
    let bg_planet_count = 5;
    for _ in 0..bg_planet_count {
        let offset = Vec2::new(
            rng.gen_range(-FIELD_W / 2.0..FIELD_W / 2.0),
            rng.gen_range(-FIELD_H / 2.0..FIELD_H / 2.0),
        );
        let pos = center + offset;
        let s = rng.gen_range(150.0..380.0);
        let image = bg_planet_handles[rng.gen_range(0..bg_planet_handles.len())].clone();
        commands.spawn((
            Sprite {
                image,
                custom_size: Some(Vec2::splat(s)),
                ..default()
            },
            Transform::from_xyz(pos.x, pos.y, -600.0),
            StarfieldStar { parallax: 0.97, offset },
            BackgroundPlanet,
        ));
    }
}

/// Apply per-layer parallax, wrap stars that drift out of the field box, and
/// keep the box and the stars in scale with the zoom.
pub fn update_starfield(
    camera_query: Query<(&Transform, &Projection), (With<MainCamera>, Without<StarfieldStar>)>,
    mut star_query: Query<(&mut StarfieldStar, &mut Transform), Without<MainCamera>>,
    mut last: Local<Option<(Vec2, f32)>>,
) {
    let Ok((cam, projection)) = camera_query.single() else { return };
    let Projection::Orthographic(ortho) = projection else { return };
    let cam_pos = cam.translation.truncate();
    let scale = field_scale(ortho.scale);

    let (prev_pos, prev_scale) = last.unwrap_or((cam_pos, scale));
    *last = Some((cam_pos, scale));
    let delta = cam_pos - prev_pos;
    if delta == Vec2::ZERO && scale == prev_scale && !star_query.iter().any(|(_, t)| t.scale.x != scale) {
        return;
    }

    for (mut star, mut transform) in star_query.iter_mut() {
        // A modulo wrap, because the camera can jump hundreds of thousands of
        // units in a frame (warp) and a single correction wouldn't catch up.
        star.offset = drift(star.offset, delta, star.parallax, scale);
        let at = cam_pos + star.offset * scale;
        transform.translation.x = at.x;
        transform.translation.y = at.y;
        transform.scale = Vec3::new(scale, scale, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_box_covers_the_screen_at_every_zoom() {
        // A 1440p window at the widest zoom has to fall inside the box.
        for zoom in [0.5, 1.8, 8.0, 20.0, 90.0] {
            let (seen_w, seen_h) = (2560.0 * zoom, 1440.0 * zoom);
            let s = field_scale(zoom);
            assert!(FIELD_W * s >= seen_w, "too narrow at zoom {zoom}");
            assert!(FIELD_H * s >= seen_h, "too short at zoom {zoom}");
        }
    }

    #[test]
    fn zoomed_right_in_the_box_stays_its_size() {
        assert_eq!(field_scale(0.5), 1.0);
        assert_eq!(field_scale(FIELD_ZOOM), 1.0);
        assert!((field_scale(FIELD_ZOOM * 10.0) - 10.0).abs() < 1e-3);
    }

    #[test]
    fn near_stars_drift_past_faster_than_far_ones() {
        let delta = Vec2::new(100.0, 0.0);
        let near = drift(Vec2::ZERO, delta, 0.4, 1.0);
        let far = drift(Vec2::ZERO, delta, 0.92, 1.0);
        assert!(near.x < far.x && far.x < 0.0, "{near} {far}");
    }

    #[test]
    fn a_warp_leaves_every_star_in_the_box() {
        let wrapped = drift(Vec2::new(10.0, -20.0), Vec2::new(4_000_000.0, -900_000.0), 0.4, 1.0);
        assert!(wrapped.x.abs() <= FIELD_W / 2.0 && wrapped.y.abs() <= FIELD_H / 2.0, "{wrapped}");
    }
}
