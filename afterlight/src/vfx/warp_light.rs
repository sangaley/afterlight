//! The warp, seen from outside: the charge running through the ship's own
//! rooms, and the light of the ship left behind it -- and sent on ahead.
//!
//! 1. The charge. While the drive spools, a front of light spreads out from
//!    the reactors through the ship itself: cell by cell along the hallways
//!    and into the rooms, the way power would actually travel through this
//!    layout, until it reaches the engines and they flare. A tidy ship charges
//!    in a clean sweep; a sprawling one in a ragged one.
//! 2. The afterlight. Late in the charge the ship sheds faint copies of
//!    itself, light that hasn't caught up. When it jumps they're left where
//!    it was, each one fading slower than the one before.
//! 3. The light arrives first. Where it comes out, copies of the ship show up
//!    ahead of it and close in; only when they've come together is the ship
//!    there.
//!
//! 0. First the ship turns to face where it's going -- the way a ship lines
//!    up before it burns, turning up to speed and easing off -- so the
//!    copies and the light ahead all lie along the jump.
//!
//! Everything is kept faint: light over the ship, never a flash over the
//! screen. It reads either drive -- the jump between stars
//! (celestial::components::WarpCharging) and the dash within a system
//! (ui::MapWarpCharging) -- and knows the jump happened by the ship being
//! somewhere else the moment the charge is gone.

use bevy::asset::RenderAssetUsages;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::VecDeque;

use crate::building::{footprints, grid_to_local, ShipGrid, GRID_SIZE};
use crate::components::{DestroyedModule, HullDestroyed, HullSegment, Module, ModuleCategory, Ship};

/// Of the charge, how much the wave takes to reach the engines.
const WAVE_SHARE: f32 = 0.72;
/// Cells deep the bright front is.
const BAND: f32 = 3.5;
/// From this far into the charge the ship sheds copies of itself...
const SHED_FROM: f32 = 0.55;
/// ...one every so often.
const SHED_EVERY: f32 = 0.28;
const MOST_COPIES: usize = 7;
/// Seconds the light arriving ahead takes to close in.
const ARRIVE_S: f32 = 1.0;
/// Pixels a cell in the copies' picture.
const PX: u32 = 8;

/// One ship's charge, while it lasts.
pub struct Charge {
    /// Cells and how far each is from a reactor, in steps through the ship.
    pub dist: HashMap<IVec2, f32>,
    pub reach: f32,
    pub engines: HashSet<IVec2>,
    /// The ship's silhouette, for its copies, and where its middle sits.
    pub copy: Handle<Image>,
    pub copy_size: Vec2,
    pub copy_centre: Vec2,
    pub colour: Vec3,
    pub shed_at: f32,
    pub progress: f32,
}

#[derive(Resource, Default)]
pub struct WarpLight {
    pub charge: Option<Charge>,
    /// The ship's last place while charging, to tell a jump from a cancel.
    last: Option<Vec2>,
    /// Frames since the charge ended, waiting to see the ship move.
    ended: Option<u8>,
    /// Arriving: hidden until the light closes in.
    arriving: Option<f32>,
    /// The ship's turn rate while it lines up with the jump.
    omega: f32,
}

/// Lining up with a jump: how fast the turn can build, and how fast it goes.
const TURN_ACCEL: f32 = 1.6;
const TURN_MAX: f32 = 1.2;

/// Which way a charging warp is going, if one is.
fn jump_heading(
    from: Vec2,
    stars: Option<&crate::celestial::components::WarpCharging>,
    dash: Option<&crate::ui::MapWarpCharging>,
    galaxy: &crate::celestial::resources::GalaxyMap,
    streaming: &crate::celestial::resources::SystemStreamingManager,
) -> Option<f32> {
    let way = if let Some(dash) = dash {
        dash.target_pos - from
    } else {
        let to = crate::celestial::warp::target_galaxy_pos(galaxy, stars?.target)?;
        to - streaming.current_galaxy_pos
    };
    (way.length() > 1e-3).then(|| way.to_angle())
}

/// Turn `angle` toward `target` at a rate that builds and eases off.
pub fn line_up(angle: f32, omega: &mut f32, target: f32, dt: f32) -> f32 {
    let wrap = |a: f32| (a + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
    let diff = wrap(target - angle);
    if diff.abs() < 0.002 && omega.abs() < 0.03 {
        *omega = 0.0;
        return target;
    }
    let want = diff.signum() * (2.0 * TURN_ACCEL * diff.abs()).sqrt().min(TURN_MAX);
    *omega += (want - *omega).clamp(-TURN_ACCEL * dt, TURN_ACCEL * dt);
    let step = *omega * dt;
    if step.abs() > diff.abs() && step.signum() == diff.signum() {
        *omega = 0.0;
        return target;
    }
    wrap(angle + step)
}

/// While a warp charges, the ship turns to face where it's going. After
/// physics, so the helm doesn't turn it back.
#[allow(clippy::type_complexity)]
pub fn face_the_jump(
    time: Res<Time>,
    mut state: ResMut<WarpLight>,
    galaxy: Res<crate::celestial::resources::GalaxyMap>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    mut ship: Query<
        (&mut Transform, &mut crate::components::ShipPhysics, Option<&crate::celestial::components::WarpCharging>, Option<&crate::ui::MapWarpCharging>),
        With<Ship>,
    >,
) {
    let Ok((mut tf, mut physics, stars, dash)) = ship.single_mut() else { return };
    let Some(heading) = jump_heading(tf.translation.truncate(), stars, dash, &galaxy, &streaming) else {
        state.omega = 0.0;
        return;
    };
    let angle = line_up(physics.rotation, &mut state.omega, heading, time.delta_secs().min(0.1));
    physics.rotation = angle;
    physics.angular_velocity = 0.0;
    tf.rotation = Quat::from_rotation_z(angle);
}

/// A cell of the charge's light, over the ship.
#[derive(Component)]
pub struct ChargeCell {
    cell: IVec2,
}

/// A copy of the ship left in the light: world space, fading.
#[derive(Component)]
pub struct Afterimage {
    age: f32,
    /// Seconds it takes to fade once the ship has gone; None while charging.
    fade: Option<f32>,
    start_alpha: f32,
    drift: Vec2,
}

/// A copy arriving ahead of the ship.
#[derive(Component)]
pub struct Foreimage {
    from: Vec2,
    to: Vec2,
    delay: f32,
    age: f32,
}

/// Light shown for a charge fraction `p` at a cell `d` steps from a reactor,
/// on a ship whose farthest cell is `reach` steps out. 0..1.
pub fn cell_light(p: f32, d: f32, reach: f32, engine: bool) -> f32 {
    let wave = (p / WAVE_SHARE).clamp(0.0, 1.0);
    // Eased, so it creeps out of the reactors and runs once it's going.
    let front = (wave * wave * (3.0 - 2.0 * wave)) * (reach + BAND * 2.0) - BAND;
    let behind = front - d;
    if behind < -BAND {
        return 0.0;
    }
    // The bright front, and what it leaves charged behind it.
    let edge = (-(behind / BAND).powi(2)).exp() * 0.4;
    let charged = if behind > 0.0 { 0.05 + 0.05 * p } else { 0.0 };
    // The engines take it all at the end.
    let flare = if engine && behind > 0.0 { 0.25 + 0.25 * ((p - WAVE_SHARE) / (1.0 - WAVE_SHARE)).clamp(0.0, 1.0) } else { 0.0 };
    (edge + charged + flare).min(0.6)
}

/// Steps through the ship from the nearest reactor to every cell: the way
/// the charge travels. With no reactor, from the middle.
pub fn charge_distances(cells: &HashSet<IVec2>, sources: &[IVec2]) -> HashMap<IVec2, f32> {
    let mut dist: HashMap<IVec2, f32> = HashMap::default();
    let mut queue: VecDeque<IVec2> = VecDeque::new();
    let mut starts: Vec<IVec2> = sources.iter().copied().filter(|c| cells.contains(c)).collect();
    if starts.is_empty() {
        if let Some(mid) = middle(cells) {
            starts.push(mid);
        }
    }
    for s in starts {
        dist.insert(s, 0.0);
        queue.push_back(s);
    }
    while let Some(c) = queue.pop_front() {
        let d = dist[&c];
        for step in [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
            let n = c + step;
            if cells.contains(&n) && !dist.contains_key(&n) {
                dist.insert(n, d + 1.0);
                queue.push_back(n);
            }
        }
    }
    // Cells cut off from every reactor charge last.
    let far = dist.values().copied().fold(0.0, f32::max);
    for c in cells {
        dist.entry(*c).or_insert(far + 1.0);
    }
    dist
}

fn middle(cells: &HashSet<IVec2>) -> Option<IVec2> {
    let n = cells.len() as f32;
    let centre = cells.iter().fold(Vec2::ZERO, |a, c| a + c.as_vec2()) / n.max(1.0);
    cells.iter().min_by(|a, b| a.as_vec2().distance(centre).total_cmp(&b.as_vec2().distance(centre))).copied()
}

/// The ship as light: its silhouette, bright at the edge and faint inside.
fn silhouette(cells: &HashSet<IVec2>, images: &mut Assets<Image>) -> Option<(Handle<Image>, Vec2, Vec2)> {
    let (lo, hi) = cells.iter().fold((IVec2::MAX, IVec2::MIN), |(lo, hi), c| (lo.min(*c), hi.max(*c)));
    if lo.x > hi.x {
        return None;
    }
    let pad = 2;
    let (w, h) = (((hi.x - lo.x + 1 + pad * 2) as u32) * PX, ((hi.y - lo.y + 1 + pad * 2) as u32) * PX);
    let inside = |x: i32, y: i32| -> bool {
        // Image rows run top-down; grid y runs up.
        let cell = IVec2::new(lo.x - pad + x.div_euclid(PX as i32), hi.y + pad - y.div_euclid(PX as i32));
        cells.contains(&cell)
    };
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let filled = inside(x, y);
            // Near the outline: a ring a cell wide each side of it.
            let r = PX as i32;
            let mut near_edge = false;
            'look: for dy in [-r, 0, r] {
                for dx in [-r, 0, r] {
                    if inside(x + dx, y + dy) != filled {
                        near_edge = true;
                        break 'look;
                    }
                }
            }
            // Mostly its outline: light, not a solid ship.
            let a = match (filled, near_edge) {
                (true, true) => 1.0,
                (true, false) => 0.12,
                (false, true) => 0.25,
                (false, false) => 0.0,
            };
            px.extend([255, 255, 255, (a * 255.0) as u8]);
        }
    }
    let image = Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        px,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    let size = Vec2::new(w as f32, h as f32) / PX as f32 * GRID_SIZE;
    // The middle of the picture, ship-local.
    let centre = (grid_to_local(lo) + grid_to_local(hi)) * 0.5;
    Some((images.add(image), size, centre))
}

/// How far along a charge is, if one is running, from either drive.
fn charging(
    stars: Option<&crate::celestial::components::WarpCharging>,
    dash: Option<&crate::ui::MapWarpCharging>,
) -> Option<f32> {
    stars.map(|c| c.charge_timer.fraction()).or(dash.map(|c| c.charge_timer.fraction()))
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn warp_light(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<WarpLight>,
    mut images: ResMut<Assets<Image>>,
    mut ship: Query<
        (
            Entity,
            &Transform,
            &Children,
            &mut Visibility,
            Option<&crate::celestial::components::WarpCharging>,
            Option<&crate::ui::MapWarpCharging>,
            Option<&crate::vfx::ship_roof::ShipPaint>,
        ),
        With<Ship>,
    >,
    hulls: Query<&HullSegment, Without<HullDestroyed>>,
    modules: Query<&Module, Without<DestroyedModule>>,
    mut cells_lit: Query<(Entity, &ChargeCell, &mut Sprite), (Without<Afterimage>, Without<Foreimage>)>,
    mut after: Query<(Entity, &mut Afterimage, &mut Sprite, &mut Transform), (Without<Ship>, Without<Foreimage>, Without<ChargeCell>)>,
    mut fore: Query<(Entity, &mut Foreimage, &mut Sprite, &mut Transform), (Without<Ship>, Without<Afterimage>, Without<ChargeCell>)>,
) {
    let dt = time.delta_secs();
    let Ok((ship_e, tf, children, mut visibility, stars, dash, paint)) = ship.single_mut() else { return };
    let here = tf.translation.truncate();
    let progress = charging(stars, dash);

    // A charge begins: work out how it'll run through this ship.
    if progress.is_some() && state.charge.is_none() {
        let mut cells: HashSet<IVec2> = HashSet::default();
        let mut reactors = Vec::new();
        let mut engines = HashSet::default();
        for child in children.iter() {
            if let Ok(h) = hulls.get(child) {
                cells.insert(h.grid_position);
            } else if let Ok(m) = modules.get(child) {
                let footprint = ShipGrid::cells_for(m.grid_position, m.size, m.rotation, footprints::footprint_override(m.module_type));
                for c in footprint.iter() {
                    cells.insert(*c);
                }
                match m.module_type.category() {
                    ModuleCategory::Power if m.module_type.name().contains("Reactor") => reactors.extend(footprint.iter().copied()),
                    ModuleCategory::Propulsion => engines.extend(footprint.iter().copied()),
                    _ => {}
                }
            }
        }
        if let Some((copy, copy_size, copy_centre)) = silhouette(&cells, &mut images) {
            let dist = charge_distances(&cells, &reactors);
            let reach = dist.values().copied().fold(1.0, f32::max);
            // The ship's own trim, washed toward white: its light, not a stock blue.
            let trim = paint.cloned().unwrap_or_default().trim;
            let colour = Vec3::from_array(trim).lerp(Vec3::ONE, 0.55);
            for cell in &cells {
                commands.spawn((
                    Sprite { color: Color::srgba(colour.x, colour.y, colour.z, 0.0), custom_size: Some(Vec2::splat(GRID_SIZE)), ..default() },
                    Transform::from_translation(grid_to_local(*cell).extend(0.8)),
                    ChargeCell { cell: *cell },
                    ChildOf(ship_e),
                ));
            }
            state.charge = Some(Charge { dist, reach, engines, copy, copy_size, copy_centre, colour, shed_at: 0.0, progress: 0.0 });
            state.ended = None;
        }
    }

    // Charging: the wave, and the copies shed late in it.
    if let (Some(p), Some(charge)) = (progress, state.charge.as_mut()) {
        charge.progress = p;
        // Slightly alive: a breath over the charged cells.
        let breath = 1.0 + 0.06 * (time.elapsed_secs() * 7.0).sin() * p;
        for (_, lit, mut sprite) in &mut cells_lit {
            let d = charge.dist.get(&lit.cell).copied().unwrap_or(charge.reach);
            let a = cell_light(p, d, charge.reach, charge.engines.contains(&lit.cell)) * breath;
            sprite.color.set_alpha(a.min(0.6));
        }
        if p >= SHED_FROM && time.elapsed_secs() - charge.shed_at >= SHED_EVERY && after.iter().filter(|(_, a, ..)| a.fade.is_none()).count() < MOST_COPIES {
            charge.shed_at = time.elapsed_secs();
            let at = tf.transform_point(charge.copy_centre.extend(0.0));
            let c = charge.colour;
            // It slips back off the ship as it's shed: the light falling
            // behind, a trail of the ship stretching out astern.
            let length = charge.copy_size.x.max(charge.copy_size.y);
            let astern = -(tf.rotation * Vec3::X).truncate() * length * 0.22;
            commands.spawn((
                Sprite { image: charge.copy.clone(), color: Color::srgba(c.x, c.y, c.z, 0.0), custom_size: Some(charge.copy_size), ..default() },
                Transform { translation: at.truncate().extend(tf.translation.z + 0.85), rotation: tf.rotation, scale: Vec3::ONE },
                Afterimage { age: 0.0, fade: None, start_alpha: 0.0, drift: astern },
            ));
        }
        state.last = Some(here);
    }

    // The charge is over: jumped, or called off?
    if progress.is_none() && state.charge.is_some() {
        let moved = state.last.is_some_and(|l| l.distance(here) > 3_000.0);
        let waited = state.ended.get_or_insert(0);
        *waited += 1;
        if moved || *waited > 4 {
            let charge = state.charge.take().expect("checked");
            state.ended = None;
            for (e, ..) in &cells_lit {
                commands.entity(e).try_despawn();
            }
            let heading = (tf.rotation * Vec3::X).truncate();
            // Left behind: the newest copy lingers longest.
            let mut copies: Vec<_> = after.iter_mut().filter(|(_, a, ..)| a.fade.is_none()).collect();
            copies.sort_by(|a, b| b.1.age.total_cmp(&a.1.age));
            let n = copies.len().max(1) as f32;
            for (i, (_, mut copy, sprite, _)) in copies.into_iter().enumerate() {
                let order = (i as f32 + 1.0) / n;
                copy.fade = Some(if moved { 0.6 + 1.6 * order } else { 0.4 });
                copy.start_alpha = sprite.color.alpha();
                copy.age = 0.0;
                copy.drift = if moved { copy.drift * (0.6 + 0.8 * order) } else { Vec2::ZERO };
            }
            if moved && charge.progress > 0.5 {
                // The light gets there first.
                *visibility = Visibility::Hidden;
                state.arriving = Some(0.0);
                let length = charge.copy_size.x.max(charge.copy_size.y);
                let to = tf.transform_point(charge.copy_centre.extend(0.0)).truncate();
                let c = charge.colour;
                for (i, ahead) in [2.2f32, 1.6, 1.05, 0.55].into_iter().enumerate() {
                    commands.spawn((
                        Sprite { image: charge.copy.clone(), color: Color::srgba(c.x, c.y, c.z, 0.0), custom_size: Some(charge.copy_size), ..default() },
                        Transform { translation: (to + heading * length * ahead).extend(tf.translation.z + 0.85), rotation: tf.rotation, scale: Vec3::ONE },
                        Foreimage { from: to + heading * length * ahead, to, delay: i as f32 * 0.12, age: 0.0 },
                    ));
                }
            }
        }
    }

    // Arriving: the ship shows once its light has closed in.
    if let Some(t) = state.arriving.as_mut() {
        *t += dt;
        if *t >= ARRIVE_S {
            state.arriving = None;
            *visibility = Visibility::Inherited;
        }
    }

    for (e, mut copy, mut sprite, mut t) in &mut after {
        copy.age += dt;
        match copy.fade {
            // Shed while the ship charges: in, then dimming as it falls back.
            None => {
                let a = (copy.age / 0.3).min(1.0) * 0.22 * (-copy.age / 2.5).exp();
                sprite.color.set_alpha(a);
                t.translation += (copy.drift * dt).extend(0.0);
            }
            Some(fade) => {
                let left = 1.0 - copy.age / fade;
                if left <= 0.0 {
                    commands.entity(e).try_despawn();
                    continue;
                }
                sprite.color.set_alpha(copy.start_alpha * left * left);
                t.translation += (copy.drift * dt).extend(0.0);
            }
        }
    }
    for (e, mut copy, mut sprite, mut t) in &mut fore {
        copy.age += dt;
        let f = ((copy.age - copy.delay) / (ARRIVE_S - copy.delay)).clamp(0.0, 1.0);
        let eased = f * f;
        t.translation = copy.from.lerp(copy.to, eased).extend(t.translation.z);
        // In from nothing, gone as it lands.
        sprite.color.set_alpha(0.26 * (f * 4.0).min(1.0) * (1.0 - f).powf(0.5));
        if copy.age >= ARRIVE_S {
            commands.entity(e).try_despawn();
        }
    }
}

/// AFTERLIGHT_WARP_DASH_AT=<secs> (filming): charge a dash 60 km off to the
/// side then (so the ship has to turn to it), as G with a target set would.
/// AFTERLIGHT_WARP_STAR_AT=<secs>: charge a jump to the nearest other star.
pub fn warp_dash_test_hook(
    mut commands: Commands,
    time: Res<Time>,
    ship: Query<(Entity, &Transform), (With<Ship>, Without<crate::ui::MapWarpCharging>, Without<crate::celestial::components::WarpCharging>)>,
    galaxy: Res<crate::celestial::resources::GalaxyMap>,
    streaming: Res<crate::celestial::resources::SystemStreamingManager>,
    mut done: Local<bool>,
) {
    let secs = std::env::var("AFTERLIGHT_WARP_DASH_SECS").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(5.0);
    if let Some(at) = std::env::var("AFTERLIGHT_WARP_STAR_AT").ok().and_then(|v| v.parse::<f32>().ok()) {
        if *done || time.elapsed_secs() < at {
            return;
        }
        let Ok((e, _)) = ship.single() else { return };
        let here = streaming.current_galaxy_pos;
        let Some(to) = galaxy
            .systems
            .iter()
            .filter(|s| Some(s.id) != streaming.loaded_system)
            .min_by(|a, b| a.galaxy_pos.distance(here).total_cmp(&b.galaxy_pos.distance(here)))
        else {
            return;
        };
        *done = true;
        commands.entity(e).try_insert(crate::celestial::components::WarpCharging {
            target: crate::celestial::resources::GalaxyWarpTarget::System(to.id),
            charge_timer: Timer::from_seconds(secs, TimerMode::Once),
            fuel_cost: 0.0,
        });
        info!("WARP STAR test: charging {secs}s to {} ({})", to.name, to.id);
        return;
    }
    let Some(at) = std::env::var("AFTERLIGHT_WARP_DASH_AT").ok().and_then(|v| v.parse::<f32>().ok()) else { return };
    if *done || time.elapsed_secs() < at {
        return;
    }
    let Ok((e, tf)) = ship.single() else { return };
    *done = true;
    let ahead = (tf.rotation * Quat::from_rotation_z(1.9) * Vec3::X).truncate() * 60_000.0;
    commands.entity(e).try_insert(crate::ui::MapWarpCharging {
        charge_timer: Timer::from_seconds(secs, TimerMode::Once),
        target_pos: tf.translation.truncate() + ahead,
        fuel_cost: 0.0,
    });
    info!("WARP DASH test: charging {secs}s");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_charge_runs_out_from_the_reactor_through_the_ship() {
        // An L of cells: the reactor at one end, the far end round the corner.
        let mut cells: HashSet<IVec2> = HashSet::default();
        for x in 0..6 {
            cells.insert(IVec2::new(x, 0));
        }
        for y in 1..4 {
            cells.insert(IVec2::new(5, y));
        }
        let d = charge_distances(&cells, &[IVec2::ZERO]);
        assert_eq!(d[&IVec2::ZERO], 0.0);
        // Through the ship, round the corner: not as the crow flies.
        assert_eq!(d[&IVec2::new(5, 3)], 8.0);
    }

    #[test]
    fn the_front_reaches_everything_by_the_end_of_the_wave() {
        let reach = 30.0;
        assert_eq!(cell_light(0.0, 20.0, reach, false), 0.0, "nothing lit before it starts");
        assert!(cell_light(WAVE_SHARE, reach, reach, false) > 0.05, "the far end is lit when the wave's done");
        assert!(cell_light(0.3, 2.0, reach, false) > cell_light(0.3, 25.0, reach, false), "near the reactor first");
        assert!(cell_light(0.95, reach, reach, true) > cell_light(0.95, reach, reach, false), "the engines flare");
    }

    #[test]
    fn lining_up_turns_smoothly_and_lands_on_the_heading() {
        let mut omega = 0.0;
        let mut angle = 0.0f32;
        let target = 2.5;
        let mut last = angle;
        for _ in 0..60 * 10 {
            angle = line_up(angle, &mut omega, target, 1.0 / 60.0);
            assert!((angle - last).abs() <= TURN_MAX / 60.0 + 1e-4, "spun too fast");
            last = angle;
        }
        assert_eq!(angle, target);
        // The short way round.
        let mut omega = 0.0;
        let a = line_up(3.0, &mut omega, -3.0, 1.0 / 60.0);
        assert!(a > 3.0 || a < -3.0, "turned the long way: {a}");
    }

    #[test]
    fn cells_cut_off_from_the_reactors_charge_last() {
        let cells: HashSet<IVec2> = [IVec2::ZERO, IVec2::X, IVec2::new(9, 9)].into_iter().collect();
        let d = charge_distances(&cells, &[IVec2::ZERO]);
        assert!(d[&IVec2::new(9, 9)] > d[&IVec2::X]);
    }
}
