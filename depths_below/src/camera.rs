use bevy::prelude::*;
use bevy::input::mouse::MouseWheel;
use bevy::sprite::Anchor;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use crate::components::{Ship, ShipPhysics, ShipLight, Module, HullSegment};
use crate::resources::DepthState;
use crate::events::ShipDamaged;
use crate::states::GameState;

/// Marker for the main camera
#[derive(Component)]
pub struct MainCamera;

/// Where the camera looks instead of the ship. A filming aid only
/// (DEPTHS_CAM_AT in demo.rs); None in play.
#[derive(Resource, Default)]
pub struct CameraPin(pub Option<Vec2>);

/// Camera zoom and shake state
#[derive(Resource)]
pub struct CameraState {
    pub zoom: f32,
    pub min_zoom: f32,
    pub max_zoom: f32,
    pub shake_intensity: f32,
    pub shake_decay: f32,
    pub shake_offset: Vec2,
    /// Camera offset from the ship while free-look (hold T) is active.
    /// Eases toward a target each frame rather than snapping, and eases
    /// back to zero on release — see `free_look_input`.
    pub free_look_offset: Vec2,
    /// True while the free-look key is held. Read by `ship::movement` to
    /// freeze the mouse-aim turret so looking around doesn't spin the ship
    /// to face wherever the cursor happens to be.
    pub free_look_active: bool,
    /// Set the first time the player scrolls or presses +/-. Until then the
    /// camera frames the ship itself (`frame_ship_until_player_zooms`);
    /// after, the zoom is theirs and stays where they put it.
    pub user_zoomed: bool,
    /// While an enemy is fighting you in sensor range: the zoom that keeps
    /// it on screen, which the camera never sits closer than
    /// (`frame_the_fight`).
    pub fight_zoom: Option<f32>,
    /// Zoomed by hand during the current fight: the framing leaves the
    /// camera to you until the fight is over.
    pub fight_framing_off: bool,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            zoom: DEFAULT_ZOOM,
            min_zoom: 0.5,
            // Was 8.0 — at scale 8 on a 1280x720 window the visible area is
            // only ~10,240x5,760 world units, so an enemy holding an
            // 8,000-9,600 unit standoff (now that shots can actually reach
            // that far, see combat::PROJECTILE_SPEED) sat right at the
            // screen edge even fully zoomed out. 20.0 gives ~25,600x14,400
            // visible, comfortably framing a long-range fight.
            // A fight at 20 km, framed: fight_framing_zoom.
            max_zoom: 90.0,
            shake_intensity: 0.0,
            shake_decay: 5.0,
            shake_offset: Vec2::ZERO,
            free_look_offset: Vec2::ZERO,
            free_look_active: false,
            user_zoomed: false,
            fight_zoom: None,
            fight_framing_off: false,
        }
    }
}

/// The zoom a small ship opens at, and the floor under the fitted zoom.
const DEFAULT_ZOOM: f32 = 1.8;

/// Share of the view's height the ship's turning circle should take. At the
/// old fixed 1.8 the starter pincer filled 97% of a 720-high view, so in a
/// fight there was no room on screen for anything but your own hull.
const SHIP_FRAME_FRACTION: f32 = 0.5;

/// Half the diagonal of one grid cell: a block's far corner from its centre.
const CELL_HALF_DIAGONAL: f32 = 66.0 * std::f32::consts::FRAC_1_SQRT_2;

/// Zoom at which a ship of this radius takes `SHIP_FRAME_FRACTION` of a view
/// this many pixels tall. Never tighter than the old default, so small ships
/// look exactly as they used to.
pub fn framing_zoom(ship_radius: f32, view_height: f32) -> f32 {
    if view_height <= 0.0 || ship_radius <= 0.0 {
        return DEFAULT_ZOOM;
    }
    (2.0 * ship_radius / (SHIP_FRAME_FRACTION * view_height)).max(DEFAULT_ZOOM)
}

/// World-space text that should stay the size it was designed at on screen.
///
/// Text2d is sized in world units, so it shrinks as the camera pulls back.
/// Station names, damage numbers and bounty labels were all sized for the old
/// fixed zoom of 1.8; once the camera started framing the ship (out to ~5.6
/// for the starter pincer) they drew at a third of that -- a station's name
/// plate became a smudge. These scale up with the zoom instead, and never
/// below their designed size.
#[derive(Component)]
pub struct ZoomInvariantText;

/// Scale a zoom-invariant label should carry at this camera zoom.
pub fn label_scale(camera_zoom: f32) -> f32 {
    (camera_zoom / DEFAULT_ZOOM).max(1.0)
}

fn keep_world_labels_readable(
    camera: Query<&Projection, With<MainCamera>>,
    mut labels: Query<&mut Transform, (With<ZoomInvariantText>, Without<MainCamera>)>,
) {
    let Ok(Projection::Orthographic(projection)) = camera.single() else { return };
    let want = Vec3::splat(label_scale(projection.scale));
    for mut transform in &mut labels {
        if transform.scale != want {
            transform.scale = want;
        }
    }
}

/// Rounds are sized in world units too, so pulled back they shrank to a pixel
/// or two -- a wide fight showed ships breaking up with nothing flying between
/// them. Like the labels, a round grows as the camera pulls back: never drawn
/// shorter than this on screen, and never smaller than its own size. (The
/// round sprites are about three-quarters body, tracer the rest.) Was 14:
/// zoomed out, every round was a fat dash.
const MIN_ROUND_PX: f32 = 8.0;

fn keep_rounds_readable(
    camera: Query<&Projection, With<MainCamera>>,
    mut rounds: Query<
        (&mut Transform, &Sprite),
        (
            Or<(
                With<crate::components::Projectile>,
                With<crate::combat::new_projectiles::Projectile>,
                With<crate::combat::new_projectiles::MissileProjectile>,
            )>,
            Without<MainCamera>,
        ),
    >,
) {
    let Ok(Projection::Orthographic(projection)) = camera.single() else { return };
    for (mut transform, sprite) in &mut rounds {
        let length = sprite.custom_size.map_or(1.0, |s| s.x.max(1.0));
        let want = Vec3::splat(round_scale(projection.scale, length));
        if transform.scale != want {
            transform.scale = want;
        }
    }
}

/// Scale a round `length` world units long carries at this camera zoom.
fn round_scale(camera_zoom: f32, length: f32) -> f32 {
    (MIN_ROUND_PX * camera_zoom / length).max(1.0)
}

/// Camera can pan up to this far from the ship while free-looking.
const FREE_LOOK_MAX_PAN: f32 = 4500.0;
/// Lerp rate (per second) for easing the pan offset toward/away from target.
const FREE_LOOK_EASE: f32 = 6.0;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<CameraState>()
            .init_resource::<CameraPin>()
            .add_systems(Startup, spawn_camera)
            .add_systems(
                Update,
                (
                    camera_zoom_input,
                    frame_ship_until_player_zooms,
                    frame_the_fight,
                    camera_shake_on_damage,
                    camera_shake_on_kill,
                    camera_shake_update,
                    free_look_input,
                    camera_follow_ship,
                    keep_world_labels_readable,
                    keep_rounds_readable,
                    update_background_color,
                )
                    .chain()
                    // After the ship has moved this frame. Unordered, the
                    // camera ran first and drew the ship one frame of travel
                    // behind centre -- 27 units at 1,500 u/s, 95 at 5,500 --
                    // and that gap swung with every frame-time wobble, so at
                    // speed the view lurched forward and back a few pixels
                    // many times a second.
                    .after(crate::states::ShipSet::Movement)
                    .run_if(in_state(GameState::Exploring)
                        .or_else(in_state(GameState::StationDocked))),
            );
        // DEPTHS_CAM_TRACE=1: once a second, how far the camera sat from the
        // ship when each frame was drawn, and how much that wobbled. Dev
        // tooling for judder at speed; off unless asked for.
        if std::env::var("DEPTHS_CAM_TRACE").ok().as_deref() == Some("1") {
            app.add_systems(Last, trace_camera_lag);
        }
    }
}

/// Logs the spread of the camera-to-ship offset along the ship's travel, per
/// second. A camera that follows in step holds it near zero; one that runs
/// before the ship moves on some frames and after it on others swings by a
/// frame's worth of travel -- the stutter you see at speed.
fn trace_camera_lag(
    time: Res<Time<Real>>,
    ship: Query<(&Transform, &crate::components::Velocity), With<Ship>>,
    camera: Query<&Transform, (With<MainCamera>, Without<Ship>)>,
    mut window: Local<(f32, f32, f32, f32, u32)>,
) {
    let (Ok((ship_tf, vel)), Ok(cam)) = (ship.single(), camera.single()) else { return };
    let speed = vel.0.length();
    let along = if speed > 1.0 {
        (cam.translation.truncate() - ship_tf.translation.truncate()).dot(vel.0 / speed)
    } else {
        0.0
    };
    let (elapsed, lo, hi, top_speed, frames) = &mut *window;
    if *frames == 0 {
        *lo = along;
        *hi = along;
    }
    *lo = lo.min(along);
    *hi = hi.max(along);
    *top_speed = top_speed.max(speed);
    *frames += 1;
    *elapsed += time.delta_secs();
    if *elapsed >= 1.0 {
        info!(
            "[CAM] speed {:.0} u/s, camera-ship offset along travel {:.1}..{:.1} (wobble {:.1} u) over {} frames",
            top_speed, lo, hi, *hi - *lo, frames
        );
        *window = (0.0, 0.0, 0.0, 0.0, 0);
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        MainCamera,
    ));
}

/// Scroll wheel + keyboard (+/-) zoom
fn camera_zoom_input(
    mut scroll_events: MessageReader<MouseWheel>,
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut camera_state: ResMut<CameraState>,
    // The build panel's item strip scrolls on the wheel too (see
    // build_ui::scroll_item_slots). Checked here rather than via a flag
    // resource so it can't depend on which system happens to run first.
    build_panel: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<crate::ui::build_ui::BuildPanelRoot>>,
    // Same for the scrolling lists in windows (diplomacy).
    scroll_lists: Query<(&ComputedNode, &bevy::ui::UiGlobalTransform), With<crate::ui::windows::framework::ScrollsOnWheel>>,
    windows: Query<&Window>,
    // The wheel moves the map while it's open (ui::map_scroll_system).
    map_open: Query<(), With<crate::ui::MapOverlay>>,
) {
    if !map_open.is_empty() {
        scroll_events.clear();
        return;
    }
    let cursor = windows.single().ok()
        .and_then(|w| w.cursor_position().map(|p| p * w.scale_factor()));
    let over_build_panel = build_panel.iter().any(|(node, transform)| {
        cursor
            .and_then(|c| node.normalize_point(*transform, c))
            .is_some_and(|n| n.x.abs() <= 0.5 && n.y.abs() <= 0.5)
    }) || crate::ui::windows::framework::cursor_over_scroll_list(&scroll_lists, cursor);

    for event in scroll_events.read() {
        if over_build_panel { continue; }
        camera_state.user_zoomed = true;
        if camera_state.fight_zoom.is_some() {
            camera_state.fight_framing_off = true;
        }
        // A wheel notch is one Line; a Mac trackpad or Magic Mouse sends
        // Pixels, 1-50 per event, and at 0.1 per unit one swipe slammed the
        // camera from one zoom limit to the other.
        let zoom_delta = match event.unit {
            bevy::input::mouse::MouseScrollUnit::Line => -event.y * 0.1,
            bevy::input::mouse::MouseScrollUnit::Pixel => -event.y * 0.004,
        };
        camera_state.zoom = (camera_state.zoom + zoom_delta)
            .clamp(camera_state.min_zoom, camera_state.max_zoom);
    }

    // Keyboard zoom: +/= zooms in, -/_ zooms out (plus numpad variants)
    let kb_speed = 1.5 * time.delta_secs();
    if keyboard.pressed(KeyCode::Equal) || keyboard.pressed(KeyCode::NumpadAdd) {
        camera_state.user_zoomed = true;
        camera_state.fight_framing_off |= camera_state.fight_zoom.is_some();
        camera_state.zoom = (camera_state.zoom - kb_speed)
            .clamp(camera_state.min_zoom, camera_state.max_zoom);
    }
    if keyboard.pressed(KeyCode::Minus) || keyboard.pressed(KeyCode::NumpadSubtract) {
        camera_state.user_zoomed = true;
        camera_state.fight_framing_off |= camera_state.fight_zoom.is_some();
        camera_state.zoom = (camera_state.zoom + kb_speed)
            .clamp(camera_state.min_zoom, camera_state.max_zoom);
    }
}

/// Zoom that keeps a point `reach` world units from the ship on screen, top
/// to bottom with a margin (the camera is centred on the ship).
pub fn fight_framing_zoom(reach: f32, view_height: f32) -> f32 {
    if view_height <= 0.0 {
        return DEFAULT_ZOOM;
    }
    // Far enough to keep it well clear of the HUD along the top: at 2.3 an
    // enemy sat half under it, too near the edge to see what you'd done to it.
    (2.9 * reach / view_height).max(DEFAULT_ZOOM)
}

/// Fights happen at around 20 km now. While an enemy is fighting you inside sensor
/// range, the camera stays out far enough to keep the nearest such ship, all
/// of it, on screen -- easing out and back in on its own. Zoom by hand
/// during the fight and the camera is yours until it's over.
fn frame_the_fight(
    mut camera_state: ResMut<CameraState>,
    ship: Query<(Entity, &Transform), With<Ship>>,
    enemies: Query<
        (&Transform, &crate::ai_ship::components::AiShipBehavior, &crate::ai_ship::components::AiShipTarget, &crate::ai_ship::components::AiShipState, Option<&crate::ship::collision::Collider>),
        (With<crate::ai_ship::components::AiShip>, Without<Ship>),
    >,
    windows: Query<&Window>,
) {
    let Ok((player, player_tf)) = ship.single() else { return };
    let at = player_tf.translation.truncate();
    let nearest = enemies
        .iter()
        .filter(|(_, behavior, target, state, _)| {
            **behavior == crate::ai_ship::components::AiShipBehavior::Engaging && target.entity == Some(player) && !state.is_destroyed
        })
        .map(|(tf, _, _, _, collider)| tf.translation.truncate().distance(at) + collider.map_or(0.0, |c| c.bound_radius))
        .filter(|reach| *reach <= SENSOR_RANGE)
        .fold(None, |a: Option<f32>, r| Some(a.map_or(r, |a| a.min(r))));
    let Some(reach) = nearest else {
        // The fight's over: the next one frames itself again.
        if camera_state.fight_zoom.is_some() || camera_state.fight_framing_off {
            camera_state.fight_zoom = None;
            camera_state.fight_framing_off = false;
        }
        return;
    };
    if camera_state.fight_framing_off {
        camera_state.fight_zoom = None;
        return;
    }
    let height = windows.iter().next().map_or(720.0, |w| w.height());
    let want = fight_framing_zoom(reach, height).min(camera_state.max_zoom);
    if camera_state.fight_zoom != Some(want) {
        camera_state.fight_zoom = Some(want);
    }
}

/// Pulls the camera back far enough to see past the ship, and keeps doing it
/// as the ship grows, until the player picks a zoom of their own.
fn frame_ship_until_player_zooms(
    mut camera_state: ResMut<CameraState>,
    ships: Query<&Children, With<Ship>>,
    blocks: Query<&Transform, Or<(With<Module>, With<HullSegment>)>>,
    windows: Query<&Window>,
) {
    if camera_state.user_zoomed {
        return;
    }
    let Ok(children) = ships.single() else { return };
    let radius = children
        .iter()
        .filter_map(|child| blocks.get(child).ok())
        .map(|t| t.translation.truncate().length() + CELL_HALF_DIAGONAL)
        .fold(0.0_f32, f32::max);
    let Some(window) = windows.iter().next() else { return };
    camera_state.zoom = framing_zoom(radius, window.height())
        .clamp(camera_state.min_zoom, camera_state.max_zoom);
}

/// Ceiling on accumulated hit shake. Low on purpose: this is a nudge that
/// tells you something connected, not a rattle you fight to aim through.
const MAX_HIT_SHAKE: f32 = 3.5;

/// Trigger shake on ship damage.
///
/// This was emptied out — it read the events and did nothing — because the
/// original `(amount * 0.3).min(15.0)` stacked toward a cap of 20 and, once
/// weapon damage reached 80, pegged near maximum on nearly every hit and
/// stayed there under fire. Deleting it fixed the shaking and lost the
/// feedback: being shot became something you could only read off the hull bar.
///
/// Restored an order of magnitude smaller and capped well below the old one,
/// so a hit registers in the frame and a sustained burst hums rather than
/// throws the camera around.
fn camera_shake_on_damage(
    mut damage_events: MessageReader<ShipDamaged>,
    mut camera_state: ResMut<CameraState>,
) {
    for event in damage_events.read() {
        let kick = (event.amount * 0.03).min(1.2);
        camera_state.shake_intensity = (camera_state.shake_intensity + kick).min(MAX_HIT_SHAKE);
    }
}

/// A kill is the payoff of a fight and deserves to land as an event.
///
/// Scaled by distance so a hulk coming apart across the system is a tremor and
/// one dying at knife range is a jolt, and by cause: a reactor letting go
/// should hit harder than a crew striking colours.
pub fn camera_shake_on_kill(
    mut destroyed: MessageReader<crate::events::AiShipDestroyed>,
    ship: Query<&Transform, With<Ship>>,
    mut camera_state: ResMut<CameraState>,
) {
    let Ok(ship_transform) = ship.single() else { return };
    let here = ship_transform.translation.truncate();
    for event in destroyed.read() {
        let base = match event.cause {
            crate::events::ShipDeathCause::Meltdown => 9.0, // containment lets go
            crate::events::ShipDeathCause::Gutted => 5.0,
            crate::events::ShipDeathCause::Struck => 2.5,   // she just stops fighting
        };
        // Full strength inside 1500 units, nothing past 9000.
        let d = here.distance(event.position);
        let falloff = (1.0 - ((d - 1500.0) / 7500.0)).clamp(0.0, 1.0);
        if falloff <= 0.0 { continue; }
        camera_state.shake_intensity = (camera_state.shake_intensity + base * falloff).min(14.0);
    }
}

/// Decay shake over time and compute offset
pub fn camera_shake_update(
    time: Res<Time>,
    mut camera_state: ResMut<CameraState>,
) {
    if camera_state.shake_intensity > 0.1 {
        let t = time.elapsed_secs();
        // Pseudo-random shake using sin waves at different frequencies
        camera_state.shake_offset = Vec2::new(
            (t * 37.0).sin() * camera_state.shake_intensity,
            (t * 53.0).cos() * camera_state.shake_intensity,
        );
        camera_state.shake_intensity -= camera_state.shake_decay * time.delta_secs();
        if camera_state.shake_intensity < 0.1 {
            camera_state.shake_intensity = 0.0;
            camera_state.shake_offset = Vec2::ZERO;
        }
    } else {
        camera_state.shake_offset = Vec2::ZERO;
    }
}

/// Hold T to look around without turning the ship: the camera pans toward
/// the cursor, up to FREE_LOOK_MAX_PAN away, and eases back to centered on
/// release. Ship mouse-aim is frozen for the duration (see ship::movement)
/// so the ship doesn't spin to face wherever you're just looking.
fn free_look_input(
    time: Res<Time>,
    keyboard: Res<ButtonInput<KeyCode>>,
    game_state: Res<State<GameState>>,
    mut camera_state: ResMut<CameraState>,
    ship_query: Query<&Transform, (With<Ship>, Without<MainCamera>)>,
    windows_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    // On Live the ship keeps flying behind the map, and the cursor is on the
    // map, not aiming.
    map_open: Query<(), With<crate::ui::MapOverlay>>,
) {
    // Exploring-only: while docked, T cycles build templates (see
    // building/templates.rs) — forcing the target to zero here (rather than
    // skipping the system outright) still lets any in-progress pan ease
    // back to centered instead of getting stuck if the player docks mid-hold.
    let held = *game_state.get() == GameState::Exploring && keyboard.pressed(KeyCode::KeyT);
    camera_state.free_look_active = held || !map_open.is_empty();

    let mut target_offset = Vec2::ZERO;
    if held {
        if let (Ok(ship_transform), Ok(window), Ok((camera, cam_gt))) =
            (ship_query.single(), windows_query.single(), camera_query.single())
        {
            if let Some(cursor) = window.cursor_position() {
                if let Ok(cursor_world) = camera.viewport_to_world_2d(cam_gt, cursor) {
                    let to_cursor = cursor_world - ship_transform.translation.truncate();
                    target_offset = to_cursor.clamp_length_max(FREE_LOOK_MAX_PAN);
                }
            }
        }
    }

    let blend = (FREE_LOOK_EASE * time.delta_secs()).min(1.0);
    camera_state.free_look_offset = camera_state.free_look_offset.lerp(target_offset, blend);
}

/// Smoothly follows the ship with the camera, applies zoom and shake
pub(crate) fn camera_follow_ship(
    time: Res<Time>,
    camera_state: Res<CameraState>,
    pin: Option<Res<CameraPin>>,
    ship_query: Query<&Transform, (With<Ship>, Without<MainCamera>)>,
    mut camera_query: Query<(&mut Transform, &mut Projection), (With<MainCamera>, Without<Ship>)>,
) {
    let Ok(ship_transform) = ship_query.single() else { return };
    let Ok((mut cam_transform, mut projection)) = camera_query.single_mut() else { return };
    let Projection::Orthographic(ref mut projection) = *projection else { return };

    // Target position with shake offset and free-look pan
    let look_at = pin.and_then(|p| p.0).unwrap_or(ship_transform.translation.truncate());
    let target = Vec3::new(
        look_at.x + camera_state.shake_offset.x + camera_state.free_look_offset.x,
        look_at.y + camera_state.shake_offset.y + camera_state.free_look_offset.y,
        cam_transform.translation.z,
    );

    // Snap camera to ship position (instant follow, no lag)
    cam_transform.translation = target;

    // Smooth zoom: never closer than a fight in progress needs.
    let target_scale = camera_state.fight_zoom.map_or(camera_state.zoom, |z| camera_state.zoom.max(z));
    projection.scale = projection.scale + (target_scale - projection.scale) * 5.0 * time.delta_secs();
}

/// Dynamic background based on celestial proximity.
/// Near stars: warm glow. Near black holes: deep red/dark. Open void: dark blue/black.
pub fn update_background_color(
    _depth_state: Res<DepthState>,
    ship_query: Query<&Transform, With<Ship>>,
    star_query: Query<(&Transform, &crate::celestial::components::Star, &crate::celestial::components::CelestialBody)>,
    bh_query: Query<(&Transform, &crate::celestial::components::BlackHole, &crate::celestial::components::CelestialBody)>,
    mut clear_color: ResMut<ClearColor>,
) {
    let ship_pos = ship_query
        .single()
        .map(|t| t.translation.truncate())
        .unwrap_or(Vec2::ZERO);

    // Base color: deep void (dark blue-black)
    let void_color = Vec3::new(0.01, 0.02, 0.06);

    // Star influence: warm glow when close
    let mut star_influence = Vec3::ZERO;
    let mut star_proximity = 0.0_f32;
    for (star_transform, star, body) in star_query.iter() {
        let dist = ship_pos.distance(star_transform.translation.truncate());
        let influence_range = body.radius * 4.0;
        if dist < influence_range {
            let t = 1.0 - (dist / influence_range).clamp(0.0, 1.0);
            let warmth = match star.size_class {
                crate::celestial::components::StarSizeClass::Dwarf => Vec3::new(0.15, 0.08, 0.03),
                crate::celestial::components::StarSizeClass::Main => Vec3::new(0.15, 0.12, 0.06),
                crate::celestial::components::StarSizeClass::Giant => Vec3::new(0.20, 0.10, 0.03),
                crate::celestial::components::StarSizeClass::Supergiant => Vec3::new(0.10, 0.12, 0.20),
            };
            star_influence += warmth * t * t;
            star_proximity = star_proximity.max(t);
        }
    }

    // Black hole influence: dark red pull
    let mut bh_influence = Vec3::ZERO;
    for (bh_transform, bh, _body) in bh_query.iter() {
        let dist = ship_pos.distance(bh_transform.translation.truncate());
        let influence_range = bh.accretion_disk_radius * 3.0;
        if dist < influence_range {
            let t = 1.0 - (dist / influence_range).clamp(0.0, 1.0);
            bh_influence += Vec3::new(0.08 * t, 0.0, 0.02 * t); // Dark red
            // Also darken overall as black hole sucks light
            bh_influence -= Vec3::splat(0.03 * t);
        }
    }

    let c = void_color + star_influence + bh_influence;
    let c = c.clamp(Vec3::ZERO, Vec3::ONE);
    clear_color.0 = Color::srgb(c.x, c.y, c.z);
}

/// How far the ship's sensors hold a ship as a contact in a fight, which is
/// how far out the camera will frame one.
pub const SENSOR_RANGE: f32 = 42_000.0;

#[cfg(test)]
mod framing_tests {
    use super::*;

    /// A ship whose blocks reach `reach` units out along +x, on a 1280x720 window.
    fn app_with_ship(reach: f32) -> App {
        let mut app = App::new();
        app.init_resource::<CameraState>();
        app.add_systems(Update, frame_ship_until_player_zooms);
        app.world_mut().spawn(Window::default());
        let ship = app.world_mut().spawn((Ship, Transform::default())).id();
        for x in [-reach, 0.0, reach] {
            let block = app.world_mut().spawn((HullSegment::default(), Transform::from_xyz(x, 0.0, 0.0))).id();
            app.world_mut().entity_mut(ship).add_child(block);
        }
        app
    }

    /// The starter pincer reaches roughly 1,000 units from its centre. At the
    /// old fixed zoom it filled the view; framed, its turning circle takes
    /// half the height and the rest is room to see what is shooting at you.
    #[test]
    fn a_big_ship_is_framed_with_room_around_it() {
        let mut app = app_with_ship(1000.0);
        app.update();
        let zoom = app.world().resource::<CameraState>().zoom;
        let radius = 1000.0 + CELL_HALF_DIAGONAL;
        let share = 2.0 * radius / (720.0 * zoom);
        assert!((share - SHIP_FRAME_FRACTION).abs() < 0.01, "ship takes {share:.2} of the view at zoom {zoom}");
    }

    /// Labels hold their designed size from the default zoom outward, and
    /// are never shrunk when the player zooms in close.
    #[test]
    fn labels_scale_with_the_zoom_but_not_below_design_size() {
        assert_eq!(label_scale(DEFAULT_ZOOM), 1.0);
        assert!((label_scale(DEFAULT_ZOOM * 3.0) - 3.0).abs() < 1e-5);
        assert_eq!(label_scale(0.8), 1.0);
    }

    /// A round is never drawn shorter than MIN_ROUND_PX on screen, and never
    /// smaller than its own size when the camera is close.
    #[test]
    fn rounds_stay_readable_when_zoomed_out() {
        assert_eq!(round_scale(1.0, 20.0), 1.0);
        let s = round_scale(12.0, 20.0);
        assert!((20.0 * s / 12.0 - MIN_ROUND_PX).abs() < 1e-4, "{s}");
        // a big shell already long enough on screen is left alone
        assert_eq!(round_scale(2.0, 38.0), 1.0);
    }

    #[test]
    fn a_small_ship_keeps_the_old_zoom() {
        let mut app = app_with_ship(132.0);
        app.update();
        assert_eq!(app.world().resource::<CameraState>().zoom, DEFAULT_ZOOM);
    }

    /// Once the player has chosen a zoom, the camera stops second-guessing it.
    #[test]
    fn the_players_own_zoom_is_left_alone() {
        let mut app = app_with_ship(1000.0);
        {
            let mut state = app.world_mut().resource_mut::<CameraState>();
            state.user_zoomed = true;
            state.zoom = 2.5;
        }
        app.update();
        assert_eq!(app.world().resource::<CameraState>().zoom, 2.5);
    }
}

#[cfg(test)]
mod fight_framing_tests {
    use super::*;

    #[test]
    fn a_fight_7_km_off_fits_on_screen() {
        let zoom = fight_framing_zoom(7000.0, 720.0);
        assert!(zoom * 720.0 / 2.0 > 7000.0, "half the view covers the enemy: {zoom}");
        assert!(zoom < 30.0);
    }
}
