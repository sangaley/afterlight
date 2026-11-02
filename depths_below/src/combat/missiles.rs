use bevy::prelude::*;
use crate::celestial::components::{GravityAffected, GravityForce};
use super::targeting::{TargetSelection, FireGroupState, FireGroup};
use super::new_projectiles::MissileProjectile;
use crate::vfx::particles::Particle;
use super::*;

// ============================================================================
// MISSILE SYSTEM
// Cold-gas eject -> motor burn -> proportional-navigation guidance.
// Bay chain length determines missile size. Decoyable. Shootable.
// ============================================================================

/// Cold-gas eject speed. The missile leaves the tube on a gas charge, not its
/// own motor — ~3 cells/s, slow enough to read as a "pop" rather than a shot.
const EJECT_SPEED: f32 = 200.0;

/// Seconds of coast before the motor lights, per launcher. At EJECT_SPEED
/// that is ~90-100 units for the big tubes: clear of the hull and its
/// outboard plating before the burn starts, which is what makes the silo
/// obstruction rule mean something.
const EJECT_TIME_HEAVY: f32 = 0.5;
const EJECT_TIME_GUIDED: f32 = 0.4;
const EJECT_TIME_CLUSTER: f32 = 0.2;

/// Base motor thrust. Ignition has to read as a kick, not a drift.
const BOOST_THRUST: f32 = 700.0;

/// Seconds the motor burns. Thrust x burn time is the missile's whole speed
/// budget: 900 x 1.6 = 1440 u/s on top of the eject charge.
const BURN_TIME: f32 = 1.4;

/// Speed ceiling. Turn radius goes with the SQUARE of speed, so this is the
/// single biggest lever on whether a missile can corner: dropping the cap
/// from 2200 to 1200 cuts the circle it flies to under a third of its old
/// size before agility is touched at all.
const MISSILE_MAX_SPEED: f32 = 1200.0 * super::gun_assembly::RANGE_SCALE;

/// Radians of steering authority a fresh missile carries. Every radian it
/// turns is deducted; at zero it coasts, so only a missile genuinely fighting
/// an evasive target runs itself dry.
///
/// Raised alongside the launch agility bump: at LAUNCH_TURN_RATE a missile
/// held at full deflection burns ~7 rad/s, which would bankrupt the old 25
/// budget inside the agility window and leave it coasting exactly when it
/// should be dangerous. The hard turn is meant to be affordable once, not
/// free forever.
const RESERVE_TURN: f32 = 34.0;

/// Turn-rate ceiling the instant the motor lights, in rad/s.
///
/// A missile is at its most agile right after it clears the tube: slow, and
/// carrying every bit of its control authority. This is the "hard turn off
/// the rail" — it leaves straight down the silo heading, then cranks over
/// onto the target.
const LAUNCH_TURN_RATE: f32 = 7.0;

/// Turn-rate ceiling once the missile has been flying a while.
///
/// Late in flight it is fast and has spent its authority, so corrections come
/// out wide and lazy. This is what makes a missile dodgeable: survive the
/// first seconds and its turning circle keeps growing.
const CRUISE_TURN_RATE: f32 = 1.4;

/// Seconds over which agility decays from LAUNCH_TURN_RATE to
/// CRUISE_TURN_RATE. Short enough that a knife-fight launch is a real threat
/// and a long-range shot can be out-turned.
const AGILITY_FALLOFF: f32 = 3.5;

/// The turn-rate ceiling for a missile that has been flying `age` seconds.
///
/// Independent of the lateral-acceleration cap, which already widens turns
/// with speed. This adds the TIME axis the user asked for: straight off the
/// rail, hard over early, progressively wider the longer it stays up.
fn turn_ceiling(age: f32) -> f32 {
    let t = (age / AGILITY_FALLOFF).clamp(0.0, 1.0);
    // Ease-out so the loss of agility is felt early rather than as a cliff.
    let t = t * t * (3.0 - 2.0 * t);
    LAUNCH_TURN_RATE + (CRUISE_TURN_RATE - LAUNCH_TURN_RATE) * t
}

/// Heading-error gain for the pure-pursuit fallback. Proportional, so a
/// missile only slightly off the target eases on instead of slamming full
/// deflection and sawing back the other way.
const PURSUIT_GAIN: f32 = 2.5;

/// Proportional-navigation gain. 3 flies a visible curve onto the target; 5
/// snaps almost straight; below 2 it wanders and misses.
const PN_GAIN: f32 = 3.0;

/// Max lateral acceleration by launcher, in units/s².
///
/// The number to reason about is TURN RADIUS, r = v²/a, because that is what
/// you actually see. At the 1200 u/s cap a Guided pulls a ~275-unit circle
/// (~4 cells) and a Heavy a ~450-unit one (~7 cells). The previous values
/// gave the Heavy a 29-cell circle: technically guided, and in practice it
/// sailed past everything it was aimed at.
const LATERAL_GUIDED: f32 = 5200.0;
const LATERAL_HEAVY: f32 = 3200.0;

/// Inside this range the seeker is allowed a harder turn — the terminal dive.
const TERMINAL_RANGE: f32 = 300.0 * super::gun_assembly::RANGE_SCALE;
const TERMINAL_AGILITY_BONUS: f32 = 1.6;

/// Seconds before self-destruct, per launcher. Heavy carries the most: range
/// 9600 at ~1400 u/s average is ~7s, plus margin to hook round for a second
/// pass after an overshoot.
const LIFE_HEAVY: f32 = 9.0;
const LIFE_GUIDED: f32 = 8.0;
const LIFE_CLUSTER: f32 = 5.0;

/// Fire missile weapons when their fire group is active
pub fn fire_missiles_system(
    time: Res<Time>,
    fire_state: Res<FireGroupState>,
    power_state: Res<crate::resources::PowerState>,
    selection: Res<TargetSelection>,
    aim_lock: (Res<super::targeting::AimLock>, Res<super::targeting::MarkedTargets>, Local<usize>),
    ship_query: Query<(Entity, &ShipPhysics, &Transform, &Velocity), With<Ship>>,
    mut weapon_query: Query<(
        // WeaponMount is gone from here deliberately: a missile leaves down
        // its own tube regardless of arc, so nothing consults the mount.
        Entity, &Module, Has<CrewStation>, Option<&ModuleEfficiency>, &mut Weapon, &mut WeaponCooldown,
        &GlobalTransform, &FireGroup, &ChildOf,
        Option<&crate::building::customization::tuning::WeaponTuning>,
        Option<&ModuleTemperature>,
        Option<&super::missile_design::MissileLoadout>,
    ), Without<DestroyedModule>>,
    // One query, not three: everything a missile may chase, and the lookup
    // for whatever is already locked. Bevy caps a system at 16 parameters
    // and this one was at the ceiling.
    target_query: Query<
        (Entity, &Transform, Option<&Creature>, Option<&Velocity>),
        (Without<Ship>, Or<(With<crate::ai_ship::components::AiShip>, With<Creature>)>),
    >,
    machine_stats: Query<&crate::building::multiblock::components::MachineStats>,
    mut fuel_state: ResMut<crate::resources::FuelState>,
    windows_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<crate::camera::MainCamera>>,
    input_state: Res<crate::resources::InputState>,
    mut commands: Commands,
    mut notifications: MessageWriter<ShowNotification>,
    mut fired_events: MessageWriter<crate::events::WeaponFired>,
) {
    let (aim_lock, marks, mut mark_turn) = aim_lock;
    let Ok((player_ship, ship_physics, ship_transform, ship_velocity)) = ship_query.single() else { return };

    // Weapons need power — grid deficit silences launchers too.
    if power_state.power_balance < 0.0 {
        return;
    }

    // Cursor world position — dumb-fire fallback when no target is selected.
    let cursor_world: Option<Vec2> = windows_query.single().ok()
        .and_then(|w| w.cursor_position())
        .and_then(|c| {
            camera_query.single().ok()
                .and_then(|(cam, gt)| cam.viewport_to_world_2d(gt, c).ok())
        });
    // Controller right-stick aim beats the mouse while it owns aim (see
    // InputState.gamepad_aim).
    let cursor_world = input_state.gamepad_aim
        .map(|dir| ship_transform.translation.truncate() + dir * 2000.0)
        .or(cursor_world);

    // Everything a missile could reasonably chase, gathered once. Cheap —
    // a handful of AI ships and whatever creatures are loaded — and reused
    // by every launcher below.
    let candidates: Vec<(Entity, Vec2)> = target_query
        .iter()
        .filter(|(_, _, creature, _)| creature.is_none_or(|c| c.health > 0.0))
        .map(|(e, t, _, _)| (e, t.translation.truncate()))
        .collect();

    for (entity, module, has_station, staffing, mut weapon, mut cooldown, global_transform, fire_group, parent, tuning, temp, loadout) in weapon_query.iter_mut() {
        // A gun with nobody on it does not fire.
        if !crate::combat::weapon_is_crewed(has_station, staffing) {
            if loadout.is_some() || super::missile_design::is_built_launcher(module.module_type) {
                if super::gun_assembly::trace() && time.elapsed_secs() % 5.0 < time.delta_secs() {
                    info!("MISSILE launcher {:?} unmanned", module.module_type);
                }
            }
            continue;
        }
        // Player ship only — see fire_weapons_system for why this matters:
        // AI ships carry identical missile-bay components and would
        // otherwise launch whenever the player fires, homing on the
        // player's own target selection.
        if parent.parent() != player_ship { continue; }
        // Only missile-type weapons
        let built = super::missile_design::launcher_spec(module.module_type);
        if !matches!(module.module_type,
            ModuleType::HeavyMissile | ModuleType::GuidedMissile | ModuleType::ClusterRocket
        ) && built.is_none() { continue; }
        // A design too long for its launcher can't be loaded.
        let design = built.map(|spec| {
            let d = super::missile_design::design_of(module.module_type, loadout);
            let s = super::missile_design::stats(&d, spec);
            (d, s)
        });
        if design.as_ref().is_some_and(|(_, s)| s.parts > s.launcher.max_parts) { continue; }

        if !module.is_active { continue; }

        // Tick before the thermal gate — see fire_weapons_system: gating
        // first freezes the timer, which generate_heat reads as "recently
        // fired", locking the launcher hot forever.
        cooldown.timer.tick(time.delta());
        if !cooldown.timer.is_finished() { continue; }

        // Thermal throttle — same gate the laser/kinetics use.
        if let Some(temp) = temp {
            if temp.current >= temp.max_temp * 0.95 { continue; }
        }

        let group_firing = fire_state.firing[fire_group.group as usize % 4];
        if !group_firing { continue; }

        if !crate::combat::INFINITE_AMMO && weapon.ammo <= 0 { continue; }

        let weapon_pos = global_transform.translation().truncate();

        // Homing target, in the order the player expects to be obeyed.
        //
        // Right-click block lock is the documented primary way to aim the
        // battery (see combat::targeting::aim_lock) and every other weapon
        // honours it — missiles read only `selection.target`, the middle-click
        // ship lock, so the normal aiming flow produced unguided rockets that
        // flew at the cursor. Homing follows the locked SHIP rather than the
        // locked block: a block's Transform is ship-local, and the warhead's
        // blast radius covers the difference anyway.
        let locked = aim_lock.ship.filter(|_| aim_lock.is_locked());
        // Nothing locked: seek on the player's behalf rather than throwing a
        // guided weapon away as a dumb rocket. Candidates are scored by
        // distance to the CURSOR, so where you point still decides which
        // enemy gets the salvo, but only ones inside this launcher's range
        // are eligible — a seeker aimed at something it cannot reach is just
        // a slower miss.
        let auto = cursor_world.and_then(|cursor| {
            candidates.iter()
                .filter(|(_, pos)| weapon_pos.distance(*pos) <= weapon.range)
                .min_by(|(_, a), (_, b)| {
                    cursor.distance_squared(*a)
                        .partial_cmp(&cursor.distance_squared(*b))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(e, _)| *e)
        });
        // Only the seeker's target is decided here. Where the missile is
        // POINTED is no longer a function of the target at all — it leaves
        // down the tube either way — so a launcher with nothing to chase
        // still fires, and flies out unguided.
        // Marked ships take the launchers' shots in turn, among those in range.
        let marked: Vec<Entity> = marks.ships.iter().copied()
            .filter(|e| candidates.iter().any(|(c, pos)| c == e && weapon_pos.distance(*pos) <= weapon.range))
            .collect();
        if locked.is_none() && !marks.ships.is_empty() && marked.is_empty() {
            continue;
        }
        let next_marked = (!marked.is_empty()).then(|| {
            *mark_turn = mark_turn.wrapping_add(1);
            marked[*mark_turn % marked.len()]
        });
        let homing_target = locked.or(next_marked).or(selection.target).or(auto)
            .filter(|e| target_query.get(*e).is_ok());

        // The tube points where the tube points.
        //
        // Launch direction used to be the aim direction bent back to the edge
        // of the firing arc, so a target anywhere inside the cone meant the
        // missile left already angled onto it — nothing about the launch said
        // "silo". Ejecting along the module's own facing instead makes every
        // launch come out of the tube the way the tube is welded on, whatever
        // the ship is doing, and leaves the turn onto the target to the
        // seeker where it belongs. It is also inherently safe in the way the
        // arc clamp was trying to be: a missile that leaves along its own
        // tube never leaves backwards through the hull.
        //
        // The +FRAC_PI_2 is not a fudge: module art in this game is drawn
        // pointing +Y, not +X (turrets::aim_turrets subtracts the same
        // quarter turn for barrels). Rotation::East is -PI/2, so an
        // East-mounted launcher RENDERS pointing +X — the bow — which is why
        // every layout puts bow tubes at East and stern engines at West.
        // Without this the missile left 90 degrees off the tube it visibly
        // came out of.
        let mut launch_dir = Vec2::from_angle(ship_physics.rotation + module.rotation.facing_angle());
        // Where fire control wants a designed missile to go until its seeker
        // takes over: where the target will be when it gets there.
        let target_now = homing_target.and_then(|e| target_query.get(e).ok())
            .map(|(_, t, _, v)| (t.translation.truncate(), v.map(|v| v.0).unwrap_or(Vec2::ZERO)));
        let launch_aim = match (&design, target_now) {
            (Some((_, s)), Some((at, vel))) => {
                let tgo = weapon_pos.distance(at) / (s.top_speed * 0.8).max(600.0);
                Some(at + vel * tgo)
            }
            (Some(_), None) => cursor_world.or(Some(weapon_pos + launch_dir * 3000.0)),
            _ => None,
        };
        // A silo launches straight up out of the deck: drawn top-down, out
        // from the ship on the side it stands, and the missile turns onto
        // its target once its motor lights. Aimed at the target from the
        // tube, one behind the ship flew back across its own hull.
        if module.module_type == ModuleType::VlsSilo {
            launch_dir = silo_out(weapon_pos, ship_transform.translation.truncate(), launch_dir);
        }

        // Bay chain length is missile size — see MachineStats.
        let bay_count = machine_stats.get(entity)
            .map(|s| s.barrel_count.max(1))
            .unwrap_or(1);
        // Ship fuel pays for the LAUNCH; the missile's own burn and steering
        // authority are properties of the missile, not of how much fuel the
        // ship happened to have.
        let fuel_cost = 5.0 * bay_count as f32;
        if fuel_state.current_fuel < fuel_cost {
            notifications.write(ShowNotification {
                message: "No fuel for missile launch!".into(),
                notification_type: NotificationType::Warning,
                duration: 2.0,
            });
            continue;
        }
        fuel_state.current_fuel -= fuel_cost;

        cooldown.timer.reset();
        if !crate::combat::INFINITE_AMMO {
            weapon.ammo = weapon.ammo.saturating_sub(1);
        }
        fired_events.write(crate::events::WeaponFired {
            weapon_type: module.module_type,
            position: weapon_pos,
            from_player: true,
        });

        let launch = MissileLaunch {
            weapon: entity,
            ship: player_ship,
            launch_cell: module.grid_position,
            // A silo fires up out of the deck: nothing of its own ship is
            // ever in front of it (a zero axis obstructs nothing).
            launch_axis: if module.module_type == ModuleType::VlsSilo { IVec2::ZERO } else { module.rotation.facing_offset() },
            muzzle: weapon_pos,
            launch_dir,
            ship_velocity: ship_velocity.0,
            target: homing_target,
            module_type: module.module_type,
            damage: weapon.damage,
            bays: bay_count,
            thrust_mult: tuning.map(|t| t.velocity).unwrap_or(1.0),
        };
        match &design {
            Some((d, s)) => launch_built(&mut commands, &launch, s, d.head, launch_aim.unwrap_or(weapon_pos + launch_dir * 3000.0), true),
            None => { launch_missiles(&mut commands, &launch); }
        }

        // (No launch notification. There was a yellow "Heavy missile
        // launched!" for every one of our own shots: two launchers on the
        // starter filled the notification stack in any fight, burying the
        // messages that matter. The missile leaving the hull is the feedback.)
    }
}

/// Everything one launcher needs to put a missile in the air.
///
/// Shared by the player's fire_missiles_system and the AI's gunnery loop.
/// Enemy launchers used to route through the legacy straight-line projectile
/// path, so an "enemy missile" was a fast red bullet — none of the eject,
/// guidance or collision behaviour applied to them at all.
pub struct MissileLaunch {
    pub weapon: Entity,
    pub ship: Entity,
    pub launch_cell: IVec2,
    /// The tube's axis in ship-LOCAL cells: the module's facing offset.
    ///
    /// `launch_dir` is the same heading in world space and drifts out of date
    /// the moment the ship turns, which is exactly when it matters. The cell
    /// axis does not move with the ship, so it still says which way is "down
    /// the tube" a second after launch.
    pub launch_axis: IVec2,
    pub muzzle: Vec2,
    /// Tube heading in WORLD space: ship rotation + the module's facing.
    pub launch_dir: Vec2,
    pub ship_velocity: Vec2,
    pub target: Option<Entity>,
    pub module_type: ModuleType,
    pub damage: f32,
    pub bays: u32,
    pub thrust_mult: f32,
}

/// Which art a missile wears (assets/sprites/missiles, tools/missile-art):
/// one per seeker head so a salvo reads at a glance, and the old one-block
/// launchers' rocket. Put on by `skin_missiles` the frame after launch.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissileSkin(pub &'static str);

impl MissileSkin {
    pub fn of(head: super::missile_design::MissileHead) -> Self {
        use super::missile_design::MissileHead;
        Self(match head {
            MissileHead::Heat => "missile_heat",
            MissileHead::Radar => "missile_radar",
            MissileHead::CommandLink => "missile_command",
            MissileHead::DumbNose => "missile_dumb",
        })
    }
}

/// Dress newly launched missiles in their art. They were flat rectangles.
pub fn skin_missiles(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut new: Query<(Entity, &MissileSkin, &mut Sprite), Added<MissileSkin>>,
) {
    for (missile, skin, mut sprite) in new.iter_mut() {
        sprite.image = assets.load(format!("sprites/missiles/{}.png", skin.0));
        sprite.color = Color::WHITE.with_alpha(sprite.color.alpha());
        commands.entity(missile).try_remove::<MissileSkin>();
    }
}

/// Spawn a launcher's salvo. Returns how many bodies left the tube.
pub fn launch_missiles(commands: &mut Commands, l: &MissileLaunch) -> u32 {
    let size_mult = l.bays.max(1) as f32;
    let bulk = size_mult.sqrt();
    let (max_lateral, eject_time, life) = match l.module_type {
        ModuleType::GuidedMissile => (LATERAL_GUIDED / bulk, EJECT_TIME_GUIDED, LIFE_GUIDED),
        ModuleType::HeavyMissile => (LATERAL_HEAVY / bulk, EJECT_TIME_HEAVY, LIFE_HEAVY),
        ModuleType::ClusterRocket => (0.0, EJECT_TIME_CLUSTER, LIFE_CLUSTER),
        _ => (LATERAL_HEAVY / bulk, EJECT_TIME_HEAVY, LIFE_HEAVY),
    };
    let missile_damage = l.damage * size_mult;
    let volley = match l.module_type {
        ModuleType::ClusterRocket => (3.0 * size_mult).min(8.0) as u32,
        _ => 1,
    };
    let visual_w = 24.0 + size_mult * 8.0;
    // The rocket art is four times as long as it's wide, fins and all.
    let visual_h = visual_w * 0.25;

    for i in 0..volley {
        // Fan the volley by ROTATING the launch heading, not by adding a
        // world-space offset — that shoved every rocket the same way
        // regardless of which way the tube pointed.
        let spread_angle = if volley > 1 {
            (i as f32 - (volley - 1) as f32 / 2.0) * 0.12
        } else {
            0.0
        };
        let (sin, cos) = spread_angle.sin_cos();
        let dir = Vec2::new(
            l.launch_dir.x * cos - l.launch_dir.y * sin,
            l.launch_dir.x * sin + l.launch_dir.y * cos,
        );
        // Inherit the ship's own velocity, or a ship at speed rear-ends its
        // own salvo.
        let vel = dir * EJECT_SPEED + l.ship_velocity;
        let per_missile = if volley > 1 { missile_damage / volley as f32 } else { missile_damage };

        commands.spawn((
            (Sprite {
                    color: Color::srgb(0.8, 0.3, 0.2),
                    custom_size: Some(Vec2::new(visual_w, visual_h)),
                    ..default()
                }, Transform {
                    translation: Vec3::new(l.muzzle.x, l.muzzle.y, 0.5),
                    rotation: Quat::from_rotation_z(dir.y.atan2(dir.x)),
                    ..default()
                }, MissileSkin("rocket")),
            MissileProjectile {
                age: 0.0,
                damage: per_missile,
                target: if max_lateral > 0.0 { l.target } else { None },
                burn_fuel: BURN_TIME * bulk,
                reserve_fuel: RESERVE_TURN,
                thrust: BOOST_THRUST * l.thrust_mult / bulk * super::gun_assembly::RANGE_SCALE,
                max_lateral,
                arm_distance: 80.0,
                blast_radius: blast_radius_for(size_mult),
                owner: l.weapon,
                eject_time,
                launch_dir: dir,
                life,
                terminal_range: TERMINAL_RANGE,
                prev_pos: l.muzzle,
                owner_ship: Some(l.ship),
                launch_cell: l.launch_cell,
                launch_axis: l.launch_axis,
                ..default()
            },
            MissileTrail::default(),
            Velocity(vel),
            GravityAffected { mass: 2.0 + size_mult },
            GravityForce::default(),
        ));
    }
    volley
}

/// A missile built from a design (combat::missile_design): its numbers and
/// what its seeker is doing. It flies on the same `MissileProjectile` as the
/// fixed-list missiles; `move_missiles` and `check_missile_hits` read this
/// for everything the design decides.
#[derive(Component, Clone, Debug)]
pub struct BuiltMissile {
    pub stats: super::missile_design::MissileStats,
    pub head: super::missile_design::MissileHead,
    /// The seeker has the target.
    pub locked: bool,
    /// Where fire control sent it, until the seeker locks.
    pub launch_aim: Vec2,
    /// A command link's steering error, growing with distance from its ship.
    pub wander: Vec2,
    pub hp: f32,
    /// Launched by the player: a command link follows the player's aim.
    pub player: bool,
    /// The decoy it has been fooled into chasing (missile_defence).
    pub decoy: Option<Entity>,
}

/// Put one designed missile in the air.
pub fn launch_built(
    commands: &mut Commands,
    l: &MissileLaunch,
    s: &super::missile_design::MissileStats,
    head: super::missile_design::MissileHead,
    launch_aim: Vec2,
    player: bool,
) {
    use super::missile_design::MissileHead;
    let dir = l.launch_dir;
    let vel = dir * s.launcher.eject + l.ship_velocity;
    let length = 16.0 + 16.0 * s.k.sqrt() + 3.0 * s.parts as f32;
    if super::gun_assembly::trace() {
        info!("MISSILE launched from {:?} ({}): {:?} seeker, target {:?}, {:.0} u/s top, aiming {:.0} u off", l.module_type, if player { "player" } else { "AI" }, head, l.target, s.top_speed, l.muzzle.distance(launch_aim));
    }
    commands.spawn((
        (Sprite {
                color: Color::srgb(0.78, 0.8, 0.84),
                // The art is four times as long as it's wide, fins and all.
                custom_size: Some(Vec2::new(length, (length * 0.25).max(6.0 + 6.0 * s.k.sqrt()))),
                ..default()
            }, Transform {
                translation: Vec3::new(l.muzzle.x, l.muzzle.y, 0.5),
                rotation: Quat::from_rotation_z(dir.y.atan2(dir.x)),
                ..default()
            }, MissileSkin::of(head)),
        MissileProjectile {
            damage: s.he_damage,
            target: if head == MissileHead::DumbNose { None } else { l.target },
            burn_fuel: s.burn_s,
            // A designed missile's agility is its fins and its age, not a
            // budget it spends.
            reserve_fuel: f32::MAX,
            thrust: s.accel,
            max_lateral: s.lateral,
            arm_distance: 80.0,
            blast_radius: s.he_radius.max(s.frag_radius).max(60.0),
            owner: l.weapon,
            eject_time: s.launcher.popup,
            launch_dir: dir,
            life: s.life_s,
            terminal_range: 0.0,
            prev_pos: l.muzzle,
            owner_ship: Some(l.ship),
            launch_cell: l.launch_cell,
            launch_axis: l.launch_axis,
            ..default()
        },
        BuiltMissile { stats: *s, head, locked: head == MissileHead::CommandLink, launch_aim, wander: Vec2::ZERO, hp: s.toughness, player, decoy: None },
        MissileTrail::default(),
        Velocity(vel),
        GravityAffected { mass: 1.0 + s.mass_t },
        GravityForce::default(),
    ));
}

/// Per-missile exhaust bookkeeping.
///
/// Kept off `MissileProjectile` so a plasma bolt — which borrows that
/// component purely for blast resolution — does not trail rocket smoke.
/// The timers are per-missile rather than a shared `Local`, so a volley
/// staggers its puffs instead of emitting every particle on one frame.
#[derive(Component)]
pub struct MissileTrail {
    flame: f32,
    smoke: f32,
    ignited: bool,
}

impl Default for MissileTrail {
    fn default() -> Self {
        // Staggered start so a cluster salvo does not pulse in lockstep.
        Self { flame: rand::random::<f32>() * 0.02, smoke: rand::random::<f32>() * 0.03, ignited: false }
    }
}

/// Distance from a missile's centre back to its nozzle.
const NOZZLE_OFFSET: f32 = 14.0;

/// Exhaust, smoke and the one-shot ignition flash.
///
/// The smoke is the load-bearing part: it is emitted with almost no velocity
/// of its own, so it hangs where the missile passed and draws the whole curve
/// on screen. Without it the flight path only exists for as long as the
/// missile is standing on it.
pub fn spawn_missile_trails(
    time: Res<Time>,
    mut commands: Commands,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    mut query: Query<(&MissileProjectile, &Transform, &Velocity, &mut MissileTrail)>,
) {
    let dt = time.delta_secs();

    for (missile, transform, velocity, mut trail) in query.iter_mut() {
        let pos = transform.translation.truncate();
        // While ejecting the missile is pointed down the tube even though its
        // velocity carries the ship's motion, so the plume has to follow the
        // same heading the sprite does or it blows out of the wrong end.
        let heading = if missile.eject_time > 0.0 {
            missile.launch_dir
        } else {
            velocity.0.normalize_or_zero()
        };
        if heading == Vec2::ZERO {
            continue;
        }
        let nozzle = pos - heading * NOZZLE_OFFSET;

        // === COLD GAS: pale, slow, thin. The motor is not lit yet. ===
        if missile.eject_time > 0.0 {
            trail.flame -= dt;
            if trail.flame <= 0.0 {
                trail.flame = 0.045;
                let drift = Vec2::from_angle((rand::random::<f32>() - 0.5) * 0.9).rotate(-heading);
                let life = 0.3 + rand::random::<f32>() * 0.2;
                commands.spawn((
                    Sprite {
                        image: fx.puff(),
                        color: Color::srgba(0.82, 0.86, 0.92, 0.5),
                        custom_size: Some(Vec2::splat(10.0 + rand::random::<f32>() * 7.0)),
                        ..default()
                    },
                    Transform::from_xyz(nozzle.x, nozzle.y, 0.45),
                    Particle::wisp(drift * (50.0 + rand::random::<f32>() * 40.0), life, 0.5, true),
                ));
            }
            continue;
        }

        // === IGNITION: one-shot flash out the back. ===
        if !trail.ignited {
            trail.ignited = true;
            spawn_muzzle_flash(&mut commands, &fx, nozzle, -heading, 30.0, Color::srgb(1.0, 0.86, 0.55));
            spawn_impact_sparks(&mut commands, &fx, nozzle, -heading, 0.45, 10);
        }

        // === FLAME: only while the motor is actually burning. ===
        if missile.burn_fuel > 0.0 {
            trail.flame -= dt;
            if trail.flame <= 0.0 {
                trail.flame = 0.018;
                let spread = Vec2::from_angle((rand::random::<f32>() - 0.5) * 0.5).rotate(-heading);
                let life = 0.12 + rand::random::<f32>() * 0.14;
                let hot = rand::random::<f32>() < 0.4;
                commands.spawn((
                    Sprite {
                        image: fx.flame(),
                        color: if hot {
                            Color::srgba(1.0, 0.95, 0.75, 1.0)
                        } else {
                            Color::srgba(1.0, 0.55, 0.15, 0.95)
                        },
                        // Up from 7/10 for the same reason as the trail smoke:
                        // a soft flame's low-alpha rim reads as void, so the
                        // plume would otherwise have shrunk when it gained a
                        // texture.
                        custom_size: Some(Vec2::splat(if hot { 13.0 } else { 19.0 })),
                        ..default()
                    },
                    Transform::from_xyz(nozzle.x, nozzle.y, 0.55),
                    Particle::wisp(spread * (200.0 + rand::random::<f32>() * 180.0), life, 1.0, true),
                ));
            }
        }

        // === SMOKE: hangs in place and marks the path. ===
        trail.smoke -= dt;
        if trail.smoke <= 0.0 {
            trail.smoke = 0.03;
            let life = 1.0 + rand::random::<f32>() * 0.6;
            let grey = 0.30 + rand::random::<f32>() * 0.18;
            commands.spawn((
                Sprite {
                    image: fx.puff(),
                    // Bigger than the solid quad this replaced. A soft puff's
                    // visible core is roughly the inner half of its footprint
                    // -- the rest is low-alpha rim that vanishes against the
                    // void -- so matched sizes would read as a fainter,
                    // thinner trail than the squares, not a softer one.
                    color: Color::srgba(grey, grey * 0.96, grey * 0.93, 0.5),
                    custom_size: Some(Vec2::splat(13.0 + rand::random::<f32>() * 10.0)),
                    ..default()
                },
                Transform::from_xyz(nozzle.x, nozzle.y, 0.44),
                // Almost no velocity: the trail should stay where it was laid
                // down, not chase the missile that laid it.
                Particle::wisp(
                    Vec2::from_angle(rand::random::<f32>() * std::f32::consts::TAU) * 14.0,
                    life,
                    0.5,
                    false,
                ),
            ));
        }
    }
}

/// Which way a VLS silo's missile leaves: straight out from its ship's
/// middle through the silo, or `fallback` for a silo dead on the middle.
pub fn silo_out(silo: Vec2, ship_middle: Vec2, fallback: Vec2) -> Vec2 {
    (silo - ship_middle).normalize_or(fallback)
}

/// A missile near its own ship never flies back through it. `from` and
/// `dir` are in the ship's own cells (its grid turns with it); `look` is how
/// many cells ahead to check, a second and a half of flight or as far as
/// its target, whichever is nearer. A heading whose line runs into the
/// ship's blocks turns, by as little as clears them, toward the side it
/// was already leaning; None when it's clear as it is.
pub fn clear_of_home(grid: &crate::building::ShipGrid, from: Vec2, dir: Vec2, look: f32) -> Option<Vec2> {
    let blocked = |d: Vec2| !grid.walk(from + d * 0.75, from + d * look.max(1.0)).is_empty();
    if dir.length_squared() < 1e-6 || !blocked(dir) {
        return None;
    }
    let base = dir.to_angle();
    (1..=18)
        .flat_map(|i| [i as f32 * 0.17, -(i as f32) * 0.17])
        .map(|a| Vec2::from_angle(base + a))
        .find(|d| !blocked(*d))
}

/// Move and guide missiles: cold-gas eject, then motor burn, then guidance.
///
/// The eject phase is the whole trick. A missile that lights its motor in the
/// tube and steers immediately just flies at the target, which is what made
/// the old ones read as slow bullets. Committing the first ~90 units to the
/// silo heading means the seeker inherits a bad angle and has to fly a curve
/// out of it — and a sluggish one overshoots and comes back around.
pub fn move_missiles(
    time: Res<Time>,
    mut commands: Commands,
    mut missile_query: Query<(Entity, &mut MissileProjectile, &mut Transform, &mut Velocity, Option<&mut BuiltMissile>)>,
    target_query: Query<(&Transform, Option<&Velocity>), Without<MissileProjectile>>,
    hulls: Query<(&GlobalTransform, &crate::building::ShipGrid)>,
    // What a designed missile's seeker sees on its target.
    seeing: (
        Query<(Option<&crate::combat::shields::ShipShield>, Option<&Children>), Without<MissileProjectile>>,
        Query<(&Module, &GlobalTransform), Without<DestroyedModule>>,
        Res<super::turret_deck::PlayerAimPoint>,
        Query<&Transform, (With<super::missile_defence::MissileDecoy>, Without<MissileProjectile>)>,
    ),
) {
    let dt = time.delta_secs();
    let (ships_seen, blocks_seen, player_aim, decoys) = (&seeing.0, &seeing.1, &seeing.2, &seeing.3);

    for (entity, mut missile, mut transform, mut velocity, built) in missile_query.iter_mut() {
        let pos = transform.translation.truncate();
        missile.prev_pos = pos;

        missile.age += dt;
        missile.life -= dt;
        if missile.life <= 0.0 {
            commands.entity(entity).try_despawn();
            continue;
        }

        missile.traveled += velocity.0.length() * dt;
        if !missile.armed && missile.traveled > missile.arm_distance {
            missile.armed = true;
        }

        // Still threading its own ship? Then it is in a launch channel and
        // holds its heading.
        //
        // This is the Forts behaviour: blocks to the LEFT and RIGHT of a tube
        // do not stop a missile, they guide it — it keeps going forward until
        // it is clear. Only something dead ahead stops it, and that is a
        // collision, handled in check_missile_hits. A fixed eject timer could
        // not express this: a tube at the bottom of a long channel would
        // start steering while still inside the hull and turn into a wall.
        let in_channel = missile.owner_ship.and_then(|ship| hulls.get(ship).ok())
            .is_some_and(|(ship_gt, grid)| {
                let cell = crate::building::world_to_cell(ship_gt, pos);
                let c = IVec2::new(cell.x.round() as i32, cell.y.round() as i32);
                // The cell it is in, or either shoulder of it — a missile
                // level with the hull is still leaving, not yet away.
                grid.contains(c)
                    || grid.contains(c + IVec2::X) || grid.contains(c + IVec2::NEG_X)
                    || grid.contains(c + IVec2::Y) || grid.contains(c + IVec2::NEG_Y)
            });

        // === EJECT: gas charge only. Motor cold, seeker caged. ===
        let ejecting = missile.eject_time > 0.0;
        if let Some(mut built) = built {
            if ejecting {
                missile.eject_time -= dt;
            } else if fly_built(&mut missile, &mut built, &mut velocity, pos, dt, in_channel, &target_query, ships_seen, blocks_seen, player_aim, decoys) {
                // Stalled: out of speed and out of the fight.
                commands.entity(entity).try_despawn();
                continue;
            }
        } else if ejecting {
            missile.eject_time -= dt;
        } else {
            // === BURN: motor lit. ===
            if missile.burn_fuel > 0.0 {
                missile.burn_fuel -= dt;
                let forward = velocity.0.normalize_or_zero();
                velocity.0 += forward * missile.thrust * dt;
                let speed = velocity.0.length();
                if speed > MISSILE_MAX_SPEED {
                    velocity.0 *= MISSILE_MAX_SPEED / speed;
                }
            }

            // === GUIDANCE: proportional navigation. ===
            //
            // PN steers to null the rotation rate of the line of sight rather
            // than to point at the target, which is what produces a lead
            // curve instead of the tail-chase the old cross-product
            // controller flew. Commanded lateral acceleration is
            // N x (LOS rate) x (closing speed).
            if missile.reserve_fuel > 0.0 && missile.max_lateral > 0.0 && !in_channel {
                if let Some(target) = missile.target {
                    if let Ok((target_tf, target_vel)) = target_query.get(target) {
                        let speed = velocity.0.length();
                        let los = target_tf.translation.truncate() - pos;
                        let range = los.length();
                        if speed > 1.0 && range > 1.0 {
                            let los_dir = los / range;
                            let rel_vel = target_vel.map(|v| v.0).unwrap_or(Vec2::ZERO) - velocity.0;
                            let closing = -rel_vel.dot(los_dir);
                            let cap = if range < missile.terminal_range {
                                missile.max_lateral * TERMINAL_AGILITY_BONUS
                            } else {
                                missile.max_lateral
                            };
                            let max_turn = (cap / speed).min(turn_ceiling(missile.age));

                            let turn_rate = if closing > 1.0 {
                                let los_rate = los_dir.perp_dot(rel_vel) / range;
                                let lateral = PN_GAIN * los_rate * closing;
                                (lateral / speed).clamp(-max_turn, max_turn)
                            } else {
                                // Overshot, or the target is outrunning us. PN
                                // has no closing speed to work with and would
                                // command nothing, leaving the missile to fly
                                // off into the dark. Fall back to pure pursuit
                                // — proportional on heading error, so it hooks
                                // round hard when the target is behind it and
                                // gently when it is nearly lined up.
                                let heading = velocity.0 / speed;
                                let angle_err = heading.perp_dot(los_dir)
                                    .atan2(heading.dot(los_dir));
                                (angle_err * PURSUIT_GAIN).clamp(-max_turn, max_turn)
                            };

                            let turn = turn_rate * dt;
                            let heading = velocity.0.y.atan2(velocity.0.x) + turn;
                            velocity.0 = Vec2::new(heading.cos(), heading.sin()) * speed;
                            // Every radian of correction is spent authority.
                            missile.reserve_fuel -= turn.abs();
                        }
                    }
                }
            }
        }

        // No gravity term here: celestial::gravity::apply_gravity_to_velocity
        // already integrates GravityForce into Velocity for everything
        // GravityAffected. Adding it again pulled missiles down at twice the
        // field.

        // Clear of its tube, it never turns back through its own ship.
        if !ejecting && !in_channel {
            if let Some((ship_gt, grid)) = missile.owner_ship.and_then(|s| hulls.get(s).ok()) {
                let speed = velocity.0.length();
                let to_target = missile.target.and_then(|t| target_query.get(t).ok()).map(|(tf, _)| tf.translation.truncate().distance(pos));
                let look = (speed * 1.5).min(to_target.unwrap_or(f32::MAX)) / crate::building::GRID_SIZE;
                let from = crate::building::world_to_cell(ship_gt, pos);
                let ship_turn = ship_gt.to_scale_rotation_translation().1;
                let local_dir = (ship_turn.inverse() * velocity.0.normalize_or_zero().extend(0.0)).truncate();
                if let Some(d) = clear_of_home(grid, from, local_dir, look) {
                    velocity.0 = (ship_turn * d.extend(0.0)).truncate() * speed;
                }
            }
        }

        transform.translation.x += velocity.0.x * dt;
        transform.translation.y += velocity.0.y * dt;

        // Point down the TUBE while the gas charge carries it out, and along
        // the flight path once the motor and seeker take over. Facing purely
        // by velocity made a missile launched from a moving ship slide out
        // sideways: it inherits the hull's speed, which at anything above a
        // crawl swamps the 200 u/s eject and leaves the sprite pointing where
        // the ship is going rather than where the silo is aimed.
        let facing = if ejecting || in_channel { missile.launch_dir } else { velocity.0 };
        if facing.length_squared() > 1.0e-4 {
            transform.rotation = Quat::from_rotation_z(facing.y.atan2(facing.x));
        }
    }
}

/// What a block can take from a warhead: health off a module or hull plate,
/// and how much it actually lost.
fn take(
    e: Entity,
    amount: f32,
    modules: &mut Query<(&mut Module, &GlobalTransform), Without<DestroyedModule>>,
    hulls: &mut Query<(&mut HullSegment, &GlobalTransform), Without<HullDestroyed>>,
) -> f32 {
    if amount <= 0.0 {
        return 0.0;
    }
    if let Ok((mut m, _)) = modules.get_mut(e) {
        let dealt = amount.min(m.health);
        m.health -= dealt;
        return dealt;
    }
    if let Ok((mut h, _)) = hulls.get_mut(e) {
        let dealt = amount.min(h.health);
        h.health -= dealt;
        return dealt;
    }
    0.0
}

/// Whether any block of this ship is within `radius` of `at` (`cell` is the
/// cell `at` falls in): a proximity fuse's test.
fn near_a_block(grid: &crate::building::ShipGrid, ship_gt: &GlobalTransform, cell: IVec2, at: Vec2, radius: f32) -> bool {
    let r = (radius / 66.0).ceil() as i32;
    (-r..=r).any(|dx| (-r..=r).any(|dy| {
        let c = cell + IVec2::new(dx, dy);
        grid.get(c).is_some()
            && ship_gt.transform_point(crate::building::grid_to_local(c).extend(0.0)).truncate().distance(at) < radius
    }))
}

/// A designed missile going off at `at` (combat::missile_design's rules):
/// HE takes 70% off plate and all of it off what's inside; frag 30% off
/// plate and 130% off what's inside and off weapon mounts; a shaped charge
/// is a jet straight on through the blocks behind, losing 8% a block; EMP
/// switches blocks off. A burst near the hull rather than on it is 85%
/// against plate and wastes most of a shaped charge. Returns the damage done
/// and the hull plates it touched.
#[allow(clippy::too_many_arguments)]
fn burst_built(
    commands: &mut Commands,
    at: Vec2,
    heading: Vec2,
    s: &super::missile_design::MissileStats,
    contact: Option<Entity>,
    children: &Children,
    modules: &mut Query<(&mut Module, &GlobalTransform), Without<DestroyedModule>>,
    hulls: &mut Query<(&mut HullSegment, &GlobalTransform), Without<HullDestroyed>>,
    blocks: &Query<&crate::building::Block>,
    grid: &crate::building::ShipGrid,
    ship_gt: &GlobalTransform,
) -> (f32, Vec<Entity>) {
    use crate::components::ModuleCategory;
    let airburst = contact.is_none();
    let mut total = 0.0;
    let mut plates = Vec::new();
    // Where each block is and what it is: plate, a weapon mount, or inside.
    let mut nearby: Vec<(Entity, f32, bool, bool)> = Vec::new();
    let reach = s.he_radius.max(s.frag_radius).max(s.emp_radius);
    for c in children.iter() {
        let (pos, plate, mount) = if let Ok((m, gt)) = modules.get(c) {
            let thick = blocks.get(c).map(|b| b.thickness).unwrap_or(0.0);
            let cat = m.module_type.category();
            (gt.translation().truncate(), thick > 0.0, matches!(cat, ModuleCategory::Weapons | ModuleCategory::Guns))
        } else if let Ok((_, gt)) = hulls.get(c) {
            (gt.translation().truncate(), true, false)
        } else {
            continue;
        };
        let d = pos.distance(at);
        if d < reach {
            nearby.push((c, d, plate, mount));
        }
    }
    let plate_share = |base: f32| if airburst { base * 0.85 } else { base };
    for &(e, d, plate, mount) in &nearby {
        let mut dmg = 0.0;
        if s.he_damage > 0.0 && d < s.he_radius {
            dmg += s.he_damage * (1.0 - d / s.he_radius) * if plate { plate_share(0.7) } else { 1.0 };
        }
        if s.frag_damage > 0.0 && d < s.frag_radius {
            let mul = if mount { 1.3 } else if plate { plate_share(0.3) } else { 1.3 };
            dmg += s.frag_damage * (1.0 - d / s.frag_radius) * mul;
        }
        if dmg > 0.0 && hulls.contains(e) {
            plates.push(e);
        }
        total += take(e, dmg, modules, hulls);
        if s.emp_radius > 0.0 && d < s.emp_radius && modules.contains(e) {
            commands.entity(e).try_insert(super::energy_weapons::IonDisabled { timer: super::missile_design::EMP_SECONDS });
        }
    }
    if s.shaped_damage > 0.0 {
        // The jet carries on along the missile's line, block after block.
        let mut energy = s.shaped_damage * if airburst { 0.3 } else { 1.0 };
        let mut seen: Vec<Entity> = Vec::new();
        for step in 0..80 {
            if energy < 5.0 {
                break;
            }
            let p = at + heading * (step as f32 * 33.0);
            let here = crate::building::world_to_cell(ship_gt, p);
            let Some(e) = grid.get(IVec2::new(here.x.round() as i32, here.y.round() as i32)) else { continue };
            if seen.contains(&e) {
                continue;
            }
            seen.push(e);
            if hulls.contains(e) {
                plates.push(e);
            }
            let dealt = take(e, energy, modules, hulls);
            total += dealt;
            energy = (energy - dealt) * 0.92;
        }
    }
    if s.he_damage + s.frag_damage + s.shaped_damage == 0.0 && s.emp_radius == 0.0 {
        if let Some(e) = contact {
            total += take(e, s.ram_damage * s.top_speed / 1000.0, modules, hulls);
        }
    }
    (total, plates)
}

/// Where a designed missile's seeker would put it on `target`: a heat seeker
/// on the engines, anything else on the middle of the ship.
fn seeker_point(
    head: super::missile_design::MissileHead,
    target: Entity,
    target_tf: &Transform,
    ships: &Query<(Option<&crate::combat::shields::ShipShield>, Option<&Children>), Without<MissileProjectile>>,
    blocks: &Query<(&Module, &GlobalTransform), Without<DestroyedModule>>,
) -> Vec2 {
    let Ok((shield, children)) = ships.get(target) else { return target_tf.translation.truncate() };
    if head == super::missile_design::MissileHead::Heat {
        let (mut sum, mut n) = (Vec2::ZERO, 0.0);
        for c in children.into_iter().flatten() {
            if let Ok((m, gt)) = blocks.get(*c) {
                if m.health > 0.0 && m.module_type.category() == crate::components::ModuleCategory::Propulsion {
                    sum += gt.translation().truncate();
                    n += 1.0;
                }
            }
        }
        if n > 0.0 {
            return sum / n;
        }
    }
    shield.map(|s| s.world_center(target_tf)).unwrap_or(target_tf.translation.truncate())
}

/// One frame of a designed missile under power and guidance. Returns true
/// when it has stalled and should go.
#[allow(clippy::too_many_arguments)]
fn fly_built(
    missile: &mut MissileProjectile,
    built: &mut BuiltMissile,
    velocity: &mut Velocity,
    pos: Vec2,
    dt: f32,
    in_channel: bool,
    targets: &Query<(&Transform, Option<&Velocity>), Without<MissileProjectile>>,
    ships: &Query<(Option<&crate::combat::shields::ShipShield>, Option<&Children>), Without<MissileProjectile>>,
    blocks: &Query<(&Module, &GlobalTransform), Without<DestroyedModule>>,
    player_aim: &super::turret_deck::PlayerAimPoint,
    decoys: &Query<&Transform, (With<super::missile_defence::MissileDecoy>, Without<MissileProjectile>)>,
) -> bool {
    use super::missile_design::{MissileHead, Seeker, MAX_SPEED, STALL_SPEED};
    // Fooled: it chases the decoy, and is spent when it gets there or the
    // decoy burns out.
    if let Some(d) = built.decoy {
        match decoys.get(d) {
            Ok(tf) if tf.translation.truncate().distance(pos) < 60.0 => return true,
            Ok(_) => {}
            Err(_) => {
                built.decoy = None;
                built.locked = false;
                built.launch_aim = pos + velocity.0.normalize_or(Vec2::X) * 4000.0;
            }
        }
    }
    let s = built.stats;
    let mut speed = velocity.0.length();
    let mut heading = velocity.0.y.atan2(velocity.0.x);
    let burning = missile.burn_fuel > 0.0;
    if burning {
        missile.burn_fuel -= dt;
        speed = (speed + missile.thrust * dt).min(MAX_SPEED);
    } else {
        speed *= (-s.drag * dt).exp();
        if speed < STALL_SPEED {
            return true;
        }
    }
    if built.head != MissileHead::DumbNose && !in_channel {
        // What the seeker sees, and whether it has it.
        let seen = missile.target.and_then(|t| targets.get(t).ok().map(|(tf, v)| (t, tf, v)));
        let target_point = seen.map(|(t, tf, v)| (seeker_point(built.head, t, tf, ships, blocks), v.map(|v| v.0).unwrap_or(Vec2::ZERO)));
        match (s.seeker, target_point) {
            (Seeker::Heat { range, cone_deg } | Seeker::Radar { range, cone_deg }, Some((at, _))) => {
                let off = Vec2::from_angle(heading).angle_to(at - pos).abs().to_degrees();
                let dist = pos.distance(at);
                if !built.locked && dist < range && off < cone_deg {
                    built.locked = true;
                } else if built.locked && off > cone_deg + 15.0 {
                    // Overshot: lost it. Carry on the way it was going.
                    built.locked = false;
                    built.launch_aim = pos + Vec2::from_angle(heading) * 4000.0;
                }
            }
            _ => {}
        }
        let mut aim = built.launch_aim;
        if let Some(tf) = built.decoy.and_then(|d| decoys.get(d).ok()) {
            aim = tf.translation.truncate();
        } else if built.head == MissileHead::CommandLink {
            // Your aim, if it's yours; the target, if it's an AI's.
            let point = if built.player { player_aim.first.or(player_aim.then) } else { None }
                .or(target_point.map(|(at, _)| at));
            if let Some(p) = point {
                // Looser the farther it has flown from the ship guiding it.
                let spread = 30.0 + missile.traveled * 0.02;
                built.wander += Vec2::new(rand::random::<f32>() - 0.5, rand::random::<f32>() - 0.5) * spread * dt * 3.0;
                built.wander *= 0.98;
                aim = p + built.wander;
            }
        } else if built.locked {
            if let Some((at, vel)) = target_point {
                let tgo = (pos.distance(at) / speed.max(500.0)).min(3.0);
                aim = at + vel * tgo * 0.9;
            }
        }
        let want = (aim - pos).to_angle();
        let err = (want - heading + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        let lateral = if burning { s.lateral_burning } else { s.lateral };
        let cap = turn_ceiling(missile.age).min(lateral / speed.max(150.0));
        heading += (err * 4.0).clamp(-cap, cap) * dt;
    }
    velocity.0 = Vec2::from_angle(heading) * speed;
    false
}

/// Largest creature hit radius (Leviathan) — used to size the spatial grid query margin.
const MAX_CREATURE_HIT_RADIUS: f32 = 100.0;

/// Check missile hits — armed missiles explode on contact.
/// Uses the creature spatial grid to only distance-check nearby creatures.
/// Is `cell` a genuine obstruction in the tube that fired from `launch_cell`
/// along `axis`?
///
/// True only for cells on the tube's axis, ahead of the mouth. Everything
/// else the missile touches on its way out is off to one side, and the side
/// of a tube guides a warhead rather than stopping it.
fn obstructs_tube(launch_cell: IVec2, axis: IVec2, cell: IVec2) -> bool {
    if axis == IVec2::ZERO {
        return false;
    }
    let delta = cell - launch_cell;
    let along = delta.x * axis.x + delta.y * axis.y;
    along > 0 && delta == axis * along
}

/// How much harder a warhead hits STRUCTURE than its listed damage.
///
/// Designed hulls carry 100 HP per block times the material (steel 100,
/// titanium 150, composite 200, abyssal 300), and the launchers list 75 for
/// a Heavy, 44 for a Guided and 19 per Cluster rocket. So at the listed
/// figure no missile could destroy even ONE steel plate at the dead centre of
/// its own blast -- a "devastating" heavy left a block at a quarter health
/// and its neighbours barely scratched, which reads as doing nothing at all.
///
/// Tripled, a Heavy guts a three-by-three of steel and dents the ring around
/// it; a Guided kills the steel block it lands on. Applied here and not in
/// the registry on purpose: the registry figure also feeds the player's own
/// damage pipeline when ENEMY missiles hit you, and raising it there would
/// make every enemy launcher three times as deadly too.
const WARHEAD_VS_STRUCTURE: f32 = 3.0;

/// Blast radius for a launcher with `bays` bays (`size_mult`, never under 1).
///
/// One function, used by the launcher and by the tests, so the tests check
/// the radius the game actually uses. Was `30 + bays * 20` -- 50 units for a
/// single bay against a 66-unit cell, unable to reach the block beside the
/// one it hit -- then `60 + bays * 40`. Now about three cells for a single
/// bay. A feel number: raise it for a heavier warhead, lower it for precision.
pub(crate) fn blast_radius_for(size_mult: f32) -> f32 {
    140.0 + size_mult.max(1.0) * 60.0
}

/// How much of a warhead one block `d` away takes.
///
/// Linear falloff across the blast: at the rim a warhead scorches, at the
/// centre it guts. Armour gets a say through the same `resolve_impact` the
/// guns use -- this used to apply the FULL warhead to every block in range
/// regardless of plating or distance, which made a heavily belted Pressure
/// King no harder to crack than a bare Rust Swarm.
///
/// Pass-through is high because a shaped charge is meant to defeat plate
/// that stops a bullet. High, not total: plating you stand behind should
/// still be worth having.
///
/// A detonation has no incoming angle to speak of -- it envelops the block
/// rather than striking a face -- so it is always head-on and never skips.
fn blast_damage(warhead: f32, d: f32, radius: f32, block: &crate::building::Block) -> f32 {
    const WARHEAD_PASS_THROUGH: f32 = 0.65;
    if d >= radius || radius <= 0.0 {
        return 0.0;
    }
    let share = warhead * WARHEAD_VS_STRUCTURE * (1.0 - d / radius);
    let impact = crate::combat::impact::resolve_impact(
        share,
        block,
        1.0,
        crate::combat::impact::Obliquity::HEAD_ON,
        Some(WARHEAD_PASS_THROUGH),
    );
    (impact.to_block + impact.through).max(0.0)
}

pub fn check_missile_hits(
    mut commands: Commands,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    // Without<EmpWarhead>: an EMP round is a delivery vehicle, and its fuse
    // lives in energy_weapons::emp_detonation. Resolved here it went off as a
    // 5-damage plain warhead the moment it reached the target's bubble, so
    // the disable never happened at all.
    missile_query: Query<
        (Entity, &MissileProjectile, &Transform, Option<&BuiltMissile>),
        Without<super::energy_weapons::EmpWarhead>,
    >,
    mut creature_query: Query<(&Transform, &mut Creature), Without<Ship>>,
    creature_grid: Res<crate::spatial::CreatureGrid>,
    mut ai_ship_query: Query<
        (Entity, &Transform, &Children, &mut crate::combat::shields::ShipShield),
        With<crate::ai_ship::components::AiShip>,
    >,
    mut ai_module_query: Query<(&mut Module, &GlobalTransform), Without<DestroyedModule>>,
    // Hull is a separate component from Module, and the blast only ever
    // looked at Modules — so a warhead striking an enemy's armour belt did
    // nothing at all, and was not even consumed. See the walk below.
    mut ai_hull_query: Query<(&mut HullSegment, &GlobalTransform), Without<HullDestroyed>>,
    block_query: Query<&crate::building::Block>,
    owner_parent_query: Query<&ChildOf>,
    mut ai_damage_events: MessageWriter<crate::events::AiShipDamaged>,
    mut notifications: MessageWriter<ShowNotification>,
    parent_hulls: Query<(&GlobalTransform, &crate::building::ShipGrid)>,
    // What a hit on the player's ship tells the rest of the game: the hit,
    // and any hull it holed. One tuple: this system is at Bevy's ceiling.
    mut player_hit: (
        MessageWriter<crate::events::ShipDamaged>,
        MessageWriter<crate::events::HullBreached>,
        MessageWriter<crate::events::RoomDepressurized>,
        Res<crate::building::rooms::RoomMap>,
    ),
    // The player's shield rides along here (a 16th parameter would hit
    // Bevy's ceiling). Without<AiShip> is what lets this &mut ShipShield sit
    // beside ai_ship_query's.
    mut player_query: Query<
        (Entity, &Transform, Option<&mut crate::combat::shields::ShipShield>, Option<&Children>),
        (With<Ship>, Without<crate::ai_ship::components::AiShip>),
    >,
) {
    'missiles: for (missile_entity, missile, missile_transform, built) in missile_query.iter() {
        // === HULLS ===
        //
        // Missiles are the exception to the rule that a round ignores the
        // ship that fired it. A warhead is a physical object leaving a tube
        // at walking pace, and flying it through armour looks absurd in a way
        // a hypersonic slug does not. The same walk serves both ends of that:
        // against its OWN hull it is the cook-off the silo warning promises,
        // and against the player's it is how an enemy missile lands at all —
        // the blast branch below only ever looks at AI ships.
        //
        // Checked BEFORE arming: the whole point is the ones that never get
        // clear of the tube.
        let player_ship = player_query.single().ok().map(|(e, _, _, _)| e);
        let missile_pos_now = missile_transform.translation.truncate();
        for ship in [missile.owner_ship, player_ship].into_iter().flatten() {
            let Ok((ship_gt, grid)) = parent_hulls.get(ship) else { continue };
            let own = missile.owner_ship == Some(ship);

            // An enemy warhead meets your shield before your hull, the same
            // way an enemy round does (projectiles::projectile_collision).
            // The bubble stands well clear of the plating, so this is tested
            // before the cell lookup below -- which only ever fires once the
            // missile is already inside a block. Without it an enemy missile
            // flew straight through a live shield into the hull.
            if !own && Some(ship) == player_ship {
                if let Ok((_, ship_tf, Some(mut shield), _)) = player_query.single_mut() {
                    let center = shield.world_center(ship_tf);
                    if shield.is_up()
                        && missile_pos_now.distance(center) < shield.radius
                        && shield.covers_arc(missile_pos_now - center)
                    {
                        shield.absorb(missile.damage);
                        spawn_explosion(&mut commands, &fx, missile_pos_now, missile.blast_radius * 0.7, Color::srgb(0.5, 0.8, 1.0));
                        commands.entity(missile_entity).try_despawn();
                        continue 'missiles;
                    }
                }
            }

            let here = crate::building::world_to_cell(ship_gt, missile_pos_now);
            let cell = IVec2::new(here.x.round() as i32, here.y.round() as i32);

            // An enemy's designed missile on the player's ship: its own fuse
            // and warheads, the same as one of ours on theirs.
            if let Some(built) = built.filter(|_| !own) {
                let s = &built.stats;
                let contact = grid.get(cell);
                let fused = contact.is_some() || (s.proximity && near_a_block(grid, ship_gt, cell, missile_pos_now, s.fuse_radius));
                if !fused { continue; }
                let Ok((_, _, _, Some(children))) = player_query.get(ship) else { continue };
                let heading = (missile_pos_now - missile.prev_pos).normalize_or(missile.launch_dir);
                let (dealt, hulls_hit) = burst_built(
                    &mut commands, missile_pos_now, heading, s, contact, children,
                    &mut ai_module_query, &mut ai_hull_query, &block_query, grid, ship_gt,
                );
                // Holes in the plating let the air out, as a gun's would.
                for h in hulls_hit {
                    let Ok((mut hull, _)) = ai_hull_query.get_mut(h) else { continue };
                    let pct = if hull.max_health > 0.0 { hull.health / hull.max_health } else { 0.0 };
                    if pct < 0.3 && !hull.is_depressurized {
                        hull.is_depressurized = true;
                        player_hit.1.write(crate::events::HullBreached { segment: h, severity: 1.0 - pct });
                        if let Some(&room_id) = player_hit.3.tile_to_room.get(&hull.grid_position) {
                            player_hit.2.write(crate::events::RoomDepressurized { room_id, severity: 1.0 - pct });
                        }
                    }
                }
                spawn_explosion(&mut commands, &fx, missile_pos_now, s.he_radius.max(s.frag_radius).max(s.emp_radius).max(50.0), Color::srgb(1.0, 0.45, 0.1));
                player_hit.0.write(crate::events::ShipDamaged {
                    source: crate::events::DamageSource::Explosion,
                    amount: dealt,
                    position: Some(missile_pos_now),
                    direction: Some((missile.prev_pos - missile_pos_now).normalize_or_zero()),
                    applied: true,
                });
                notifications.write(ShowNotification {
                    message: format!("MISSILE IMPACT -{dealt:.0}"),
                    notification_type: NotificationType::Danger,
                    duration: 2.5,
                });
                commands.entity(missile_entity).try_despawn();
                continue 'missiles;
            }

            let Some(hit) = grid.get(cell) else { continue };

            if own {
                // Not the tube it is leaving, and not its own plating: one
                // course of armour is a blow-out panel, exactly as it is for
                // the build-mode silo check. Identified by CELL, because
                // update_ship_grids writes hull after modules and a launcher
                // on a hull tile resolves to the hull segment.
                if cell == missile.launch_cell || hit == missile.owner {
                    continue;
                }
                if ai_module_query.get(hit).is_ok_and(|(m, _)| crate::building::is_blowout_panel(m.module_type)) {
                    continue;
                }
                // Only something DEAD AHEAD stops it. Blocks to the left and
                // right of a tube guide a missile rather than stopping it --
                // `move_missiles` says exactly that and holds the heading
                // while the warhead is still threading its own ship, but this
                // check never honoured it, so any cell the missile touched
                // cooked it off.
                //
                // It touches plenty. The missile keeps the world-space
                // velocity it launched with while the ship goes on turning and
                // thrusting underneath it, so the hull swings across a warhead
                // that is flying perfectly straight down its own tube. That
                // was 3 launches in 22 on the old hull and 8 in 30 on this
                // one: a weapon that blew up inside the player roughly one
                // time in five, with a warning that read like a malfunction
                // rather than a manoeuvre.
                //
                // An obstruction is a cell on the tube's axis, ahead of the
                // mouth. Anything off-axis is drift, and drift is the ship's
                // doing, not the layout's -- and the layout is already
                // guaranteed clear by `building::entombed_launchers`.
                if !obstructs_tube(missile.launch_cell, missile.launch_axis, cell) {
                    continue;
                }
            }

            spawn_explosion(&mut commands, &fx, missile_pos_now, missile.blast_radius, Color::srgb(1.0, 0.45, 0.1));
            // Own cook-off only. An ENEMY warhead's damage goes through the
            // ShipDamaged event below, which process_ship_damage applies;
            // hitting the module here as well charged every enemy missile
            // that landed on one of your modules twice.
            if own {
                if let Ok((mut module, _)) = ai_module_query.get_mut(hit) {
                    module.health = (module.health - missile.damage).max(0.0);
                }
            }

            if Some(ship) == player_ship {
                // Direction points from the ship TOWARD the attacker — the
                // convention process_ship_damage walks inward along.
                let from = (missile.prev_pos - missile_pos_now).normalize_or_zero();
                player_hit.0.write(crate::events::ShipDamaged {
                    source: crate::events::DamageSource::Explosion,
                    // Real amount when it is an ENEMY warhead: the player's
                    // damage pipeline applies it, and nothing else does. Zero
                    // for our own cook-off, which was applied to the block
                    // above.
                    amount: if own { 0.0 } else { missile.damage },
                    position: Some(missile_pos_now),
                    direction: Some(from),
                    applied: false,
                });
                notifications.write(ShowNotification {
                    message: if own {
                        "SILO OBSTRUCTED - WARHEAD COOK-OFF".into()
                    } else {
                        "MISSILE IMPACT".to_string()
                    },
                    notification_type: NotificationType::Danger,
                    duration: 2.5,
                });
            }
            commands.entity(missile_entity).try_despawn();
            continue 'missiles;
        }

        if !missile.armed { continue; }

        let missile_pos = missile_transform.translation.truncate();
        // A weapon's own ship is never a valid target for its own missile.
        let owner_ship = owner_parent_query.get(missile.owner).ok().map(|p| p.parent());

        // === AI SHIPS: shield absorbs the warhead; shield down = blast
        // damage to every block inside the blast radius ===
        for (ai_entity, ai_transform, children, mut shield) in ai_ship_query.iter_mut() {
            if Some(ai_entity) == owner_ship { continue; }
            let center = shield.world_center(ai_transform);
            let dist_to_ship = missile_pos.distance(center);

            if shield.is_up() && dist_to_ship < shield.radius && shield.covers_arc(missile_pos - center) {
                shield.absorb(missile.damage);
                spawn_explosion(&mut commands, &fx, missile_pos, missile.blast_radius * 0.7, Color::srgb(0.5, 0.8, 1.0));
                commands.entity(missile_entity).try_despawn();
                continue 'missiles;
            }

            // A designed missile's own fuse and warheads (missile_design).
            if let Some(built) = built {
                let Ok((ship_gt, grid)) = parent_hulls.get(ai_entity) else { continue };
                let s = &built.stats;
                let here = crate::building::world_to_cell(ship_gt, missile_pos);
                let cell = IVec2::new(here.x.round() as i32, here.y.round() as i32);
                let contact = grid.get(cell);
                let fused = contact.is_some()
                    || (s.proximity && dist_to_ship < shield.radius + s.fuse_radius && near_a_block(grid, ship_gt, cell, missile_pos, s.fuse_radius));
                if !fused { continue; }
                let heading = (missile_pos - missile.prev_pos).normalize_or(missile.launch_dir);
                let (dealt, _) = burst_built(
                    &mut commands, missile_pos, heading, s, contact, children,
                    &mut ai_module_query, &mut ai_hull_query, &block_query, grid, ship_gt,
                );
                let radius = s.he_radius.max(s.frag_radius).max(s.emp_radius).max(50.0);
                let colour = if s.emp_radius > 0.0 && s.he_damage + s.frag_damage == 0.0 { Color::srgb(0.48, 0.66, 1.0) } else { Color::srgb(1.0, 0.5, 0.1) };
                spawn_explosion(&mut commands, &fx, missile_pos, radius, colour);
                if dealt > 0.0 {
                    spawn_floating_damage(&mut commands, missile_pos, dealt, Color::srgb(1.0, 0.4, 0.1));
                }
                if super::gun_assembly::trace() {
                    info!("MISSILE burst on {:?} ({}): {:.0} damage", ai_entity, if contact.is_some() { "contact" } else { "proximity" }, dealt);
                }
                ai_damage_events.write(crate::events::AiShipDamaged {
                    target: ai_entity,
                    source: crate::events::DamageSource::Explosion,
                    amount: 0.0,
                    position: Some(missile_pos),
                    direction: None,
                    attacker: owner_ship,
                });
                commands.entity(missile_entity).try_despawn();
                continue 'missiles;
            }

            // Contact: is the warhead inside a LIVE block of this ship?
            // `update_ship_grids` maintains a ShipGrid for AI ships exactly
            // as it does for the player, so the same cell lookup the
            // own-hull cook-off uses above works here. Without this a
            // missile only ever detonated if a Module happened to fall
            // inside a blast radius smaller than one 66-unit cell -- so it
            // flew through armour plate, and through whole ships, untouched.
            let contact = parent_hulls.get(ai_entity).ok().is_some_and(|(gt, grid)| {
                let here = crate::building::world_to_cell(gt, missile_pos);
                grid.get(IVec2::new(here.x.round() as i32, here.y.round() as i32)).is_some()
            });

            if contact || dist_to_ship < shield.radius + 60.0 {
                // Blast damage, but armour gets a say.
                //
                // This used to apply the FULL warhead to every module inside
                // the radius, ignoring plating, hull and distance alike — the
                // one place in combat where armour did not matter, so the
                // answer to a heavily belted Corpse Stars was the same as
                // for a bare Recursive Kingdom. Now each block soaks by its own
                // thickness through the same resolve_impact the guns use,
                // and damage falls off across the radius.
                //
                // Pass-through is high: a shaped charge is meant to defeat
                // plate that stops a bullet. High, not total — plating you
                // stand behind should still be worth having.
                let radius = missile.blast_radius.max(50.0);
                let mut total_damage = 0.0;
                let mut hit_any = false;
                for child in children.iter() {
                    // Hull first, then modules. Both are solid, both are
                    // children of the ship, and a warhead does not care
                    // which component a block happens to carry.
                    if let Ok((mut hull, gt)) = ai_hull_query.get_mut(child) {
                        let block = block_query.get(child).copied()
                            .unwrap_or_else(|_| crate::building::Block::module(IVec2::ZERO));
                        let dealt = blast_damage(
                            missile.damage,
                            missile_pos.distance(gt.translation().truncate()),
                            radius,
                            &block,
                        );
                        if dealt > 0.0 {
                            hull.health = (hull.health - dealt).max(0.0);
                            total_damage += dealt;
                            hit_any = true;
                        }
                        continue;
                    }
                    if let Ok((mut module, gt)) = ai_module_query.get_mut(child) {
                        let d = missile_pos.distance(gt.translation().truncate());
                        if d >= radius { continue; }
                        let block = block_query.get(child).copied()
                            .unwrap_or_else(|_| crate::building::Block::module(IVec2::ZERO));
                        let dealt = blast_damage(missile.damage, d, radius, &block);
                        if dealt <= 0.0 { continue; }
                        module.health = (module.health - dealt).max(0.0);
                        total_damage += dealt;
                        hit_any = true;
                    }
                }
                if hit_any {
                    spawn_explosion(&mut commands, &fx, missile_pos, missile.blast_radius, Color::srgb(1.0, 0.5, 0.1));
                    spawn_floating_damage(&mut commands, missile_pos, total_damage, Color::srgb(1.0, 0.4, 0.1));
                    // amount: 0.0 — damage already applied directly above to
                    // every module in the blast radius. process_ai_ship_damage_system
                    // used to re-apply this same total again via its own
                    // distance-sort, double-damaging and often hitting
                    // different blocks than the ones actually in the blast.
                    ai_damage_events.write(crate::events::AiShipDamaged {
                        target: ai_entity,
                        source: crate::events::DamageSource::Explosion,
                        amount: 0.0,
                        position: Some(missile_pos),
                        direction: None,
                        attacker: owner_ship,
                    });
                    commands.entity(missile_entity).try_despawn();
                    continue 'missiles;
                }
            }
        }

        for (creature_entity, _) in creature_grid.0.nearby(missile_pos, MAX_CREATURE_HIT_RADIUS) {
            let Ok((creature_transform, mut creature)) = creature_query.get_mut(creature_entity) else { continue };
            if creature.health <= 0.0 { continue; }

            let creature_pos = creature_transform.translation.truncate();
            let dist = missile_pos.distance(creature_pos);
            let hit_radius = match creature.creature_type {
                CreatureType::Leviathan => 100.0,
                _ => 40.0,
            };

            if dist > hit_radius { continue; }

            // IMPACT!
            // Direct hit damage
            creature.health -= missile.damage;

            // Blast radius damage to nearby creatures
            // (handled by the explosion effect — could expand later)

            // Explosion visual
            spawn_explosion(&mut commands, &fx, missile_pos, missile.blast_radius, Color::srgb(1.0, 0.5, 0.1));
            spawn_floating_damage(&mut commands, missile_pos, missile.damage, Color::srgb(1.0, 0.3, 0.1));

            commands.entity(missile_entity).try_despawn();
            break;
        }
    }
}

#[cfg(test)]
mod silo_tests {
    use super::*;

    const UP: IVec2 = IVec2::new(0, 1);

    /// Something in the tube stops the warhead. That is the mechanic.
    #[test]
    fn a_block_down_the_tube_is_an_obstruction() {
        let mouth = IVec2::new(5, 5);
        assert!(obstructs_tube(mouth, UP, IVec2::new(5, 6)));
        assert!(obstructs_tube(mouth, UP, IVec2::new(5, 9)));
    }

    /// The sides of a tube guide it. `move_missiles` has always said so --
    /// blocks left and right do not stop a missile, it keeps going until it is
    /// clear -- but the collision check cooked the warhead off on any cell it
    /// touched, including ones it was never aimed at.
    #[test]
    fn the_walls_of_the_tube_are_not_an_obstruction() {
        let mouth = IVec2::new(5, 5);
        for beside in [
            IVec2::new(4, 6),
            IVec2::new(6, 6),
            IVec2::new(4, 5),
            IVec2::new(6, 5),
            IVec2::new(3, 9),
        ] {
            assert!(
                !obstructs_tube(mouth, UP, beside),
                "{beside:?} is beside the tube, not in it"
            );
        }
    }

    /// And nothing behind it counts. A ship that turns or accelerates while a
    /// warhead is still leaving swings its own hull across a missile flying
    /// perfectly straight; before this rule that was eight cook-offs in thirty
    /// launches, inside the player's own ship.
    #[test]
    fn the_hull_swinging_past_behind_is_not_an_obstruction() {
        let mouth = IVec2::new(5, 5);
        assert!(!obstructs_tube(mouth, UP, IVec2::new(5, 4)));
        assert!(!obstructs_tube(mouth, UP, mouth));
    }

    /// A launcher with no axis recorded must never cook off, or an old save
    /// or an untouched code path becomes a bomb in the player's hull.
    #[test]
    fn a_missing_axis_never_detonates() {
        assert!(!obstructs_tube(IVec2::ZERO, IVec2::ZERO, IVec2::new(0, 1)));
    }
}

#[cfg(test)]
mod warhead_tests {
    use super::*;
    use crate::building::Block;
    use crate::components::HullMaterial;

    fn steel_plate() -> Block {
        Block::hull(IVec2::ZERO, HullMaterial::Steel)
    }

    /// The report that started this: missiles appeared to do nothing.
    ///
    /// Hull plating is a `HullSegment`, and the blast walk only ever matched
    /// entities carrying a `Module` -- so a warhead that struck an enemy's
    /// armour belt dealt no damage at all, and was not even consumed. It kept
    /// flying until some module happened to fall inside a blast radius that
    /// was itself smaller than one 66-unit grid cell.
    ///
    /// This covers the arithmetic half; the walk now checks hull before
    /// modules and detonates on contact with any live cell of the target.
    #[test]
    fn a_warhead_hurts_armour_plate() {
        let dealt = blast_damage(75.0, 0.0, 120.0, &steel_plate());
        assert!(
            dealt > 0.0,
            "a 75-damage warhead detonating ON a steel plate dealt {dealt}"
        );
    }

    /// Falloff is linear across the blast, so the centre guts and the rim
    /// scorches. Both ends have to be real: zero at the centre would be the
    /// original bug, and full damage at the rim would make blast radius the
    /// only stat that matters.
    #[test]
    fn the_centre_guts_and_the_rim_scorches() {
        let centre = blast_damage(100.0, 0.0, 120.0, &steel_plate());
        let middle = blast_damage(100.0, 60.0, 120.0, &steel_plate());
        let rim = blast_damage(100.0, 119.0, 120.0, &steel_plate());
        assert!(centre > middle, "centre {centre} no worse than mid-blast {middle}");
        assert!(middle > rim, "mid-blast {middle} no worse than the rim {rim}");
        assert!(rim >= 0.0);
    }

    /// Nothing outside the blast. Without this the radius means nothing and
    /// every warhead is a ship-wide strike.
    #[test]
    fn nothing_outside_the_blast_is_touched() {
        assert_eq!(blast_damage(100.0, 120.0, 120.0, &steel_plate()), 0.0);
        assert_eq!(blast_damage(100.0, 5_000.0, 120.0, &steel_plate()), 0.0);
        assert_eq!(blast_damage(100.0, 0.0, 0.0, &steel_plate()), 0.0);
    }

    /// A blast has to reach past the block it lands on, or "radius" is
    /// decoration.
    ///
    /// `size_mult` is `bays.max(1)`, so one bay is the smallest launcher
    /// there is. Under the old `30 + size_mult * 20` that was a radius of
    /// 50 against a 66-unit cell — a single-bay warhead could not touch the
    /// block beside the one it hit, which is most of why missiles read as
    /// doing nothing.
    #[test]
    fn even_the_smallest_blast_reaches_a_neighbouring_block() {
        const CELL: f32 = 66.0;
        for bays in 1..=4u32 {
            let radius = blast_radius_for(bays as f32);
            assert!(
                radius > CELL,
                "{bays} bay(s): blast radius {radius} is under one {CELL}-unit cell, \
                 so the warhead cannot reach a neighbouring block"
            );
        }
        // And the old formula is what this replaced — one bay fell short.
        assert!(30.0 + 20.0 < CELL, "the old single-bay radius was not actually under a cell");
    }
}

#[cfg(test)]
mod missile_hits_blocks_tests {
    use super::*;
    use crate::ai_ship::components::AiShip;
    use crate::building::ShipGrid;
    use crate::combat::new_projectiles::MissileProjectile;
    use crate::combat::shields::ShipShield;
    use crate::components::{HullLayer, HullMaterial};
    use bevy::platform::collections::HashMap;

    /// A shield that is down, so the warhead reaches the hull — a missile into
    /// a live shield is absorbed by it, which is working as intended and not
    /// what this is testing.
    fn shield_down() -> ShipShield {
        ShipShield {
            current: 0.0,
            max: 0.0,
            base_max: 0.0,
            recharge_rate: 0.0,
            recharge_delay: 0.0,
            since_hit: 999.0,
            radius: 400.0,
            center_offset: Vec2::ZERO,
            flash: 0.0,
            enabled: true,
            surge: 0.0,
            arc_facing: 0.0,
            arc_half: std::f32::consts::PI,
            arc_traverse: 0.0,
            arc_active: false,
        }
    }

    fn plate(material: HullMaterial, cell: IVec2) -> HullSegment {
        let hp = 100.0 * material.health_multiplier();
        HullSegment {
            health: hp,
            max_health: hp,
            radiation_shielding: 0.0,
            is_depressurized: false,
            depressurization_level: 0.0,
            hull_layer: HullLayer::Outer,
            material,
            grid_position: cell,
        }
    }

    /// The world, the way the game builds it: an enemy ship at the origin with
    /// a `ShipGrid`, hull blocks as its children, and one armed missile sitting
    /// on cell (0, 0). Runs the real `check_missile_hits` — not the falloff
    /// maths in isolation, which passed last time while missiles still did
    /// nothing in the game.
    fn fire_into(blocks: &[(IVec2, HullMaterial)]) -> (App, Vec<Entity>) {
        let mut app = App::new();
        app.insert_resource(crate::vfx::effect_textures::EffectTextures::empty());
        app.init_resource::<crate::spatial::CreatureGrid>();
        app.add_message::<crate::events::AiShipDamaged>();
        app.add_message::<crate::events::ShipDamaged>();
        app.add_message::<crate::events::HullBreached>();
        app.add_message::<crate::events::RoomDepressurized>();
        app.init_resource::<crate::building::rooms::RoomMap>();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, check_missile_hits);

        let ship = app
            .world_mut()
            .spawn((AiShip, Transform::default(), GlobalTransform::default(), shield_down()))
            .id();

        let mut cells = HashMap::new();
        let mut ids = Vec::new();
        for &(cell, material) in blocks {
            let local = crate::building::grid_to_local(cell).extend(0.0);
            let id = app
                .world_mut()
                .spawn((
                    plate(material, cell),
                    Transform::from_translation(local),
                    GlobalTransform::from_translation(local),
                    ChildOf(ship),
                ))
                .id();
            cells.insert(cell, id);
            ids.push(id);
        }
        app.world_mut().entity_mut(ship).insert(ShipGrid { cells });

        // Launched by someone else, so it is not its own ship's missile.
        let shooter = app.world_mut().spawn_empty().id();
        let at = crate::building::grid_to_local(IVec2::ZERO);
        app.world_mut().spawn((
            MissileProjectile {
                damage: 75.0, // Heavy Missile Launcher, single bay
                target: None,
                burn_fuel: 0.0,
                reserve_fuel: 0.0,
                thrust: 0.0,
                max_lateral: 0.0,
                armed: true,
                arm_distance: 0.0,
                traveled: 1_000.0,
                blast_radius: blast_radius_for(1.0),
                owner: shooter,
                eject_time: 0.0,
                launch_dir: Vec2::Y,
                age: 2.0,
                life: 5.0,
                terminal_range: 0.0,
                prev_pos: at,
                owner_ship: None,
                launch_cell: IVec2::ZERO,
                // cascade-story's field, read by `obstructs_tube`. ZERO is its
                // documented "no axis recorded -- never cook off" value, and
                // this missile is not leaving its own tube anyway.
                launch_axis: IVec2::ZERO,
            },
            Transform::from_translation(at.extend(0.0)),
        ));

        app.update();
        (app, ids)
    }

    fn health(app: &App, block: Entity) -> f32 {
        app.world().get::<HullSegment>(block).unwrap().health
    }

    /// The report, twice now: missiles do not damage blocks. A Heavy Missile
    /// landing on a steel plate must destroy it.
    #[test]
    fn a_heavy_missile_destroys_the_steel_plate_it_hits() {
        let (app, ids) = fire_into(&[(IVec2::ZERO, HullMaterial::Steel)]);
        let hp = health(&app, ids[0]);
        assert!(
            hp <= 0.0,
            "a Heavy Missile detonated on a steel plate and left it at {hp} of 100"
        );
    }

    /// The area of effect: the blocks beside the one it hit go too.
    #[test]
    fn the_blast_takes_out_the_neighbours() {
        let (app, ids) = fire_into(&[
            (IVec2::ZERO, HullMaterial::Steel),
            (IVec2::new(1, 0), HullMaterial::Steel),
            (IVec2::new(0, 1), HullMaterial::Steel),
            (IVec2::new(1, 1), HullMaterial::Steel),
        ]);
        for (i, id) in ids.iter().enumerate() {
            let hp = health(&app, *id);
            assert!(hp <= 0.0, "block {i} of the 2x2 the missile landed in survived at {hp}");
        }
    }

    /// And it stops somewhere. A block well outside the blast is untouched,
    /// or every warhead is a ship-wide strike.
    #[test]
    fn a_block_outside_the_blast_is_untouched() {
        let (app, ids) = fire_into(&[
            (IVec2::ZERO, HullMaterial::Steel),
            (IVec2::new(6, 0), HullMaterial::Steel),
        ]);
        assert_eq!(health(&app, ids[1]), 100.0, "a block six cells away took damage");
    }

    /// The player's side of the same function: an enemy warhead sitting on
    /// one of the player's modules, with the player's shield up or down.
    /// Returns (app, module, ship).
    fn enemy_missile_on_player(shield_up: bool) -> (App, Entity, Entity) {
        let mut app = App::new();
        app.insert_resource(crate::vfx::effect_textures::EffectTextures::empty());
        app.init_resource::<crate::spatial::CreatureGrid>();
        app.add_message::<crate::events::AiShipDamaged>();
        app.add_message::<crate::events::ShipDamaged>();
        app.add_message::<crate::events::HullBreached>();
        app.add_message::<crate::events::RoomDepressurized>();
        app.init_resource::<crate::building::rooms::RoomMap>();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, check_missile_hits);

        let mut shield = shield_down();
        if shield_up {
            shield.current = 100.0;
            shield.max = 100.0;
        }
        let ship = app.world_mut()
            .spawn((Ship, Transform::default(), GlobalTransform::default(), shield))
            .id();
        let at = crate::building::grid_to_local(IVec2::ZERO);
        let module = app.world_mut().spawn((
            Module {
                module_type: ModuleType::SmallReactor,
                health: 100.0,
                max_health: 100.0,
                power_consumption: 0.0,
                power_generation: 0.0,
                is_active: true,
                grid_position: IVec2::ZERO,
                size: IVec2::ONE,
                rotation: Rotation::North,
            },
            Transform::from_translation(at.extend(0.0)),
            GlobalTransform::from_translation(at.extend(0.0)),
            ChildOf(ship),
        )).id();
        app.world_mut().entity_mut(ship).insert(ShipGrid { cells: HashMap::from([(IVec2::ZERO, module)]) });

        // Launched by an enemy ship, so the player's hull is foreign to it.
        let enemy = app.world_mut().spawn_empty().id();
        let tube = app.world_mut().spawn(ChildOf(enemy)).id();
        app.world_mut().spawn((
            MissileProjectile {
                damage: 75.0,
                armed: true,
                owner: tube,
                prev_pos: at + Vec2::new(0.0, 40.0),
                owner_ship: Some(enemy),
                ..default()
            },
            Transform::from_translation(at.extend(0.0)),
        ));

        app.update();
        (app, module, ship)
    }

    fn ship_damage_written(app: &App) -> usize {
        app.world().resource::<bevy::ecs::message::Messages<crate::events::ShipDamaged>>().len()
    }

    /// A raised shield stops an enemy missile, exactly as it stops an enemy
    /// round. It used to be ignored: the warhead flew through the bubble.
    #[test]
    fn the_players_shield_stops_an_enemy_missile() {
        let (app, module, ship) = enemy_missile_on_player(true);
        assert_eq!(ship_damage_written(&app), 0, "the hull was hit through a live shield");
        assert_eq!(app.world().get::<Module>(module).unwrap().health, 100.0);
        assert!(app.world().get::<ShipShield>(ship).unwrap().current < 100.0, "the shield took nothing");
    }

    /// Shield down: ONE path applies the damage. The module was being hit
    /// directly here and then again by process_ship_damage off the event.
    #[test]
    fn an_enemy_missile_is_charged_once() {
        let (app, module, _) = enemy_missile_on_player(false);
        assert_eq!(ship_damage_written(&app), 1, "the impact must reach the damage pipeline");
        assert_eq!(
            app.world().get::<Module>(module).unwrap().health, 100.0,
            "the module was damaged here as well as through ShipDamaged"
        );
    }

    /// Armour still matters. The heaviest plating should survive a hit that
    /// strips steel outright — otherwise material choice means nothing
    /// against missiles.
    #[test]
    fn abyssal_plate_survives_what_steel_does_not() {
        let (app, ids) = fire_into(&[(IVec2::ZERO, HullMaterial::AbyssalAlloy)]);
        let hp = health(&app, ids[0]);
        assert!(hp > 0.0, "abyssal plate (300 HP) was destroyed outright by one missile");
        assert!(hp < 300.0, "abyssal plate took no damage at all");
    }
}

#[cfg(test)]
mod own_ship_tests {
    use super::*;

    /// A hull 5 cells long, x 5..9, y -2..2.
    fn hull() -> crate::building::ShipGrid {
        let mut cells = bevy::platform::collections::HashMap::new();
        for x in 5..10 {
            for y in -2..3 {
                cells.insert(IVec2::new(x, y), Entity::from_raw_u32(1).unwrap());
            }
        }
        crate::building::ShipGrid { cells }
    }

    #[test]
    fn a_silo_fires_out_from_the_ship_on_its_own_side() {
        assert_eq!(silo_out(Vec2::new(0.0, 300.0), Vec2::ZERO, Vec2::X), Vec2::Y);
        assert_eq!(silo_out(Vec2::new(-200.0, 0.0), Vec2::ZERO, Vec2::X), Vec2::NEG_X);
        assert_eq!(silo_out(Vec2::ZERO, Vec2::ZERO, Vec2::X), Vec2::X, "dead on the middle: the fallback");
    }

    #[test]
    fn it_never_flies_back_through_its_own_ship() {
        let grid = hull();
        // Heading straight at the hull from outside: it turns just enough to
        // pass by, and that way is clear.
        let d = clear_of_home(&grid, Vec2::ZERO, Vec2::X, 20.0).expect("it turns");
        assert!(grid.walk(d * 0.75, d * 20.0).is_empty(), "{d}");
        assert!(d.x > 0.0, "it still goes the way it was going: {d}");
        // Heading away, or with its target nearer than the ship: as it is.
        assert_eq!(clear_of_home(&grid, Vec2::ZERO, Vec2::NEG_X, 20.0), None);
        assert_eq!(clear_of_home(&grid, Vec2::ZERO, Vec2::X, 3.0), None, "its target is short of the hull");
    }
}
