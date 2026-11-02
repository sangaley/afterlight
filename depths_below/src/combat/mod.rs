use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::events::*;
use crate::states::{GameState, CombatSet, SpatialSet};

// Re-use the public HitEffect from ship::damage
use crate::ship::damage::HitEffect;

mod weapons;
pub(crate) mod projectiles;
mod mines;
mod effects;
pub mod shields;
pub mod turrets;
pub mod targeting;
pub mod new_projectiles;
pub mod missiles;
pub mod point_defense;
pub mod severance;
pub mod chain_reactions;
pub mod combat_features;
pub mod energy_weapons;
pub mod ammo_types;
pub mod recoil;
pub mod limits;
pub mod impact;
pub mod gun_math;
pub mod shell;
pub mod gun_design;
pub mod missile_design;
pub mod missile_defence;
pub mod gun_assembly;
pub mod turret_deck;
pub mod gun_house;
pub mod lasers;
pub mod report;

/// Dev switch: no weapon consumes ammunition while true (player and AI).
///
/// Off since 2026-10-14. Rounds are priced per gun (ammo_types::round_price),
/// a short wallet buys a partial rearm, and AI ships restock between fights
/// (ai_ship::combat::ai_restock_between_fights). Flip it back on to isolate
/// something that ammo starvation would muddy.
pub const INFINITE_AMMO: bool = false;

/// Floating damage number that drifts upward and fades out
#[derive(Component)]
pub struct FloatingDamage {
    pub timer: Timer,
    pub velocity: f32,
}

// Helper functions to get effective weapon stats (CalculatedStats or base Weapon)
pub(crate) fn get_weapon_damage(calculated: Option<&CalculatedStats>, weapon: &Weapon) -> f32 {
    calculated
        .and_then(|c| c.weapon.as_ref())
        .map(|w| w.damage)
        .unwrap_or(weapon.damage)
}

pub(crate) fn get_weapon_range(calculated: Option<&CalculatedStats>, weapon: &Weapon) -> f32 {
    calculated
        .and_then(|c| c.weapon.as_ref())
        .map(|w| w.range)
        .unwrap_or(weapon.range)
}

pub(crate) fn get_weapon_fire_rate(calculated: Option<&CalculatedStats>, weapon: &Weapon) -> f32 {
    calculated
        .and_then(|c| c.weapon.as_ref())
        .map(|w| w.fire_rate)
        .unwrap_or(weapon.fire_rate)
}

/// Projectile speed base. Was 600 — at the weapon ranges now reachable
/// (lifetime derives from range instead of a fixed timer, see
/// projectiles::spawn_projectile), that meant a max-range shot (9600) took
/// 16+ seconds to arrive. 1800 gets a max-range bullet there in ~3.5s.
pub(crate) const PROJECTILE_SPEED: f32 = 1800.0;
/// Projectile collision radius
pub(crate) const PROJECTILE_RADIUS: f32 = 12.0;
/// Creature collision radius
pub(crate) const CREATURE_RADIUS: f32 = 24.0;
/// Ship collision radius (for enemy projectiles)
pub(crate) const SUBMARINE_RADIUS: f32 = 60.0;

/// Spawn a visual hit-flash sprite at the given position.
pub(crate) fn spawn_hit_effect(commands: &mut Commands, position: Vec2, color: Color, size: f32) {
    commands.spawn((
        (Sprite {
                color,
                custom_size: Some(Vec2::splat(size)),
                ..default()
            }, Transform::from_xyz(position.x, position.y, 0.6)),
        HitEffect {
            timer: Timer::from_seconds(0.2, TimerMode::Once),
        },
    ));
}

/// A gun firing: a short directional flash at the muzzle.
///
/// `spawn_hit_effect` was standing in for this, which meant every weapon in
/// the game -- railgun, gatling, laser -- flashed the same axis-aligned
/// square. A flash points where the barrel does, so this rotates to `dir`,
/// and it is wider than it is tall so the shape reads as coming *out* of
/// something.
pub(crate) fn spawn_muzzle_flash(
    commands: &mut Commands,
    fx: &crate::vfx::effect_textures::EffectTextures,
    position: Vec2,
    dir: Vec2,
    size: f32,
    color: Color,
) {
    let dir = dir.normalize_or_zero();
    if dir == Vec2::ZERO {
        return;
    }
    commands.spawn((
        Sprite {
            image: fx.muzzle.clone(),
            color,
            custom_size: Some(Vec2::new(size * 1.6, size)),
            ..default()
        },
        Transform {
            translation: position.extend(0.6),
            rotation: Quat::from_rotation_z(dir.y.atan2(dir.x)),
            ..default()
        },
        HitEffect {
            // Shorter than the generic 0.2s hit flash: a muzzle flash is the
            // shortest-lived thing in a fight and a lingering one reads as the
            // gun still firing.
            timer: Timer::from_seconds(0.09, TimerMode::Once),
        },
    ));
}

/// Where blasts draw: over the ship's far picture (vfx::far_art), which would
/// otherwise hide every explosion on a hull once the camera is well back.
const BLAST_Z: f32 = 1.6;

/// A warhead going off: expanding fireball, shock ring, radial spray, smoke.
///
/// `spawn_hit_effect` is a static square sized to the blast radius, which
/// makes a 50-unit warhead and a 10-unit bullet strike look like the same
/// event at different zoom levels. `radius` here is the DAMAGE radius; the
/// visuals deliberately overshoot it, because an explosion whose fireball
/// stops exactly at its kill radius reads as smaller than it is.
///
/// `color` says what KIND of detonation this is, and every layer derives from
/// it. The core still goes near-white — anything energetic enough to matter is
/// white-hot at its centre whatever set it off — but it cools toward `color`,
/// so an EMP burst cools to blue and a warhead to orange. Without that a
/// capacitor discharge and a magazine cook-off are the same orange ball, which
/// is exactly the "colour = information, not decoration" rule the art brief is
/// built on.
pub(crate) fn spawn_explosion(
    commands: &mut Commands,
    fx: &crate::vfx::effect_textures::EffectTextures,
    position: Vec2,
    radius: f32,
    color: Color,
) {
    use crate::vfx::particles::{Blast, Particle};

    let c = color.to_srgba();
    // Toward white by `t`, for the hot end of a layer.
    let hot_of = |t: f32, a: f32| {
        Color::srgba(
            c.red + (1.0 - c.red) * t,
            c.green + (1.0 - c.green) * t,
            c.blue + (1.0 - c.blue) * t,
            a,
        )
    };
    // Darkened toward the caller's own hue, for the cool end.
    let cool_of = |k: f32, a: f32| Color::srgba(c.red * k, c.green * k, c.blue * k, a);

    // Unit-sized sprites scaled by Blast, so growth is one number.
    let core = |commands: &mut Commands, img: Option<Handle<Image>>, from: f32, to: f32, dur: f32, z: f32, hot: Color, cool: Color| {
        commands.spawn((
            Sprite {
                image: img.unwrap_or_default(),
                color: hot,
                custom_size: Some(Vec2::ONE),
                ..default()
            },
            Transform::from_xyz(position.x, position.y, z),
            Blast { elapsed: 0.0, duration: dur, from, to, hot, cool },
        ));
    };

    // Fireball: near-white core cooling to the event's own colour.
    core(commands, Some(fx.fireball.clone()), radius * 0.5, radius * 2.2, 0.42, BLAST_Z + 0.02,
         hot_of(0.82, 1.0), cool_of(0.55, 0.7));
    // Shock ring: an actual annulus, so it reads as a front travelling
    // outward rather than as a second fireball behind the first.
    core(commands, Some(fx.ring.clone()), radius * 0.8, radius * 3.4, 0.30, BLAST_Z + 0.01,
         hot_of(0.45, 0.55), cool_of(0.5, 0.0));

    // Radial spray. Unlike an impact fan this is symmetric: a detonation has
    // no incoming direction to report.
    for i in 0..22 {
        let angle = (i as f32 / 22.0) * std::f32::consts::TAU
            + rand::random::<f32>() * 0.28;
        let heading = Vec2::from_angle(angle);
        let speed = radius * (4.0 + rand::random::<f32>() * 6.0);
        let life = 0.25 + rand::random::<f32>() * 0.35;
        let hot = i % 3 == 0;
        commands.spawn((
            Sprite {
                image: fx.spark.clone(),
                color: if hot { hot_of(0.9, 1.0) } else { hot_of(0.25, 1.0) },
                // Longer than the solid quad: the texture's tail is mostly
                // low alpha, so its visible length is well under its footprint.
                custom_size: Some(Vec2::new(if hot { 16.0 } else { 11.0 }, 3.6)),
                ..default()
            },
            Transform {
                translation: position.extend(BLAST_Z + 0.03),
                rotation: Quat::from_rotation_z(angle),
                ..default()
            },
            Particle::new(heading * speed, life),
        ));
    }

    // Smoke: slow, dark, long-lived, so the site still reads a second later.
    for _ in 0..10 {
        let angle = rand::random::<f32>() * std::f32::consts::TAU;
        let heading = Vec2::from_angle(angle);
        let life = 0.9 + rand::random::<f32>() * 0.8;
        let grey = 0.18 + rand::random::<f32>() * 0.16;
        commands.spawn((
            Sprite {
                image: fx.puff(),
                color: Color::srgba(grey, grey * 0.92, grey * 0.88, 0.75),
                // Wider than the solid quad, for the same reason as the
                // missile trail: a soft puff's low-alpha rim reads as empty
                // void, so its effective size is well under its footprint.
                custom_size: Some(Vec2::splat(radius * (0.7 + rand::random::<f32>() * 0.7))),
                ..default()
            },
            Transform::from_xyz(position.x, position.y, 0.59),
            // Alpha down from 0.75: soft puffs overlap far more smoothly than
            // opaque squares, and ten of them at the old value stack into one
            // flat grey disc instead of a cloud with depth.
            Particle::wisp(heading * radius * (0.5 + rand::random::<f32>()), life, 0.5, false),
        ));
    }
}

/// Sparks thrown off an impact, sprayed along `dir`.
///
/// Directional on purpose. A symmetric puff says "something happened here"; a
/// fan says "it went THAT way", and for a ricochet the direction IS the
/// information — it's the only way to see that a round left along a new
/// heading instead of simply failing to do damage.
///
/// `energy` (0..1) scales count and spread: a grazing skip throws a long thin
/// streak, a near-square one that barely turned throws a short hot burst.
pub(crate) fn spawn_impact_sparks(
    commands: &mut Commands,
    fx: &crate::vfx::effect_textures::EffectTextures,
    position: Vec2,
    dir: Vec2,
    energy: f32,
    count: usize,
) {
    let dir = dir.normalize_or_zero();
    if dir == Vec2::ZERO {
        return;
    }
    let energy = energy.clamp(0.0, 1.0);
    for i in 0..count {
        // Grazing hits stay tight to the new heading; blunt ones scatter.
        let spread = (rand::random::<f32>() - 0.5) * (1.4 - energy * 0.9);
        let heading = Vec2::from_angle(spread).rotate(dir);
        let speed = 180.0 + rand::random::<f32>() * (260.0 + energy * 420.0);
        let life = 0.14 + rand::random::<f32>() * 0.26;
        // A few white-hot ones among the orange so the spray has depth
        // instead of reading as one flat colour.
        let hot = i % 3 == 0;
        commands.spawn((
            Sprite {
                image: fx.spark.clone(),
                color: if hot {
                    Color::srgb(1.0, 0.97, 0.86)
                } else {
                    Color::srgb(1.0, 0.68, 0.24)
                },
                custom_size: Some(Vec2::new(if hot { 11.0 } else { 8.0 }, 2.8)),
                ..default()
            },
            Transform {
                translation: position.extend(0.62),
                // Streaks lie along their own flight, so the spray reads as
                // motion rather than as confetti.
                rotation: Quat::from_rotation_z(heading.y.atan2(heading.x)),
                ..default()
            },
            crate::vfx::particles::Particle::new(heading * speed, life),
        ));
    }
}

/// DEPTHS_FX_SCALE: a filming switch that draws shots and beams this many
/// times bigger, so a wide shot of a fight can read. 1.0 in play.
pub fn fx_scale() -> f32 {
    static V: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("DEPTHS_FX_SCALE").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0))
}

/// DEPTHS_NO_DAMAGE_TEXT=1: no floating numbers or words (filming).
fn no_damage_text() -> bool {
    static V: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *V.get_or_init(|| std::env::var("DEPTHS_NO_DAMAGE_TEXT").ok().as_deref() == Some("1"))
}

/// Spawn a floating word that drifts upward and fades out — the same channel
/// as a damage number, for outcomes that aren't a number. A round that skips
/// off a plate did something, and reporting it as "-3" reads as a bad hit
/// rather than as a deflection.
pub(crate) fn spawn_floating_label(commands: &mut Commands, position: Vec2, text: &str, color: Color) {
    if no_damage_text() {
        return;
    }
    commands.spawn((
        Text2d::new(text.to_string()),
        TextFont { font_size: FontSize::Px(16.0), ..default() },
        TextColor(color),
        Transform::from_xyz(position.x, position.y + 20.0, 1.0),
        crate::camera::ZoomInvariantText,
        FloatingDamage {
            timer: Timer::from_seconds(0.9, TimerMode::Once),
            velocity: 34.0,
        },
    ));
}

/// Spawn a floating damage number that drifts upward and fades out.
pub(crate) fn spawn_floating_damage(commands: &mut Commands, position: Vec2, damage: f32, color: Color) {
    if no_damage_text() {
        return;
    }
    commands.spawn((
        Text2d::new(format!("-{}", damage as i32)),
        TextFont { font_size: FontSize::Px(18.0), ..default() },
        TextColor(color),
        Transform::from_xyz(position.x, position.y + 20.0, 1.0),
        crate::camera::ZoomInvariantText,
        FloatingDamage {
            timer: Timer::from_seconds(0.8, TimerMode::Once),
            velocity: 40.0,
        },
    ));
}

/// Apply random angular spread based on accuracy (0..1). Returns adjusted target position.
pub(crate) fn apply_accuracy_spread(origin: Vec2, target_pos: Vec2, accuracy: f32, max_spread_degrees: f32) -> Vec2 {
    let spread = (1.0 - accuracy) * max_spread_degrees;
    let angle_offset = (rand::random::<f32>() - 0.5) * spread.to_radians();
    let dir = (target_pos - origin).normalize_or_zero();
    let rotated_dir = Vec2::new(
        dir.x * angle_offset.cos() - dir.y * angle_offset.sin(),
        dir.x * angle_offset.sin() + dir.y * angle_offset.cos(),
    );
    let dist = origin.distance(target_pos);
    origin + rotated_dir * dist
}

/// Checks whether a target direction falls within a weapon's firing arc
pub(crate) fn is_in_firing_arc(
    ship_rotation: f32,
    module_rotation: &Rotation,
    mount: &WeaponMount,
    direction_to_target: Vec2,
) -> bool {
    use std::f32::consts::FRAC_PI_2;
    match mount.mount_type {
        MountType::Turret => true,
        MountType::Fixed => {
            // facing_angle, not to_radians: module art points +Y, so
            // to_radians is a quarter turn off the way the gun faces. A
            // railgun laid facing the bow had its cone over the starboard
            // side and refused anything dead ahead.
            let module_angle = ship_rotation + module_rotation.facing_angle();
            let weapon_forward = Vec2::new(module_angle.cos(), module_angle.sin());
            let dot = weapon_forward.dot(direction_to_target.normalize_or_zero());
            dot >= (mount.firing_arc / 2.0).to_radians().cos()
        }
        MountType::Broadside => {
            let perp = Vec2::new(
                (ship_rotation + FRAC_PI_2).cos(),
                (ship_rotation + FRAC_PI_2).sin(),
            );
            let dot = perp.dot(direction_to_target.normalize_or_zero()).abs();
            dot >= (mount.firing_arc / 2.0).to_radians().cos()
        }
    }
}

/// Clamps an aim direction into a mount's firing arc. Turrets pass the aim
/// through; fixed/broadside mounts return the nearest arc-edge direction
/// when the aim falls outside their cone. Used so launchers never silently
/// refuse to fire — an off-axis salvo is visible feedback that the mount
/// points elsewhere, a gun that eats the trigger press reads as broken.
pub(crate) fn clamp_to_firing_arc(
    ship_rotation: f32,
    module_rotation: &Rotation,
    mount: &WeaponMount,
    aim_dir: Vec2,
) -> Vec2 {
    use std::f32::consts::FRAC_PI_2;
    let forward = match mount.mount_type {
        MountType::Turret => return aim_dir,
        MountType::Fixed => {
            let angle = ship_rotation + module_rotation.facing_angle();
            Vec2::new(angle.cos(), angle.sin())
        }
        MountType::Broadside => {
            // Two mirrored cones — clamp against the side nearer the aim
            let angle = ship_rotation + FRAC_PI_2;
            let side = Vec2::new(angle.cos(), angle.sin());
            if side.dot(aim_dir) >= 0.0 { side } else { -side }
        }
    };
    let half_arc = (mount.firing_arc / 2.0).to_radians();
    let offset = forward.angle_to(aim_dir);
    if offset.abs() <= half_arc {
        aim_dir
    } else {
        Vec2::from_angle(offset.clamp(-half_arc, half_arc)).rotate(forward)
    }
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app
            .add_systems(Update, report::combat_report.run_if(in_state(crate::states::GameState::Exploring)))
            .init_resource::<targeting::TargetSelection>()
            .init_resource::<targeting::AimLock>()
            .init_resource::<targeting::MarkedTargets>()
            .init_resource::<targeting::FireGroupState>()
            .init_resource::<recoil::RecoilAccumulator>()
            .configure_sets(Update, CombatSet::WeaponFire.after(SpatialSet::Update).run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, CombatSet::Cleanup.after(CombatSet::WeaponFire).run_if(in_state(GameState::Exploring)))
            // Target selection + fire groups (always during exploring).
            // cycle_target (Tab) intentionally NOT registered — Tab is the
            // radar key, and having it also lock onto the nearest enemy was
            // an unwanted side effect. Guns free-aim at the cursor by default;
            // middle-click still opt-in locks a target for auto-aim + bracket.
            .add_systems(Update, (
                targeting::click_select_target,
                targeting::draw_target_bracket,
                // Lock input runs before the radial menu can claim the click,
                // and before fire_group_input so a lock made this frame fires
                // this frame.
                targeting::aim_lock_input
                    .before(crate::ui::windows::radial_menu::spawn_radial_on_right_click),
                targeting::maintain_aim_lock.after(targeting::aim_lock_input),
                targeting::draw_aim_lock.after(targeting::maintain_aim_lock),
                targeting::fire_group_input.after(targeting::maintain_aim_lock),
                // `\` picks which enemy the unlocked battery works on, and
                // assign_auto_aim then hands each gun its own block on it.
                // Ordered before the firing set so a switch takes effect on
                // the frame it was pressed.
                targeting::cycle_target,
                targeting::assign_auto_aim
                    .after(targeting::cycle_target)
                    .after(targeting::maintain_aim_lock)
                    .before(CombatSet::WeaponFire),
            ).run_if(in_state(GameState::Exploring)))
            .add_systems(Update, (
                targeting::mark_all_input.before(targeting::assign_auto_aim),
                targeting::draw_marks,
            ).run_if(in_state(GameState::Exploring)))
            // Shields: attach to player + AI ships, recharge, drive bubble visuals
            .add_systems(Update, (
                shields::attach_player_shield,
                shields::refresh_player_shield_skin,
                shields::refresh_player_shield_capacity.before(shields::update_shields),
                shields::attach_ai_shields,
                shields::refresh_shield_extents.before(shields::update_shields),
                shields::toggle_player_shield,
                shields::update_player_shield_arc,
                shields::update_ai_shield_arcs,
                shields::update_shield_segment,
                shields::update_shields,
                turrets::aim_turrets,
            ).run_if(in_state(GameState::Exploring)))
            // Guns built from parts: re-worked out per ship when its layout
            // changes, docked or flying, before the enhancer pass folds
            // tuning into the numbers it writes.
            .init_resource::<turret_deck::PlayerAimPoint>()
            .init_resource::<turret_deck::TurretShapes>()
            .add_message::<turret_deck::RebuildTurretPlate>()
            .add_systems(Update, (turret_deck::rebuild_turret_plates, gun_house::lay_default_roofs, gun_house::count_roof_fittings))
            // Lasers built from parts: worked out per ship, charged, fired
            // once the turrets have turned, then resolved.
            .add_message::<lasers::LaserShot>()
            .add_systems(Update, (
                lasers::assemble_lasers,
                lasers::charge_lasers,
                lasers::fire_lasers,
                lasers::resolve_laser_shots,
                lasers::fade_laser_beams,
            ).chain().after(turret_deck::traverse_turrets).after(missile_defence::spot_missiles))
            .init_resource::<gun_design::GunDesigns>()
            .add_systems(Startup, gun_design::load_gun_designs)
            .init_resource::<gun_design::WeaponKits>()
            .add_systems(Startup, gun_design::load_weapon_kits)
            .add_systems(Update, (
                turret_deck::ensure_turret_bodies,
                turret_deck::mount_pending_turret_blocks,
                turret_deck::update_player_aim_point,
                turret_deck::measure_turrets,
                turret_deck::traverse_turrets,
            ).chain())
            .add_systems(Update, (missile_defence::throw_decoys, missile_defence::seduce_seekers, missile_defence::age_decoys).chain()
                .after(missiles::move_missiles)
                .after(missile_defence::spot_missiles))
            // Warners first: point defence and decoys act on what they see.
            .add_systems(Update, missile_defence::spot_missiles
                .after(missiles::move_missiles)
                .before(turret_deck::traverse_turrets))
            .add_systems(Update, missile_design::fit_missile_loadouts
                .before(crate::building::multiblock::enhancers::apply_weapon_enhancers))
            .add_systems(Update, (gun_assembly::assemble_guns, gun_assembly::run_gun_racks, gun_assembly::place_muzzles, gun_assembly::flash_down_hoists).chain()
                .before(crate::building::multiblock::enhancers::apply_weapon_enhancers))
            // Player weapons: kinetic projectiles + missiles (new physics system)
            .add_systems(Update, (
                new_projectiles::fire_weapons_system,
                new_projectiles::move_projectiles,
                new_projectiles::check_projectile_hits,
                missiles::fire_missiles_system,
                missiles::move_missiles,
                missiles::skin_missiles,
                new_projectiles::skin_rounds,
                // After the move, so a puff is laid at the position the
                // missile actually reached this frame rather than the last.
                missiles::spawn_missile_trails.after(missiles::move_missiles),
                missiles::check_missile_hits,
                point_defense::intercept_missiles,
                point_defense::pd_missile_collision,
            ).in_set(CombatSet::WeaponFire))
            // Creature/AI weapons: use original projectile system (different entity type)
            .add_systems(Update, (
                effects::creature_ranged_attack,
                projectiles::projectile_movement,
                projectiles::projectile_collision,
            ).in_set(CombatSet::WeaponFire))
            // Cleanup + limits
            .add_systems(Update, (
                effects::animate_floating_damage,
                crate::ship::damage::cleanup_hit_effects,
                limits::enforce_projectile_limit,
                limits::enforce_particle_limit,
                new_projectiles::tick_burning_blocks,
            ).in_set(CombatSet::Cleanup))
            // Fire group assignment (build mode)
            // Severance + chain reactions
            .add_systems(
                Update,
                (
                    severance::check_section_severance,
                    severance::cut_test_hook.run_if(severance::cut_hook_on),
                    severance::severed_turrets_change_hands,
                    severance::move_detached_sections,
                    severance::debris_collision,
                    chain_reactions::trigger_chain_reactions,
                ).run_if(in_state(GameState::Exploring)),
            )
            // Combat features: heat glow, damage arrows, weak points, boarding
            .add_systems(
                Update,
                (
                    combat_features::weapon_heat_visual,
                    combat_features::spawn_damage_indicators,
                    combat_features::update_damage_indicators,
                    combat_features::attach_weak_points,
                    combat_features::update_weak_point_visuals,
                    combat_features::parasite_boarding,
                    combat_features::boarded_parasite_damage,
                    combat_features::crew_fights_boarders,
                ).run_if(in_state(GameState::Exploring)),
            )
            // Recoil
            .add_systems(
                Update,
                (
                    recoil::accumulate_projectile_recoil,
                    recoil::accumulate_missile_recoil,
                    recoil::apply_weapon_recoil
                        .after(recoil::accumulate_projectile_recoil)
                        .after(recoil::accumulate_missile_recoil),
                ).run_if(in_state(GameState::Exploring)),
            )
            // Energy weapons
            .add_systems(
                Update,
                (
                    energy_weapons::fire_laser_system,
                    energy_weapons::fire_ion_system,
                    energy_weapons::update_ion_pulses,
                    energy_weapons::update_ion_disabled,
                    energy_weapons::fire_plasma_system,
                    energy_weapons::fire_emp_missiles,
                    energy_weapons::emp_detonation,
                ).run_if(in_state(GameState::Exploring)),
            )
            // Build mode combat tools
            .add_systems(
                Update,
                (
                    targeting::assign_fire_group,
                    point_defense::toggle_intercept_mode,
                ).run_if(in_state(GameState::StationDocked)),
            );
    }
}

/// A gun with nobody on it does not fire.
///
/// Enemy ships have obeyed this since AI crew existed (`ai_weapon_fire_system`
/// gates on the same `ModuleEfficiency`), but every one of the PLAYER's firing
/// paths ignored it — `crew_weapon_system` only ever gated auto-fire at
/// creatures, so pressing the trigger fired batteries no one was standing at.
/// Crew scarcity is meant to be the cost of a big battery; it wasn't.
///
/// Weapons without a `CrewStation` at all fire freely — that's a property of
/// the module, not an empty post.
pub(super) fn weapon_is_crewed(
    has_station: bool,
    efficiency: Option<&crate::components::ModuleEfficiency>,
) -> bool {
    !has_station || efficiency.is_some_and(|e| e.staffing_factor > 0.0)
}

#[cfg(test)]
mod firing_arc_tests {
    use super::*;

    fn fixed(arc: f32) -> WeaponMount {
        WeaponMount { mount_type: MountType::Fixed, firing_arc: arc }
    }

    #[test]
    fn a_fixed_gun_laid_facing_the_bow_fires_over_the_bow() {
        // Ship heading 0 = bow toward +x; East is the bow (Rotation::facing_offset).
        assert!(is_in_firing_arc(0.0, &Rotation::East, &fixed(10.0), Vec2::X));
        assert!(!is_in_firing_arc(0.0, &Rotation::East, &fixed(10.0), Vec2::NEG_Y), "not over the side");
        assert!(is_in_firing_arc(0.0, &Rotation::North, &fixed(10.0), Vec2::Y));
    }

    #[test]
    fn clamping_pulls_aim_back_inside_the_cone_around_the_facing() {
        let out = clamp_to_firing_arc(0.0, &Rotation::East, &fixed(20.0), Vec2::Y);
        assert!(out.x > 0.9, "clamped toward +x, the way it faces: {out:?}");
    }
}

#[cfg(test)]
mod blast_depth_tests {
    #[test]
    fn blasts_draw_over_the_far_picture() {
        assert!(super::BLAST_Z > crate::vfx::far_art::SHIP_ART_Z);
    }
}
