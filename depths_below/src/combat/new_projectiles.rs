use bevy::prelude::*;
use crate::celestial::components::{GravityAffected, GravityForce};
use super::targeting::{TargetSelection, FireGroupState, FireGroup, lead_prediction::*};
use super::*;

// ============================================================================
// NEW PROJECTILE SYSTEM
// Every projectile is a real entity: position, velocity, gravity-affected.
// Kinetic rounds have penetration. Angled hits ricochet.
// ============================================================================

/// A projectile entity
#[derive(Component)]
pub struct Projectile {
    pub damage: f32,
    pub speed: f32,
    pub lifetime: f32,
    pub max_lifetime: f32,
    pub owner: Entity,
    pub damage_type: ProjectileDamageType,
    pub penetration: f32,     // How much armor it can go through
    pub has_penetrated: bool, // Already went through one layer
    /// Loaded round type — drives on-hit behavior (blast, EMP, burn, ...).
    /// None for AI shots and legacy paths → plain single-block damage.
    pub ammo: Option<crate::combat::ammo_types::KineticAmmoType>,
    /// A designed shell's own on-hit effect (combat::shell), worked out when
    /// it was fired. Beats `ammo`'s; `ammo` then only stands for how the
    /// round meets armour (spall, angle, what gets through plate).
    pub effect: Option<crate::combat::ammo_types::AmmoHitBehavior>,
    /// Caliber scale of the firing weapon — shrinks/grows the ammo's on-hit
    /// EFFECTS (blast radius, EMP duration, burn time). Damage numbers
    /// already scale through proj.damage; without this a gatling EMP round
    /// disabled as wide and as long as a cannon's, at 10x the fire rate.
    pub caliber: f32,
    /// Block already damaged by this round — a penetrator passing through a
    /// block is still inside its hit radius next frame; without this it
    /// would hit the same block twice instead of the one behind it.
    pub last_hit: Option<Entity>,
    /// Where this round was at the END of the previous frame.
    ///
    /// Projectiles don't travel, they teleport: `move_projectiles` adds
    /// `velocity * dt` once per frame. A railgun does 9000 u/s = 150 units
    /// per frame at 60fps, and hit detection only looked at the round's
    /// CURRENT position with a 45-unit radius — a 90-unit window on a
    /// 150-unit stride. The 60-unit blind spot in between was never tested,
    /// and blocks are 66 units apart, so fast rounds flew straight through
    /// solid ships. Worse for APFSDS (x1.5 speed → 225-unit stride) and
    /// worse again whenever the framerate dropped, i.e. during big fights.
    ///
    /// Hit tests sweep `prev_pos -> current` instead, so there is no gap
    /// left to hide in regardless of speed or frame time.
    pub prev_pos: Vec2,
    /// Ricochets so far. Bounded by MAX_BOUNCES: a round caught in a concave
    /// notch between two sloped plates would otherwise skip off the same
    /// corner every frame forever.
    pub bounces: u8,
    /// A flak round's fuse (a shell with a proximity fuse, combat::shell):
    /// it bursts beside an enemy missile instead of having to hit one.
    pub flak: Option<Flak>,
}

/// How a flak round goes off: within `fuse` of an enemy missile, it bursts,
/// and every enemy missile within `burst` takes `damage`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flak {
    pub fuse: f32,
    pub burst: f32,
    pub damage: f32,
}

impl Flak {
    /// A built gun's flak, if its shell has a fuse; `kinetic` is the round's
    /// own damage, half of which the burst carries too.
    pub fn of(gun: &crate::combat::gun_assembly::GunAssembly, kinetic: f32) -> Option<Flak> {
        let shell = &gun.parts.shell;
        shell.flak_radii(gun.stats.gauge_mm).map(|(fuse, burst)| Flak {
            fuse,
            burst,
            damage: shell.flak_damage(gun.stats.gauge_mm) + kinetic * 0.5,
        })
    }
}

/// How many times one round may skip before it's spent.
pub const MAX_BOUNCES: u8 = 2;

/// Speed cap on a deflected round.
///
/// A ricochet sheds most of its energy, and the fraction alone wasn't enough:
/// a railgun does 9000 u/s, so even at 45% it left at 4000 and was off the
/// screen before the next frame. The deflection is the feedback — you're meant
/// to SEE the round leave along a new heading — and it's worth nothing if it
/// lasts one frame. Capped to something you can follow.
pub const RICOCHET_MAX_SPEED: f32 = 1500.0;

/// Penetration a round spends on a block with no armour of its own (an
/// unplated module): even the soft insides of a ship slow a round down.
pub const PIERCE_COST_MIN: f32 = 5.0;
/// Below this a round coming out the back of a block has nothing left worth
/// carrying on with.
pub const PIERCE_MIN_DAMAGE: f32 = 5.0;
/// The share of a designed shell's blast a block at the very rim takes.
pub const HE_EDGE: f32 = 0.4;

/// Below this caliber (a 150 mm gun is 1.0) a shell just makes its hole.
const SHOCK_MIN_CALIBER: f32 = 1.0;
/// Reach of the shock, per caliber: a block and a half round a 170 mm hit, four
/// and a half round a 500 mm one.
const SHOCK_RADIUS: f32 = 88.0;
/// The share of the shell's damage the blocks at the middle of the shock take.
const SHOCK_SHARE: f32 = 0.1;

/// The shock of a heavy shell's first impact, radius and damage at the middle:
/// what makes a big gun's hit tear a piece out, not bore one line through it.
pub fn impact_shock(caliber: f32, damage: f32) -> Option<(f32, f32)> {
    (caliber >= SHOCK_MIN_CALIBER).then_some((SHOCK_RADIUS * caliber, damage * SHOCK_SHARE))
}

/// What an armour-piercing round carries into the next block, if anything.
///
/// A block soaks up to the health it has left; what the round had beyond
/// that -- the overkill -- keeps going, less what it loses in the hole, for
/// as long as it has penetration left to spend on the plates in its way. A
/// round that didn't kill the block still gets one layer further in (the
/// rule AP always had), so a light gun keeps its second block.
///
/// `penetration` is what the round has left after paying for this block.
pub fn pierce_onward(damage: f32, overkill: f32, penetration: f32, first: bool, falloff: f32) -> Option<f32> {
    let carried = if overkill > 0.0 {
        overkill
    } else if first {
        damage
    } else {
        0.0
    } * (1.0 - falloff);
    (carried >= PIERCE_MIN_DAMAGE && (first || penetration > 0.0)).then_some(carried)
}

/// Closest approach between point `p` and segment `a -> b`.
/// Returns (distance, t) where t is 0 at `a` and 1 at `b`.
///
/// t is what orders hits along the round's path: the block it crossed FIRST
/// wins, not the one whose centre happens to sit nearest the segment.
/// How far ahead of a turret gun's muzzle the firing cut-out looks for the
/// ship's other turrets: further than any turret reaches across a hull.
const TURRET_CUTOUT_REACH: f32 = 66.0 * 40.0;

pub fn segment_closest(a: Vec2, b: Vec2, p: Vec2) -> (f32, f32) {
    let ab = b - a;
    let len_sq = ab.length_squared();
    if len_sq < 1e-6 {
        return (a.distance(p), 0.0);
    }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    (a.lerp(b, t).distance(p), t)
}

/// Where a round's step `a -> b` meets a shield bubble, for the shield test.
///
/// A round that crosses the shell this step is judged at the point it
/// crossed. One already inside at the start of the step (spawned inside, or
/// let through an uncovered arc) is judged where it ends, as before. `None`
/// when the step never touches the bubble.
pub fn shield_contact(a: Vec2, b: Vec2, center: Vec2, radius: f32) -> Option<Vec2> {
    if a.distance(center) < radius {
        return (b.distance(center) < radius).then_some(b);
    }
    let d = b - a;
    let len_sq = d.length_squared();
    if len_sq < 1e-6 {
        return None;
    }
    let f = a - center;
    let half_b = f.dot(d);
    let c = f.length_squared() - radius * radius;
    let disc = half_b * half_b - len_sq * c;
    if disc < 0.0 {
        return None;
    }
    let s = (-half_b - disc.sqrt()) / len_sq;
    (0.0..=1.0).contains(&s).then(|| a + d * s)
}

/// A block set on fire by incendiary rounds — ticks damage until it burns out.
#[derive(Component)]
pub struct BlockBurning {
    pub dps: f32,
    pub remaining: f32,
    /// Owning AI ship — burn ticks report here so aggregate hull integrity
    /// (process_ai_ship_damage_system) stays in sync.
    pub ship: Entity,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProjectileDamageType {
    Kinetic,        // AP rounds — penetrates
    Explosive,      // HE rounds — area on impact
    Incendiary,     // Sets fires
    EmpRound,       // Disables modules
}

/// Missile entity — cold-gas eject, then motor burn, then guided flight.
///
/// The three phases are what separate a missile from a bullet: it leaves the
/// tube on a gas charge committed to the silo heading (`eject_time`), lights
/// the motor (`burn_fuel`), and only then starts steering (`reserve_fuel`).
/// By ignition the geometry is already off, so the seeker has to fly a curve
/// to fix it — which is the whole point.
#[derive(Component)]
pub struct MissileProjectile {
    pub damage: f32,
    pub target: Option<Entity>,
    /// SECONDS of main-motor burn remaining. Was a raw fuel quantity drained
    /// at a thrust-dependent rate, which made burn duration an accident of
    /// two unrelated numbers; as a time it is the thing you actually tune.
    pub burn_fuel: f32,
    /// RADIANS of steering authority remaining. Each frame's turn is
    /// subtracted, so a missile that fights a hard crossing shot runs out of
    /// corrections and coasts — the reason a Heavy can be baited into a miss.
    pub reserve_fuel: f32,
    pub thrust: f32,
    /// Maximum lateral acceleration in units/s². Divided by current speed to
    /// get a turn rate, so a missile that is going fast turns wide — the
    /// reason overshoot happens at all.
    pub max_lateral: f32,
    pub armed: bool,           // Needs to travel min distance before arming
    pub arm_distance: f32,
    pub traveled: f32,
    pub blast_radius: f32,
    pub owner: Entity,
    /// Seconds of cold-gas coast left before the motor lights. While this is
    /// positive the missile does not thrust and does not steer.
    pub eject_time: f32,
    /// The silo heading it was ejected along — held rigidly during the coast.
    pub launch_dir: Vec2,
    /// Seconds this body has been in flight. Drives the agility curve: a
    /// missile is at its most manoeuvrable just after the motor lights and
    /// turns progressively wider the longer it flies.
    pub age: f32,
    /// Seconds until self-destruct. Replaces the old trick of running
    /// `burn_fuel` negative and using it as a coast timer, which fought any
    /// attempt to give the motor a real burn duration.
    pub life: f32,
    /// Inside this range the seeker gets a harder turn limit — terminal dive.
    pub terminal_range: f32,
    /// Position last frame — swept-hit anchor, same role as `Projectile::prev_pos`.
    pub prev_pos: Vec2,
    /// The ship that launched it, when it was launched by a real tube.
    ///
    /// Unlike every other projectile in the game, a missile is NOT allowed
    /// through its parent hull — it is the deliberate exception. This is how
    /// it finds the hull to test. `None` for the plasma and EMP rounds that
    /// borrow this component, which keep the ordinary pass-through.
    pub owner_ship: Option<Entity>,
    /// Ship-local cell of the tube it left.
    ///
    /// Identified by CELL and not by entity: `update_ship_grids` writes hull
    /// after modules, so a launcher sitting on a hull tile — which is all of
    /// them — resolves to the hull segment in the grid, never to the weapon.
    /// Comparing entities meant a missile treated its own launcher's cell as
    /// solid hull and detonated the instant it spawned.
    pub launch_cell: IVec2,
    /// The tube's axis in ship-LOCAL cells (the launcher's facing offset).
    ///
    /// Kept in cells rather than reusing the world-space `launch_dir`, which
    /// is fixed at launch and goes stale the instant the ship turns -- which
    /// is precisely when a warhead is still leaving the tube.
    pub launch_axis: IVec2,
}

impl Default for MissileProjectile {
    fn default() -> Self {
        Self {
            damage: 0.0,
            target: None,
            burn_fuel: 0.0,
            reserve_fuel: 0.0,
            thrust: 0.0,
            max_lateral: 0.0,
            armed: false,
            arm_distance: 80.0,
            traveled: 0.0,
            blast_radius: 40.0,
            owner: Entity::PLACEHOLDER,
            age: 0.0,
            eject_time: 0.0,
            launch_dir: Vec2::X,
            life: 6.0,
            terminal_range: 300.0,
            prev_pos: Vec2::ZERO,
            owner_ship: None,
            launch_cell: IVec2::MAX,
            launch_axis: IVec2::ZERO,
        }
    }
}

// ============================================================================
// WEAPON FIRING SYSTEM — uses fire groups + lead prediction
// ============================================================================

/// Main weapon firing system: reads fire groups, aims with lead prediction, spawns projectiles
/// Everything the gun battery needs to answer "where is the player pointing".
///
/// Bundled because `fire_weapons_system` sits at Bevy's 16-parameter ceiling
/// and adding one more silently drops its `IntoSystem` impl -- which surfaces
/// as `no method named in_set` on the whole tuple in CombatPlugin, several
/// hundred lines away, rather than as anything about this function.
#[derive(bevy::ecs::system::SystemParam)]
pub struct AimInput<'w, 's> {
    pub windows: Query<'w, 's, &'static Window>,
    pub camera: Query<'w, 's, (&'static Camera, &'static GlobalTransform), With<crate::camera::MainCamera>>,
    pub input_state: Res<'w, crate::resources::InputState>,
    pub aim_lock: Res<'w, crate::combat::targeting::AimLock>,
}

/// A gun round in flight (tools/missile-art): a shell with a tracer, light
/// and neutral so its tint (brass, EMP blue, enemy red) colours it.
pub const ROUND_ART: &str = "sprites/effects/round.png";

/// On a round fired without the asset server to hand: `skin_rounds` gives
/// it the round art the frame after.
#[derive(Component)]
pub struct RoundSkin;

/// The player's rounds wear the round art. They were flat rectangles.
pub fn skin_rounds(mut commands: Commands, assets: Res<AssetServer>, mut new: Query<(Entity, &mut Sprite), Added<RoundSkin>>) {
    for (round, mut sprite) in new.iter_mut() {
        sprite.image = assets.load(ROUND_ART);
        commands.entity(round).try_remove::<RoundSkin>();
    }
}

/// DEPTHS_AIM_AT=<x>,<y> (ship-local units): where unattended runs aim.
pub(crate) fn aim_at_override() -> Option<Vec2> {
    static AT: std::sync::OnceLock<Option<Vec2>> = std::sync::OnceLock::new();
    *AT.get_or_init(|| {
        let v = std::env::var("DEPTHS_AIM_AT").ok()?;
        let (x, y) = v.split_once(',')?;
        Some(Vec2::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
    })
}

pub fn fire_weapons_system(
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    aiming: AimInput,
    time: Res<Time>,
    fire_state: Res<FireGroupState>,
    power_state: Res<crate::resources::PowerState>,
    selection: Res<TargetSelection>,
    ship_query: Query<(Entity, &Transform, &ShipPhysics, &Velocity), With<Ship>>,
    mut weapon_query: Query<(
        Entity, &Module, Has<CrewStation>, Option<&ModuleEfficiency>, &mut Weapon, &mut WeaponCooldown,
        &GlobalTransform, &FireGroup, &WeaponMount, &ChildOf,
        Option<&crate::building::customization::parameters::ModuleCustomization>,
        Option<&crate::building::customization::tuning::WeaponTuning>,
        Option<&crate::building::customization::tuning::SelectedAmmo>,
        Option<&ModuleTemperature>,
        // Its auto-engage point, and whether it's on point defence.
        (Option<&crate::combat::targeting::AutoAimPoint>, Has<crate::combat::targeting::fire_groups::InterceptMode>),
    ), Without<DestroyedModule>>,
    target_transform_query: Query<&Transform, Without<Ship>>,
    target_velocity_query: Query<&Velocity, Without<Ship>>,
    targeting_computer_query: Query<(&Module, &ChildOf), Without<DestroyedModule>>,
    mut fired_events: MessageWriter<crate::events::WeaponFired>,
    mut commands: Commands,
    debug_tuning: Res<crate::debug::DebugTuning>,
    // Guns built from parts: their velocity, spread, caliber, penetration and
    // muzzle come from the parts (combat::gun_assembly), not the block type.
    assemblies: Query<(&crate::combat::gun_assembly::GunAssembly, Option<&crate::combat::gun_assembly::GunMuzzle>)>,
    // Bundled: this system is at Bevy's parameter cap.
    turrets: (
        Query<(&ChildOf, &crate::combat::turret_deck::TurretBody, Option<&crate::combat::gun_house::RoofFittings>)>,
        Res<crate::combat::turret_deck::TurretShapes>,
    ),
) {
    let (turret_bodies, turret_shapes) = (&turrets.0, &turrets.1);
    let Ok((player_ship, ship_transform, ship_physics, ship_velocity)) = ship_query.single() else { return };
    let _dt = time.delta_secs();

    // Weapons need power: a grid in deficit (e.g. shield surging under fire)
    // silences the guns until the balance recovers.
    if power_state.power_balance < 0.0 {
        return;
    }

    // Cursor world position — dumb-fire fallback when no target is selected.
    let cursor_world: Option<Vec2> = aiming.windows.single().ok()
        .and_then(|w| w.cursor_position())
        .and_then(|c| {
            aiming.camera.single().ok()
                .and_then(|(cam, gt)| cam.viewport_to_world_2d(gt, c).ok())
        });
    // Controller right-stick aim beats the mouse while it owns aim (see
    // InputState.gamepad_aim): dumb-fire at a point projected out along
    // the stick direction.
    // DEPTHS_AIM_AT=<x>,<y>: a scripted run aims at this point (ship-local,
    // so it stays put relative to the ship as it moves). It comes first: the
    // real mouse may be sitting over the window and would steer the run.
    let cursor_world = aim_at_override().map(|p| ship_transform.transform_point(p.extend(0.0)).truncate())
        .or_else(|| aiming.input_state.gamepad_aim.map(|dir| ship_transform.translation.truncate() + dir * 2000.0))
        .or(cursor_world);

    // Build module position list for adjacency checks -- OUR modules only.
    // Grid cells are ship-local, so an enemy's Targeting Computer at the
    // local cell beside one of our guns used to count as adjacent to it.
    let all_modules: Vec<(IVec2, ModuleType, bool)> = targeting_computer_query.iter()
        .filter(|(_, parent)| parent.parent() == player_ship)
        .map(|(m, _)| (m.grid_position, m.module_type, m.is_active))
        .collect();

    for (entity, module, has_station, staffing, mut weapon, mut cooldown, global_transform, fire_group, mount, parent, customization, tuning, selected_ammo, temp, (auto_aim, point_defence)) in weapon_query.iter_mut() {
        // A gun with nobody on it does not fire.
        if !crate::combat::weapon_is_crewed(has_station, staffing) {
            continue;
        }
        // Player ship only: this query has no ownership filter on its own, and
        // AI ships carry the exact same Weapon/FireGroup/WeaponMount
        // components (shared spawn_module path). Unscoped, holding Space
        // also fired every AI ship's default-group weapon at the player's
        // current target/cursor — including an AI ship shooting itself when
        // the player had it targeted.
        // A gun on a turret deck is a grandchild: its turret is the parent.
        let owner = turret_bodies.get(parent.parent()).map(|(t, _, _)| t.parent()).unwrap_or(parent.parent());
        if owner != player_ship { continue; }
        // True kinetic weapons only. This loop has no type match arms below
        // for anything else — every OTHER weapon type (Laser, PlasmaCaster,
        // IonDisruptor, EMPPulse, the three missile types, MiningDrill,
        // TractorBeam) fell through the `_ =>` wildcard arms and phantom-fired
        // a generic small yellow bullet from this system IN ADDITION to
        // whatever their own dedicated firing system did (or, for
        // PlasmaCaster, didn't — it has no dedicated system at all). That
        // stray shot is what made the laser look broken: the real beam
        // can't hit ships (separate bug), but this phantom bullet could,
        // muddying what was actually happening.
        if !matches!(module.module_type,
            ModuleType::Cannon | ModuleType::Railgun | ModuleType::Coilgun | ModuleType::Gatling
            | ModuleType::FiringPiece
        ) {
            continue;
        }
        if !module.is_active { continue; }

        // Tick cooldown BEFORE the thermal gate. Gating first froze the
        // timer while hot — and generate_heat treats a running cooldown as
        // "recently fired", so a hot gun kept generating heat forever and
        // never came back (one burst → permanently stuck red).
        cooldown.timer.tick(time.delta().mul_f32(debug_tuning.fire_rate_mult));
        if !cooldown.timer.is_finished() { continue; }

        // Thermal throttle — same gate the laser uses. Overtuned guns heat
        // past this under sustained fire and stutter until they cool.
        if let Some(temp) = temp {
            if temp.current >= temp.max_temp * 0.95 { continue; }
        }

        // Check if this weapon's fire group is active
        // A gun on point defence fires by itself at the missile its turret
        // has engaged (missile_defence), and at nothing else; built guns
        // only -- an old gun on intercept mode is point_defense's.
        let pd_engaged = point_defence
            && module.module_type == ModuleType::FiringPiece
            && turret_bodies.get(parent.parent()).is_ok_and(|(_, t, _)| t.pd_engaged);
        if point_defence && module.module_type == ModuleType::FiringPiece && !pd_engaged { continue; }
        let group_firing = fire_state.firing[fire_group.group as usize % 4];
        if !group_firing && !pd_engaged { continue; }

        // Check ammo
        if !crate::combat::INFINITE_AMMO && weapon.ammo <= 0 { continue; }

        let (assembly, muzzle) = match assemblies.get(entity) {
            Ok((a, m)) => (Some(a), m.copied()),
            Err(_) => (None, None),
        };
        // A built gun fires from the end of its barrel, not from its breech.
        let weapon_pos = match muzzle {
            Some(m) => m.world,
            None => global_transform.translation().truncate(),
        };

        // Aim at the selected target if there is one; otherwise dumb-fire at
        // the cursor. Never silently skip on range — an out-of-range target
        // just means the shot is capped at max range and falls short, which
        // is visible feedback instead of a gun that refuses to fire.
        // A right-click lock names a specific BLOCK — that's the aim point,
        // capped to range like any other. Falls through to the ship-level
        // selection and then the cursor when nothing is locked.
        let (target_pos, target_vel) = if pd_engaged {
            // The turret's aim is already where the round meets the missile.
            (turret_bodies.get(parent.parent()).ok().and_then(|(_, t, _)| t.aim).unwrap_or(weapon_pos), Vec2::ZERO)
        } else if let Some(point) = aiming.aim_lock.aim_point() {
            let to_point = point - weapon_pos;
            let aim = if to_point.length() > weapon.range {
                weapon_pos + to_point.normalize_or_zero() * weapon.range
            } else {
                point
            };
            let vel = aiming.aim_lock.ship
                .and_then(|e| target_velocity_query.get(e).ok())
                .map(|v| v.0)
                .unwrap_or(Vec2::ZERO);
            (aim, vel)
        } else if let Some(auto) = auto_aim {
            // Nothing locked: this gun has been handed its own block on the
            // engaged ship (targeting::auto_engage). Every gun gets a
            // different one, which is what stops a battery from drilling a
            // single tile while the rest of the hull goes untouched.
            if !auto.in_reach { continue; }
            let to_point = auto.point - weapon_pos;
            let aim = if to_point.length() > weapon.range {
                weapon_pos + to_point.normalize_or_zero() * weapon.range
            } else {
                auto.point
            };
            let vel = target_velocity_query.get(auto.ship)
                .map(|v| v.0)
                .unwrap_or(Vec2::ZERO);
            (aim, vel)
        } else if let Some(target_entity) = selection.target {
            let Ok(target_transform) = target_transform_query.get(target_entity) else { continue };
            let mut target_pos = target_transform.translation.truncate();
            let to_target = target_pos - weapon_pos;
            if to_target.length() > weapon.range {
                target_pos = weapon_pos + to_target.normalize_or_zero() * weapon.range;
            }
            let vel = target_velocity_query.get(target_entity)
                .map(|v| v.0)
                .unwrap_or(Vec2::ZERO);
            (target_pos, vel)
        } else if let Some(cursor) = cursor_world {
            // Cap the aim point to weapon range along the cursor direction
            let to_cursor = cursor - weapon_pos;
            let aim = if to_cursor.length() > weapon.range {
                weapon_pos + to_cursor.normalize_or_zero() * weapon.range
            } else {
                cursor
            };
            (aim, Vec2::ZERO)
        } else {
            continue;
        };
        // A turret gun shoots at what its turret is turning toward -- one aim
        // for the turret and every gun on it. Each gun picking its own block
        // left the barrel pointed one way and the gun waiting on another.
        let target_pos = match turret_bodies.get(parent.parent()).ok().and_then(|(_, t, _)| t.aim) {
            Some(aim) => {
                let to = aim - weapon_pos;
                if to.length() > weapon.range { weapon_pos + to.normalize_or_zero() * weapon.range } else { aim }
            }
            None => target_pos,
        };

        // Fixed/broadside mounts can't swivel outside their arc — without this
        // check a forward-facing cannon would fire at a target behind the
        // ship, sending the shot straight through the hull it's mounted on.
        // A built gun fires where its barrel points -- which, on a turret, is
        // wherever the turret has got to. It holds fire until the turret has
        // traversed onto the target.
        let in_arc = match muzzle {
            Some(m) => m.dir.dot((target_pos - weapon_pos).normalize_or_zero())
                >= (mount.firing_arc / 2.0).to_radians().cos(),
            None => is_in_firing_arc(ship_physics.rotation, &module.rotation, mount, target_pos - weapon_pos),
        };
        if !in_arc {
            if crate::combat::gun_assembly::trace() && turret_bodies.contains(parent.parent()) {
                if let Some(m) = muzzle {
                    let off = m.dir.angle_to((target_pos - weapon_pos).normalize_or_zero()).to_degrees();
                    info!("TURRET HOLDS FIRE {:?}: barrel {:.1} deg off its aim, {:.0} away", entity, off, (target_pos - weapon_pos).length());
                }
            }
            continue;
        }
        // A built gun with its barrels shot away has no muzzle speed: it
        // would only drop dead rounds at the breech.
        if assembly.is_some_and(|a| a.stats.velocity_mps <= 0.0) {
            continue;
        }
        // A firing cut-out: a turret gun holds fire while its line of fire
        // crosses another of the ship's turrets. (Hull guns fire on the deck
        // below and keep the no-line-of-sight rule.)
        if turret_bodies.contains(parent.parent()) {
            let own = parent.parent();
            let to_aim = target_pos - weapon_pos;
            let end = weapon_pos + to_aim.normalize_or_zero() * to_aim.length().min(TURRET_CUTOUT_REACH);
            let fouled = turret_shapes.0.iter()
                .filter(|t| t.body != own && t.ship == player_ship)
                .flat_map(|t| t.at(t.angle))
                .any(|p| {
                    let world = ship_transform.transform_point(p.extend(0.0)).truncate();
                    segment_closest(weapon_pos, end, world).0 < crate::combat::turret_deck::TURRET_CLEARANCE * 0.8
                });
            if fouled {
                continue;
            }
        }

        // Determine prediction tier — Targeting Computer must be ADJACENT to this weapon
        // A periscope on a turret's roof is its guns' sight (combat::gun_house).
        let sighted = turret_bodies.get(parent.parent()).is_ok_and(|(_, _, f)| f.is_some_and(|f| f.sighted()));
        let has_adjacent_tc = sighted || crate::combat::targeting::lead_prediction::check_adjacent_targeting_computer(
            module.grid_position, &all_modules,
        );
        let tier = get_weapon_prediction_tier(module, customization, has_adjacent_tc);

        // Muzzle speed: per-type base (see tuning.rs — shared with the tuning
        // window's live readout) scaled by the velocity slider and the loaded
        // ammo's own velocity profile (APFSDS darts fly, HEAT crawls).
        let tuning_vel = tuning.map(|t| t.velocity).unwrap_or(1.0);
        // A built gun's shell is designed, not picked from the ammo list.
        let selected_ammo = selected_ammo.filter(|_| assembly.is_none());
        let ammo_vel = selected_ammo.map(|a| a.0.velocity_mult()).unwrap_or(1.0);
        let proj_speed = assembly
            .map(|a| a.stats.velocity_mps * crate::combat::gun_assembly::GAME_SPEED_PER_MPS)
            .unwrap_or_else(|| crate::building::customization::tuning::base_projectile_speed(module.module_type))
            * tuning_vel * ammo_vel;

        // Get shooter velocity for relative prediction
        let shooter_vel = ship_velocity.0;

        // Calculate lead — accounts for shooter velocity, degrades with distance
        let prediction = calculate_lead(
            weapon_pos,
            shooter_vel,
            target_pos,
            target_vel,
            Vec2::ZERO,
            proj_speed,
            tier,
            weapon.range,
        );

        // Apply accuracy spread — worse at longer range
        let aim_point = apply_accuracy_spread(
            weapon_pos,
            prediction.aim_point,
            prediction.distance_accuracy * 0.85, // Distance degrades accuracy
            // Max spread at worst accuracy: a built gun's own spread
            // (barrel length, recoil, absorbers), otherwise 10 degrees.
            assembly.map(|a| a.stats.spread_deg * 4.0).unwrap_or(10.0),
        );

        // Fire!
        let direction = (aim_point - weapon_pos).normalize_or_zero();
        cooldown.timer.reset();
        if !crate::combat::INFINITE_AMMO {
            weapon.ammo = weapon.ammo.saturating_sub(1);
        }
        fired_events.write(crate::events::WeaponFired {
            weapon_type: module.module_type,
            position: weapon_pos,
            from_player: true,
        });

        // Loaded ammo drives damage type, per-round damage, and penetration.
        // A built gun's shell already has its damage and penetration in the
        // gun's numbers; it carries its own on-hit effect, and the fixed-list
        // round nearest it stands for how it meets armour.
        let shell = assembly.map(|a| &a.parts.shell);
        let loaded = shell.map(|s| s.analogue()).or(selected_ammo.map(|a| a.0));
        let (damage_type, ammo_damage_mult, penetration) = round_profile(loaded, module.module_type);
        let ammo_damage_mult = if shell.is_some() { 1.0 } else { ammo_damage_mult };
        let penetration = assembly.map(|a| crate::combat::gun_assembly::penetration_for(&a.stats)).unwrap_or(penetration);
        let effect = assembly.map(|a| a.parts.shell.hit_behavior(a.stats.gauge_mm, penetration));

        // Spawn projectile(s)
        let burst_count = match module.module_type {
            ModuleType::Coilgun => 3,  // 3-round burst
            ModuleType::Gatling => 1,  // Continuous stream (high fire rate handles it)
            _ => 1,
        };

        for _i in 0..burst_count {
            let spread_offset = if burst_count > 1 {
                Vec2::new(
                    (rand::random::<f32>() - 0.5) * 3.0,
                    (rand::random::<f32>() - 0.5) * 3.0,
                )
            } else {
                Vec2::ZERO
            };

            // Projectiles inherit the ship's own velocity — calculate_lead's
            // aim point assumes this (it solves using target_vel - shooter_vel
            // as the relative velocity, which only converges to a correct
            // intercept if the shot's world-frame speed is proj_speed +
            // shooter_vel). Without this, aiming while moving fast dragged
            // the computed aim point back toward the ship itself, collapsing
            // the shot's direction to near-zero — kinetic rounds barely
            // moved while sustained-thrusting toward a target.
            let vel = direction * proj_speed + shooter_vel + spread_offset;

            // Visual size/color based on weapon — sized and colored to read
            // clearly at gameplay zoom instead of every shot looking like
            // the same small yellow sliver.
            // Lengths for the round art (effects/round.png): about 60% of the
            // old flat dashes, four times as long as wide.
            let (size, base_color) = match module.module_type {
                ModuleType::Railgun => (Vec2::new(54.0, 7.0), Color::srgb(0.2, 0.5, 1.0)),   // long blue streak
                ModuleType::Cannon => (Vec2::new(24.0, 6.0), Color::srgb(1.0, 0.45, 0.05)), // orange shell
                ModuleType::Coilgun => (Vec2::new(15.0, 4.0), Color::srgb(0.6, 0.8, 1.0)),
                ModuleType::Gatling => (Vec2::new(11.0, 3.0), Color::srgb(1.0, 0.85, 0.2)),
                // Brass, sized by the shell: 170 mm is cannon-sized, 500 mm huge.
                ModuleType::FiringPiece => {
                    let g = assembly.map(|a| a.stats.gauge_mm).unwrap_or(150.0);
                    (crate::combat::gun_assembly::round_size(g), Color::srgb(1.0, 0.78, 0.4))
                }
                _ => (Vec2::new(11.0, 3.0), Color::srgb(0.8, 0.8, 0.4)),
            };
            // Loaded ammo recolors the round (AP brass, EMP blue, ...) so a
            // mixed loadout reads at a glance; size stays per-weapon.
            let color = loaded.filter(|_| shell.is_none() || shell.is_some_and(|s| s.head != crate::combat::shell::ShellHead::Ap))
                .map(|a| a.color())
                .unwrap_or(base_color);

            let angle = vel.y.atan2(vel.x);

            commands.spawn((
                (Sprite {
                        color,
                        custom_size: Some(size),
                        ..default()
                    }, Transform {
                        translation: Vec3::new(weapon_pos.x, weapon_pos.y, 0.5),
                        rotation: Quat::from_rotation_z(angle),
                        ..default()
                    }, RoundSkin),
                Projectile {
                    damage: weapon.damage * ammo_damage_mult * debug_tuning.damage_mult,
                    speed: proj_speed,
                    lifetime: 4.0,
                    max_lifetime: 4.0,
                    owner: entity,
                    damage_type,
                    penetration,
                    has_penetrated: false,
                    ammo: loaded,
                    effect,
                    caliber: assembly.map(|a| crate::combat::gun_assembly::caliber_for(&a.stats))
                        .unwrap_or_else(|| caliber_scale(module.module_type)),
                    last_hit: None,
                    prev_pos: weapon_pos,
                    bounces: 0,
                    flak: assembly.and_then(|a| Flak::of(a, weapon.damage * ammo_damage_mult * debug_tuning.damage_mult)),
                },
                Velocity(vel),
                GravityAffected { mass: 0.5 }, // Projectiles affected by gravity (slightly)
                GravityForce::default(),
            )).insert_if(crate::combat::point_defense::PointDefenseRound { ship: player_ship }, || pd_engaged);
        }

        // Muzzle flash
        let flash_color = match module.module_type {
            ModuleType::Railgun => Color::srgb(0.2, 0.5, 1.0),
            ModuleType::Cannon => Color::srgb(1.0, 0.45, 0.05),
            ModuleType::Coilgun => Color::srgb(0.6, 0.8, 1.0),
            ModuleType::Gatling => Color::srgb(1.0, 0.85, 0.2),
            _ => Color::srgb(0.8, 0.8, 0.4),
        };
        spawn_muzzle_flash(&mut commands, &fx, weapon_pos + direction * 30.0, direction, 20.0, flash_color);
        if let (Some(a), true) = (assembly, crate::combat::gun_assembly::trace()) {
            info!("GUN fired {:?}: {:.0} mm at {:.0} u/s, {:.0} damage, next in {:.1} s, {} left",
                entity, a.stats.gauge_mm, proj_speed, weapon.damage, 1.0 / weapon.fire_rate.max(1e-3), weapon.ammo);
        }
    }
}

// ============================================================================
// PROJECTILE MOVEMENT — gravity-affected, lifetime-limited
// ============================================================================

/// Move projectiles, tick lifetime, despawn expired.
///
/// No gravity here: celestial::gravity::apply_gravity_to_velocity already
/// integrates GravityForce into Velocity for everything GravityAffected, so
/// the half-strength term this used to add put rounds at 1.5x the field --
/// the opposite of the "resist gravity" it was written for. That system
/// deliberately pulls every body at the same rate. And not requiring
/// GravityForce is what lets point-defence rounds, which carry none, move
/// and expire at all: they were sitting where they spawned forever.
pub fn move_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut proj_query: Query<(Entity, &mut Projectile, &mut Transform, &Velocity)>,
) {
    let dt = time.delta_secs();

    for (entity, mut proj, mut transform, velocity) in proj_query.iter_mut() {

        // Remember where the round was before this step. check_projectile_hits
        // runs immediately after this system and sweeps prev_pos -> current,
        // so nothing can slip through the gap between two frames.
        proj.prev_pos = transform.translation.truncate();

        // Move
        transform.translation.x += velocity.0.x * dt;
        transform.translation.y += velocity.0.y * dt;

        // Rotate to face movement direction
        if velocity.0.length_squared() > 1.0 {
            let angle = velocity.0.y.atan2(velocity.0.x);
            transform.rotation = Quat::from_rotation_z(angle);
        }

        // Age
        proj.lifetime -= dt;
        if proj.lifetime <= 0.0 {
            commands.entity(entity).try_despawn();
        }
    }
}

// ============================================================================
// PROJECTILE COLLISION — damage on hit, penetration, ricochet
// ============================================================================

/// Largest creature hit radius (Leviathan) — used to size the spatial grid query margin.
const MAX_CREATURE_HIT_RADIUS: f32 = 90.0;

/// Check projectile collisions with creatures and ships.
/// Uses the creature spatial grid to only distance-check creatures near each
/// projectile instead of every creature in the world.
/// Shrink factor from an explosive round's DAMAGE radius to the radius handed
/// to `spawn_explosion`.
///
/// spawn_explosion deliberately overshoots what it is given -- the fireball
/// grows to 2.2x and the shock ring to 3.4x -- because a blast that stops
/// exactly at its kill radius reads as smaller than it is. That overshoot was
/// tuned against missiles, whose blast_radius is ~30-70. Explosive shells
/// carry 75-160, so feeding them their raw radius put the shock ring out past
/// 500 units and swallowed the screen on every HE hit.
///
/// At 0.32 the fireball sits well inside the damage radius and the ring close
/// to it — tightened by eye from 0.4, which still read as too much on screen.
const IMPACT_BLAST_SCALE: f32 = 0.32;

/// What a round does, from what's loaded and the gun firing it: damage type,
/// damage multiplier, and penetration. Without a loaded round (pre-tuning
/// saves, untuned guns) it falls back to the per-weapon-type behaviour. The
/// player's guns and the AI's use the same table.
pub fn round_profile(
    ammo: Option<crate::combat::ammo_types::KineticAmmoType>,
    module_type: ModuleType,
) -> (ProjectileDamageType, f32, f32) {
    use crate::combat::ammo_types::KineticAmmoType;
    match ammo {
        Some(ammo) => (
            match ammo {
                KineticAmmoType::Incendiary
                | KineticAmmoType::PlasmaSlug
                | KineticAmmoType::NaniteCanister => ProjectileDamageType::Incendiary,
                // A neutron shell does nothing electrical, but of the four
                // damage types this is the one that means "the hull is
                // fine, what's inside it isn't".
                KineticAmmoType::EMPShell | KineticAmmoType::NeutronShell =>
                    ProjectileDamageType::EmpRound,
                KineticAmmoType::APHE | KineticAmmoType::HEFrag | KineticAmmoType::Flak
                | KineticAmmoType::Antimatter | KineticAmmoType::Singularity =>
                    ProjectileDamageType::Explosive,
                _ => ProjectileDamageType::Kinetic,
            },
            ammo.damage_mult(),
            ammo.penetration(),
        ),
        None => (
            ProjectileDamageType::Kinetic,
            1.0,
            match module_type {
                ModuleType::Railgun => 80.0,   // Goes through almost anything
                ModuleType::Cannon => 40.0,    // Decent penetration
                ModuleType::Coilgun => 25.0,   // Light penetration
                ModuleType::Gatling => 10.0,   // Barely penetrates
                _ => 20.0,
            },
        ),
    }
}

/// Scales every AI gun round's damage. 1.0 is parity: an enemy shell is
/// exactly as strong as the same gun and round in the player's hands.
///
/// Measured when the hit paths were merged (2026-10-25): the same hostile
/// capital took the starter from 313 to 253 blocks in 80 s through the old
/// player-only path, and to 109 through this one -- about 3.4x the damage,
/// because the old path ignored the ammo's damage multiplier, spall and
/// splash and soaked hull hits by plate thickness. This one number is where
/// to dial enemy fire back if parity is too much.
pub const AI_ROUND_DAMAGE_SCALE: f32 = 1.0;

/// An AI gun's round: the same `Projectile` the player's guns fire, resolved
/// by the same check_projectile_hits -- so an enemy shell meets your plating
/// the way yours meets theirs (angle, ricochet, spall, the ammo's own effect),
/// and AI-vs-AI fire finally meets armour at all.
///
/// Two things deliberately stay as AI rounds always flew: they do NOT inherit
/// the shooter's velocity, and nothing bends them (no gravity). The AI's lead
/// (ai_ship::combat) solves for a straight line at a fixed speed from the
/// muzzle; change either and every enemy shot misses by its own motion.
#[allow(clippy::too_many_arguments)]
pub fn spawn_ai_round(
    commands: &mut Commands,
    asset_server: &AssetServer,
    weapon: Entity,
    module_type: ModuleType,
    muzzle: Vec2,
    aim_point: Vec2,
    damage: f32,
    speed: f32,
    range: f32,
    ammo_type: AmmoType,
    loaded: Option<crate::combat::ammo_types::KineticAmmoType>,
    // A gun built from parts fires its own shell: `speed` is already its
    // muzzle velocity, and the shell sets the rest.
    built: Option<&crate::combat::gun_assembly::GunAssembly>,
) -> Entity {
    let direction = (aim_point - muzzle).normalize_or_zero();
    let speed = if built.is_some() { speed } else { speed * ammo_type.speed_mult() };
    let lifetime = (range / speed.max(1.0)).max(0.1);
    let loaded = built.map(|g| g.parts.shell.analogue()).or(loaded);
    let (damage_type, damage_mult, penetration) = round_profile(loaded, module_type);
    let (damage_mult, penetration, effect, caliber, size) = match built {
        Some(g) => {
            let pen = crate::combat::gun_assembly::penetration_for(&g.stats);
            let gauge = g.stats.gauge_mm;
            (
                1.0,
                pen,
                Some(g.parts.shell.hit_behavior(gauge, pen)),
                crate::combat::gun_assembly::caliber_for(&g.stats),
                crate::combat::gun_assembly::round_size(gauge),
            )
        }
        None => {
            // Shaped for the round art, about 60% of the old size.
            let l = ammo_type.projectile_size().x * 0.6;
            (damage_mult, penetration, None, caliber_scale(module_type), Vec2::new(l, l * 0.25))
        }
    };
    commands.spawn((
        Sprite {
            image: asset_server.load(ROUND_ART),
            // Enemy fire reads red whatever it's loaded with.
            color: Color::srgb(1.0, 0.2, 0.2),
            custom_size: Some(size * fx_scale()),
            ..default()
        },
        Transform {
            translation: muzzle.extend(0.5),
            rotation: Quat::from_rotation_z(direction.y.atan2(direction.x)),
            ..default()
        },
        Projectile {
            damage: damage * damage_mult * AI_ROUND_DAMAGE_SCALE,
            speed,
            lifetime,
            max_lifetime: lifetime,
            owner: weapon,
            damage_type,
            penetration,
            has_penetrated: false,
            ammo: loaded,
            effect,
            caliber,
            last_hit: None,
            prev_pos: muzzle,
            bounces: 0,
            flak: built.and_then(|g| Flak::of(g, damage * damage_mult * AI_ROUND_DAMAGE_SCALE)),
        },
        Velocity(direction * speed),
    )).id()
}

/// What a round landing on the PLAYER's ship sets off besides block damage --
/// the parts of the old separate player hit path (combat::projectiles +
/// ship::damage::process_ship_damage) that were about being the player, not
/// about how a round meets a block.
#[derive(bevy::ecs::system::SystemParam)]
pub struct PlayerHitEffects<'w, 's> {
    ship_damage: MessageWriter<'w, crate::events::ShipDamaged>,
    breaches: MessageWriter<'w, crate::events::HullBreached>,
    depressurized: MessageWriter<'w, crate::events::RoomDepressurized>,
    notifications: MessageWriter<'w, ShowNotification>,
    room_map: Res<'w, crate::building::rooms::RoomMap>,
    debug: Res<'w, crate::debug::DebugTuning>,
    gunnery: Query<'w, 's, &'static mut crate::ai_ship::components::AiGunneryLog>,
}

impl PlayerHitEffects<'_, '_> {
    /// Debug god mode: the player's ship takes nothing.
    pub(crate) fn god_mode(&self) -> bool {
        self.debug.god_mode
    }

    /// A player plate knocked under 30% opens to space -- the same rule
    /// process_ship_damage applied to the blocks its walk damaged.
    pub(crate) fn breach_if_holed(&mut self, segment: Entity, hull: &mut HullSegment) {
        let health_pct = if hull.max_health > 0.0 { hull.health / hull.max_health } else { 0.0 };
        if health_pct < 0.3 && !hull.is_depressurized {
            hull.is_depressurized = true;
            self.breaches.write(crate::events::HullBreached { segment, severity: 1.0 - health_pct });
            if let Some(&room_id) = self.room_map.tile_to_room.get(&hull.grid_position) {
                self.depressurized.write(crate::events::RoomDepressurized { room_id, severity: 1.0 - health_pct });
            }
        }
    }

    /// The hit, for the camera, audio, damage arrows and death screen.
    /// `applied`: the blocks have already taken it.
    pub(crate) fn report(&mut self, amount: f32, at: Vec2, travel: Vec2, announce: bool) {
        self.ship_damage.write(crate::events::ShipDamaged {
            source: crate::events::DamageSource::Gunfire,
            amount,
            position: Some(at),
            // Readers expect the direction TOWARD the attacker.
            direction: Some(-travel),
            applied: true,
        });
        if announce {
            self.notifications.write(ShowNotification {
                message: format!("Hull hit! -{:.0} damage", amount),
                notification_type: NotificationType::Danger,
                duration: 2.0,
            });
        }
    }
}

pub fn check_projectile_hits(
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    mut commands: Commands,
    // Transform and Sprite went MUTABLE here (a bounced round is stepped clear
    // of the plate and recoloured), which collides with the &Transform in the
    // creature and AI-ship queries below. Bevy can't infer that a projectile is
    // never a creature or a ship, so say so.
    mut proj_query: Query<
        (Entity, &mut Projectile, &mut Transform, &mut Velocity, &mut Sprite),
        (Without<Creature>, Without<Ship>, Without<crate::ai_ship::components::AiShip>),
    >,
    mut creature_query: Query<(&Transform, &mut Creature), Without<Ship>>,
    creature_grid: Res<crate::spatial::CreatureGrid>,
    // EVERY ship, the player's included. AI guns fire these rounds too now,
    // so there is one way a round meets a ship -- armour angle, ricochet,
    // spall, the ammo's own effect -- whoever fired it and whoever it hits.
    mut ai_ship_query: Query<
        (Entity, &Transform, &GlobalTransform, &Children, &mut crate::combat::shields::ShipShield, Has<Ship>),
        Or<(With<crate::ai_ship::components::AiShip>, With<Ship>)>,
    >,
    mut ai_module_query: Query<(&mut Module, &GlobalTransform), Without<DestroyedModule>>,
    mut ai_hull_query: Query<(&mut HullSegment, &GlobalTransform), Without<crate::components::HullDestroyed>>,
    // Neutron shells hurt the people, not the plate — see AmmoHitBehavior::Irradiate.
    mut crew_query: Query<&mut crate::components::CrewMember>,
    owner_parent_query: Query<&ChildOf>,
    turret_bodies: Query<(), With<crate::combat::turret_deck::TurretBody>>,
    turret_layers: Query<(&GlobalTransform, &Children), With<crate::combat::turret_deck::TurretBody>>,
    // Each ship's and turret's grid, and each turret's gun house roof over it.
    grids: (Query<&crate::building::ShipGrid>, Query<&crate::building::RoofGrid>),
    block_query: Query<&crate::building::Block>,
    mut ai_damage_events: MessageWriter<crate::events::AiShipDamaged>,
    mut player_hits: PlayerHitEffects,
) {
    'projectiles: for (proj_entity, mut proj, mut proj_transform, mut proj_vel, mut proj_sprite) in proj_query.iter_mut() {
        // Spent this frame: move_projectiles already queued its despawn. Both
        // systems sit in the same tuple and queue commands before the flush, so
        // resolving a hit on it here queues a SECOND despawn for the same
        // entity and the flush logs "Entity despawned: ... is invalid". Bevy
        // absorbs it, but it's noise that would mask a real one.
        if proj.lifetime <= 0.0 {
            continue;
        }
        let proj_pos = proj_transform.translation.truncate();
        // A weapon's own ship is never a valid target for its own shot,
        // regardless of aim — belt-and-suspenders on top of firing-arc and
        // per-ship query scoping.
        let owner_ship = crate::combat::turret_deck::ship_of(proj.owner, &owner_parent_query, &turret_bodies);

        // === AI SHIPS: shield first, then block-by-block hull damage ===
        for (ai_entity, ai_transform, ai_gt, children, mut shield, is_player) in ai_ship_query.iter_mut() {
            if Some(ai_entity) == owner_ship { continue; }
            // Bubble is centered on the blocks' centroid, not the root
            let center = shield.world_center(ai_transform);
            // Distance from the whole STEP, not its end point. A railgun
            // covers ~150-225 units a frame, so its end point can already be
            // past the ship's centre (or clear of a thin arm) on the frame it
            // went through -- and the end-point test missed it.
            let (dist_to_ship, _) = segment_closest(proj.prev_pos, proj_pos, center);

            // Directional shield: only the facing arc intercepts; flank/rear
            // slips past — and so does a phase slug, which is out of phase
            // with the matter the bubble is built to stop. Judged where the
            // round crossed the bubble this step: judged at the end point, a
            // fast round that finished behind the centre read as a hit from
            // the far side, the arc test failed, and it went through.
            let phased = proj.ammo.is_some_and(|a| a.ignores_shields());
            let contact = shield_contact(proj.prev_pos, proj_pos, center, shield.radius);
            if let Some(at) = contact.filter(|at| !phased && shield.is_up() && shield.covers_arc(*at - center)) {
                shield.absorb(proj.damage);
                spawn_hit_effect(&mut commands, at, Color::srgb(0.5, 0.8, 1.0), 14.0);
                commands.entity(proj_entity).try_despawn();
                continue 'projectiles;
            }

            // Shield down: hit the nearest block within impact range — hull
            // segment or module, whichever is actually closer to the impact.
            // This is the single authoritative hit resolution for AI ships;
            // process_ai_ship_damage_system used to redo its own separate
            // "nearest hull segment on the whole ship, no radius limit"
            // search off the same event, which routinely landed on some
            // unrelated block far from the actual shot — the block you aimed
            // at wasn't the one that took damage. It now only recalculates
            // aggregate integrity from whatever this system already did.
            // Scan bound follows the ship's real extent (shield radius is
            // computed from it) — a fixed bound left long hulls unhittable.
            if dist_to_ship < shield.radius + 60.0 {
                // NARROW PHASE: the grid IS the hitbox. Sweep the segment
                // from where the round WAS last frame to where it is now
                // through this ship's cell space (building::ShipGrid::walk);
                // the blocks it passed through come back in order, nearest
                // first. Two bugs die here: fast rounds no longer tunnel
                // between samples (a railgun/APFSDS step is 2-3 cells), and
                // occupancy is cell membership — no 45-unit circles over-
                // covering flat faces and leaking through corner seams.
                //
                // TOP WINS: a turret is a deck of its own standing over the
                // hull, with its own grid in its own (turning) frame. The
                // round is walked through the hull and through every turret,
                // and whichever it meets first takes the hit -- at a spot a
                // turret covers, that's the turret (ties go up). One layer
                // per spot: a round never hits the hull under a turret
                // without going through the turret, and what follows the hit
                // (spall, splash, a ricochet off the plate) stays on the layer
                // that was hit. Barrels don't count as cover: a barrel
                // reaching out past its deck is walked through, not hit.
                // A turret's gun house roof stands over its parts, so it is
                // walked after the turret and wins the tie: a round meets the
                // roof first (combat::gun_house).
                struct LayerHit {
                    layer: Entity,
                    is_turret: bool,
                    roof: bool,
                    gt: GlobalTransform,
                    cell_from: Vec2,
                    dir_local: Vec2,
                    step: crate::building::GridStep,
                    block: crate::building::Block,
                    surface: crate::combat::impact::SurfaceHit,
                }
                let mut best: Option<LayerHit> = None;
                let layers = std::iter::once((ai_entity, false, false, *ai_gt)).chain(
                    children
                        .iter()
                        .filter_map(|c| turret_layers.get(c).ok().map(|(gt, _)| (c, *gt)))
                        .flat_map(|(c, gt)| [(c, true, false, gt), (c, true, true, gt)]),
                );
                for (layer, is_turret, roof, layer_gt) in layers {
                    let layer_grid = if roof { grids.1.get(layer).map(|r| &r.0) } else { grids.0.get(layer) };
                    let Ok(layer_grid) = layer_grid else { continue };
                    let inv = layer_gt.affine().inverse();
                    // Hull cell (x, y) stands at (66x, 66y - 33); a turret's
                    // has no half-cell offset.
                    let half = if is_turret { 0.0 } else { 33.0 };
                    let to_cell = |world: Vec2| {
                        let p = inv.transform_point3(world.extend(0.0)).truncate();
                        Vec2::new(p.x / 66.0, (p.y + half) / 66.0)
                    };
                    let (cell_from, cell_to) = (to_cell(proj.prev_pos), to_cell(proj_pos));
                    // Direction in this layer's own cell space -- the hull's
                    // heading (or the turret's) is baked into `inv`, so
                    // angling it angles every plate on that side without any
                    // new per-block data.
                    let dir_local = (cell_to - cell_from).normalize_or_zero();
                    // Walk the cells, then ask each block's SHAPE what the
                    // round actually met inside it. A wedge fills half its
                    // cell, so a round crossing the hollow corner passes
                    // through and the walk carries on to whatever is behind.
                    for step in layer_grid.walk(cell_from, cell_to) {
                        // A penetrator is still inside the block it just went
                        // through; keep walking to the one behind it.
                        if Some(step.entity) == proj.last_hit { continue; }
                        if is_turret && ai_module_query.get(step.entity).is_ok_and(|(m, _)| {
                            matches!(m.module_type, ModuleType::GunBarrel | ModuleType::MuzzleBrake)
                        }) {
                            continue;
                        }
                        let block = block_query
                            .get(step.entity)
                            .copied()
                            .unwrap_or(crate::building::Block::module(step.cell));
                        let entry = cell_from + dir_local * step.t_enter;
                        let exit = cell_from + dir_local * (step.t_enter + step.span);
                        if let Some(surface) = crate::combat::impact::clip_to_shape(
                            &block, step.entry_face, step.cell, entry, exit,
                        ) {
                            let nearer = best.as_ref().is_none_or(|b| {
                                if is_turret { step.t_enter <= b.step.t_enter + 1e-3 } else { step.t_enter < b.step.t_enter }
                            });
                            if nearer {
                                best = Some(LayerHit { layer, is_turret, roof, gt: layer_gt, cell_from, dir_local, step, block, surface });
                            }
                            break;
                        }
                    }
                }
                let Some(LayerHit { layer, is_turret: on_turret, roof: on_roof, gt: layer_gt, cell_from: _, dir_local, step, block, surface }) = best else { continue };
                if crate::combat::gun_assembly::trace() {
                    if on_turret {
                        let kind = ai_module_query.get(step.entity).map(|(m, _)| format!("{:?}", m.module_type)).unwrap_or_else(|_| "deck plate".into());
                        info!("TURRET HIT on ship {:?}, turret {:?}{}: {} {:?} at turret cell {:?}, {:.0} damage", ai_entity, layer, if on_roof { " roof" } else { "" }, kind, step.entity, step.cell, proj.damage);
                    } else if proj.effect.is_some() {
                        info!("BUILT ROUND HIT ship {:?}: block {:?} at hull cell {:?}, {:.0} damage", ai_entity, step.entity, step.cell, proj.damage);
                    }
                }
                // From here on the hit is resolved on the layer it landed on.
                let grid = if on_roof { grids.1.get(layer).map(|r| &r.0) } else { grids.0.get(layer) };
                let Ok(grid) = grid else { continue };
                let ai_gt = &layer_gt;
                let children = if on_turret {
                    match turret_layers.get(layer) { Ok((_, c)) => c, Err(_) => continue }
                } else {
                    children
                };
                // World-space heading, for effects: dir_local is in the
                // target's cell space and would spray along the wrong axis on
                // any ship that isn't sitting at zero rotation.
                let dir_local_world = proj_vel.0.normalize_or_zero();
                // Debug god mode: the player's ship takes nothing.
                if is_player && player_hits.debug.god_mode {
                    commands.entity(proj_entity).try_despawn();
                    continue 'projectiles;
                }
                let obl = crate::combat::impact::obliquity(
                    surface.normal, dir_local, &block, proj.ammo, proj.caliber,
                );
                // Armour too thick for the round: what bites is less, and it
                // stops here. So does anything that hits ablative.
                let bite = crate::combat::impact::bite(proj.penetration, block.thickness, obl.cos_impact);
                let stopped = bite < 1.0 || block.stops_round;
                let incoming = proj.damage * bite;

                // Primary hit: damage the struck block, remember where.
                let mut penetrated = false;
                // What the round had beyond the block's health, and the
                // armour it went through to get there: an AP round carries
                // the first on, paying the second (see pierce_onward).
                let mut overkill = 0.0;
                let mut armour_met = 0.0;
                let primary: Option<(Entity, Vec2)> = if let Ok((_, gt)) = ai_module_query.get(step.entity) {
                    // A cell holding armour resolves to the plate (hull wins
                    // the cell in update_ship_grids), so reaching a module
                    // means nothing covers it: fully exposed, takes everything.
                    let hit_pos = gt.translation().truncate();
                    let impact = crate::combat::impact::resolve_impact(incoming, &block, surface.span, obl, Some(0.0));
                    penetrated = impact.outcome == crate::combat::impact::ImpactOutcome::Penetrated;
                    armour_met = impact.effective_thickness;
                    ai_module_query.get_mut(step.entity).ok().map(|(mut module, _)| {
                        overkill = (impact.to_block - module.health).max(0.0);
                        module.health = (module.health - impact.to_block).max(0.0);
                        spawn_hit_effect(&mut commands, hit_pos, Color::srgb(1.0, 0.6, 0.2), 12.0);
                        // Biting hits spray back along the round's own path —
                        // debris coming out of the hole, not off it.
                        spawn_impact_sparks(&mut commands, &fx, hit_pos, -dir_local_world, 0.2, 5);
                        spawn_floating_damage(&mut commands, hit_pos, impact.to_block, Color::srgb(1.0, 0.8, 0.3));
                        (step.entity, hit_pos)
                    })
                } else {
                    // ARMOUR EXPOSURE — a module still under live plating only
                    // takes the share of the round its ammo can drive through;
                    // the plate spends the rest. The walk lands on the plate
                    // (it owns the cell), so the module beneath is found by
                    // footprint, not by which sprite centre happened to be a
                    // hair nearer the round.
                    let covered = children.iter().find(|child| {
                        ai_module_query.get(*child).ok().is_some_and(|(module, _)| {
                            let footprint = crate::building::footprints::footprint_override(module.module_type);
                            crate::building::ShipGrid::cells_for(module.grid_position, module.size, module.rotation, footprint)
                                .contains(&step.cell)
                        })
                    });
                    let pass = match covered {
                        Some(_) if !stopped => crate::combat::ammo_types::armor_pass_through(proj.ammo),
                        _ => 0.0,
                    };
                    let impact = crate::combat::impact::resolve_impact(incoming, &block, surface.span, obl, Some(pass));
                    penetrated = impact.outcome == crate::combat::impact::ImpactOutcome::Penetrated;
                    armour_met = impact.effective_thickness;
                    let hull_hit = ai_hull_query.get_mut(step.entity).ok().map(|(mut hull, gt)| {
                        overkill = (impact.to_block - hull.health).max(0.0);
                        hull.health = (hull.health - impact.to_block).max(0.0);
                        let hit_pos = gt.translation().truncate();
                        spawn_hit_effect(&mut commands, hit_pos, Color::srgb(1.0, 0.5, 0.2), 16.0);
                        spawn_impact_sparks(&mut commands, &fx, hit_pos, -dir_local_world, 0.2, 6);
                        spawn_floating_damage(&mut commands, hit_pos, impact.to_block, Color::srgb(1.0, 0.3, 0.3));
                        (step.entity, hit_pos)
                    });
                    // A turret's plating keeps no air in: holing it breaches nothing.
                    if is_player && !on_turret {
                        if let Ok((mut hull, _)) = ai_hull_query.get_mut(step.entity) {
                            player_hits.breach_if_holed(step.entity, &mut hull);
                        }
                    }
                    if let (Some(module_entity), true) = (covered, impact.through > 0.0) {
                        if let Ok((mut module, gt)) = ai_module_query.get_mut(module_entity) {
                            module.health = (module.health - impact.through).max(0.0);
                            spawn_floating_damage(&mut commands, gt.translation().truncate(), impact.through, Color::srgb(1.0, 0.8, 0.3));
                        }
                    }
                    hull_hit
                };

                let Some((hit_entity, hit_pos)) = primary else { continue };

                // SPALL — the plate's inner face letting go. Not the round's
                // own effect (that's AmmoHitBehavior below); this is the
                // ARMOUR failing, which is what makes a breach categorically
                // worse than a dent.
                //
                // HESH is the exception that proves the rule: through_solid
                // means it spalls WITHOUT getting through, which is its whole
                // identity and the reason its penetration is 0 by design. It's
                // the answer to a sloped hull you can't punch.
                let spall = crate::combat::ammo_types::spall(proj.ammo);
                if (penetrated && !stopped) || spall.through_solid {
                    spall_blocks(
                        &mut commands,
                        &fx, grid, &mut ai_module_query, &mut ai_hull_query,
                        spall, step.cell, dir_local, proj.damage, hit_entity,
                        hit_pos, dir_local_world,
                    );
                }

                // RICOCHET — the round skipped instead of biting. It's still
                // flying, so deflect it rather than despawning: a bounce can
                // find another block on this same ship, which is what makes a
                // concave hull a shot trap and a convex one shed fire.
                if obl.ricochet {
                    proj.bounces += 1;
                    proj.last_hit = Some(hit_entity);
                    // No floating word here. The sparks and the round's own
                    // new heading carry it, and a label per bounce buried the
                    // screen in text during a real fight. The angle still
                    // lives in the aim-lock readout, where it's one line on
                    // the block you're actually working on.
                    // TODO(audio): wants a hard metallic skip. No suitable
                    // asset — assets/audio/impacts has explosions only — and
                    // picking one is a taste call, so it's left unwired rather
                    // than filled with something wrong.
                    spawn_hit_effect(&mut commands, hit_pos, Color::srgb(0.95, 0.95, 0.85), 10.0);
                    if proj.bounces > MAX_BOUNCES {
                        commands.entity(proj_entity).try_despawn();
                        continue 'projectiles;
                    }
                    // Plate normal back out into world space — the ship's
                    // heading is in ai_gt, so this follows the hull as it turns.
                    let n_local = surface.normal.normalize_or_zero();
                    let n = ai_gt.affine()
                        .transform_vector3(n_local.extend(0.0))
                        .truncate()
                        .normalize_or_zero();
                    let v = proj_vel.0.normalize_or_zero();
                    if n != Vec2::ZERO && v != Vec2::ZERO {
                        // A mirror bounce looks wrong: real ricochets skid
                        // ALONG the plate and shed energy. Blend mirror toward
                        // the tangent by how grazing the hit was.
                        let mirror = v - 2.0 * v.dot(n) * n;
                        let tangent = (v - v.dot(n) * n).normalize_or_zero();
                        let skid = 0.35 + 0.45 * (1.0 - obl.cos_impact);
                        let scatter = (rand::random::<f32>() - 0.5) * 0.17; // ±~5°
                        let out = Vec2::from_angle(scatter)
                            .rotate(mirror.lerp(tangent, skid).normalize_or_zero());
                        let kept = proj_vel.0.length() * (0.45 + 0.40 * (1.0 - obl.cos_impact));
                        proj_vel.0 = out * kept.min(RICOCHET_MAX_SPEED);
                        proj.damage *= 0.10 + 0.30 * (1.0 - obl.cos_impact);

                        // Sparks along the NEW heading. This is the whole
                        // point: the round already changed direction, but a
                        // small pale flash gave no way to see that it had, so
                        // a deflection looked like a shot that simply failed.
                        let graze = 1.0 - obl.cos_impact;
                        spawn_impact_sparks(&mut commands, &fx, hit_pos, out, graze, 9 + (graze * 7.0) as usize);
                        // ...and the round itself goes hot, so it can be
                        // followed off the plate instead of vanishing into the
                        // background as a dim shape travelling somewhere new.
                        proj_sprite.color = Color::srgb(1.0, 0.85, 0.55);

                        // Step the round clear of the plate it just skipped
                        // off. Without this it restarts next frame still
                        // inside the ship, finds another block immediately,
                        // and burns its two bounces within a few frames — so
                        // it never visibly goes anywhere. `last_hit` only
                        // protects it from re-hitting the SAME block.
                        let clear = out * 70.0;
                        proj_transform.translation.x += clear.x;
                        proj_transform.translation.y += clear.y;
                        proj.prev_pos = proj_transform.translation.truncate();
                    }
                    // Tell the shooter. A crew that can't see its own rounds
                    // skipping can't learn to load something else.
                    if let Some(shooter) = owner_ship {
                        if let Ok(mut log) = player_hits.gunnery.get_mut(shooter) {
                            log.ricochets += 1;
                        }
                    }
                    if is_player {
                        player_hits.report(incoming, hit_pos, dir_local_world, false);
                    } else {
                        ai_damage_events.write(crate::events::AiShipDamaged {
                            target: ai_entity,
                            source: crate::events::DamageSource::Explosion,
                            amount: 0.0,
                            position: Some(hit_pos),
                            direction: None,
                            attacker: owner_ship,
                        });
                    }
                    continue 'projectiles;
                }

                // === AMMO ON-HIT BEHAVIOR — finally consumes the
                // AmmoHitBehavior table that ammo_types.rs has defined all
                // along. `penetrates` decides whether the round survives
                // this hit and flies on into the block behind.
                let mut penetrates = false;
                let behavior = proj.effect.or_else(|| proj.ammo.map(|a| a.hit_behavior(proj.damage)));
                // A designed shell's blast weakens towards its rim; the old
                // fixed-list rounds keep the flat blast they were tuned with.
                let blast_edge = if proj.effect.is_some() { HE_EDGE } else { 1.0 };
                // A heavy shell's first impact shocks the blocks around it.
                if !proj.has_penetrated && proj.effect.is_some() && !block.stops_round {
                    if let Some((radius, shock)) = impact_shock(proj.caliber, incoming) {
                        splash_blocks(
                            &mut commands, children, &mut ai_module_query, &mut ai_hull_query,
                            hit_entity, hit_pos, radius, shock, HE_EDGE,
                        );
                        spawn_explosion(&mut commands, &fx, hit_pos, radius * IMPACT_BLAST_SCALE, Color::srgb(1.0, 0.55, 0.15));
                    }
                }
                if let Some(behavior) = behavior {
                    use crate::combat::ammo_types::AmmoHitBehavior::*;
                    match behavior {
                        Penetrate { damage_falloff, .. } => {
                            // AP/APFSDS: whatever the block couldn't soak
                            // goes on into the one behind it, block after
                            // block, until the round runs out of damage or
                            // of penetration. A heavy round bores a tunnel;
                            // a light one stops a block or two in.
                            let first = !proj.has_penetrated;
                            proj.penetration -= armour_met.max(PIERCE_COST_MIN);
                            if let Some(carried) = pierce_onward(proj.damage, overkill, proj.penetration, first, damage_falloff).filter(|_| !stopped) {
                                penetrates = true;
                                proj.has_penetrated = true;
                                proj.last_hit = Some(hit_entity);
                                proj.damage = carried;
                                // Go on from the block it just went through.
                                // The next frame sweeps from where the round
                                // is: left at the end of this frame's step, it
                                // would skip whatever lay between here and
                                // there.
                                proj_transform.translation.x = hit_pos.x;
                                proj_transform.translation.y = hit_pos.y;
                            }
                        }
                        PenetrateExplode { blast_damage, blast_radius, .. }
                        | SurfaceExplode { blast_damage, blast_radius, .. } => {
                            let radius = blast_radius * proj.caliber;
                            splash_blocks(
                                &mut commands, children, &mut ai_module_query, &mut ai_hull_query,
                                hit_entity, hit_pos, radius, blast_damage, blast_edge,
                            );
                            spawn_explosion(&mut commands, &fx, hit_pos, radius * IMPACT_BLAST_SCALE, Color::srgb(1.0, 0.5, 0.1));
                        }
                        ProximityBurst { fragment_damage, fragment_radius, .. } => {
                            let radius = fragment_radius * proj.caliber;
                            splash_blocks(
                                &mut commands, children, &mut ai_module_query, &mut ai_hull_query,
                                hit_entity, hit_pos, radius, fragment_damage, blast_edge,
                            );
                            spawn_explosion(&mut commands, &fx, hit_pos, radius * IMPACT_BLAST_SCALE, Color::srgb(1.0, 0.9, 0.4));
                        }
                        EMPDisable { disable_radius, disable_duration } => {
                            let radius = disable_radius * proj.caliber;
                            let duration = disable_duration * proj.caliber;
                            for child in children.iter() {
                                if let Ok((module, gt)) = ai_module_query.get(child) {
                                    if !module.is_active { continue; }
                                    if hit_pos.distance(gt.translation().truncate()) < radius {
                                        commands.entity(child).try_insert(
                                            crate::combat::energy_weapons::IonDisabled { timer: duration }
                                        );
                                    }
                                }
                            }
                            spawn_explosion(&mut commands, &fx, hit_pos, radius * IMPACT_BLAST_SCALE, Color::srgb(0.4, 0.5, 0.95));
                        }
                        Ignite { fire_duration, fire_intensity } => {
                            commands.entity(hit_entity).try_insert(BlockBurning {
                                // proj.damage already carries the incendiary's
                                // low direct-damage multiplier; the burn is
                                // where the real damage lives.
                                dps: proj.damage * fire_intensity,
                                remaining: fire_duration * proj.caliber,
                                ship: ai_entity,
                            });
                        }
                        Shockwave { shockwave_damage, shockwave_radius, .. } => {
                            // HESH: the block BEHIND the armor takes the spall,
                            // straight along the round's flight direction.
                            let dir = proj_vel.0.normalize_or_zero();
                            let behind = hit_pos + dir * 66.0 * shockwave_radius * 0.75;
                            let mut best: Option<(Entity, f32)> = None;
                            for child in children.iter() {
                                if child == hit_entity { continue; }
                                let block_pos = ai_module_query.get(child).map(|(_, gt)| gt.translation().truncate())
                                    .or_else(|_| ai_hull_query.get(child).map(|(_, gt)| gt.translation().truncate()));
                                if let Ok(block_pos) = block_pos {
                                    let d = behind.distance(block_pos);
                                    if d < 50.0 && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                                        best = Some((child, d));
                                    }
                                }
                            }
                            if let Some((spall_entity, _)) = best {
                                if let Ok((mut module, gt)) = ai_module_query.get_mut(spall_entity) {
                                    module.health = (module.health - shockwave_damage).max(0.0);
                                    spawn_floating_damage(&mut commands, gt.translation().truncate(), shockwave_damage, Color::srgb(0.9, 0.9, 0.5));
                                } else if let Ok((mut hull, gt)) = ai_hull_query.get_mut(spall_entity) {
                                    hull.health = (hull.health - shockwave_damage).max(0.0);
                                    spawn_floating_damage(&mut commands, gt.translation().truncate(), shockwave_damage, Color::srgb(0.9, 0.9, 0.5));
                                }
                            }
                        }
                        Implode { crush_damage, crush_radius } => {
                            // Same reach as a blast, but every block in it
                            // takes the FULL crush — an implosion has no
                            // falloff, it just closes. Splash already applies
                            // one flat figure, which is exactly right here.
                            let radius = crush_radius * proj.caliber;
                            splash_blocks(
                                &mut commands, children, &mut ai_module_query, &mut ai_hull_query,
                                hit_entity, hit_pos, radius, crush_damage, 1.0,
                            );
                            spawn_explosion(&mut commands, &fx, hit_pos, radius * IMPACT_BLAST_SCALE, Color::srgb(0.4, 0.2, 0.6));
                        }
                        Irradiate { dose, crew_affected } => {
                            // The hull is left alone on purpose. AI crew carry
                            // no Transform (see ai_ship::crew), so there is no
                            // position to measure a radius against — the cone
                            // is expressed as a headcount instead, taken off
                            // the ship's living crew.
                            let mut left = crew_affected;
                            for child in children.iter() {
                                if left == 0 { break; }
                                if let Ok(mut crew) = crew_query.get_mut(child) {
                                    if crew.health <= 0.0 { continue; }
                                    crew.health = (crew.health - dose).max(0.0);
                                    left -= 1;
                                }
                            }
                            // Dosed crew stop staffing their station, which is
                            // where the damage actually lands: an unmanned
                            // gun runs at zero efficiency (crew::compute_module_efficiency).
                            spawn_hit_effect(&mut commands, hit_pos, Color::srgb(0.8, 1.0, 0.6), 90.0);
                        }
                        // HEAT: its 1.8× damage + 70 pen already rode in on
                        // proj.damage at spawn; the angle-sensitivity part of
                        // the shaped-charge fantasy needs hit normals the
                        // grid doesn't give us yet.
                        ShapedCharge { .. } => {}
                    }
                }

                if is_player {
                    player_hits.report(incoming, hit_pos, dir_local_world, true);
                } else {
                    ai_damage_events.write(crate::events::AiShipDamaged {
                        target: ai_entity,
                        source: crate::events::DamageSource::Explosion,
                        amount: 0.0, // damage already applied directly above — this is bookkeeping only
                        position: Some(hit_pos),
                        direction: None,
                        attacker: owner_ship,
                    });
                }
                if !penetrates {
                    commands.entity(proj_entity).try_despawn();
                }
                continue 'projectiles;
            }
        }

        // Fragmenting rounds splash other creatures around the impact —
        // HE-Frag/Flak's whole identity ("great vs swarms") vs single-target AP.
        let creature_splash: Option<(f32, f32)> = proj.effect.or_else(|| proj.ammo.map(|a| a.hit_behavior(proj.damage))).and_then(|behavior| {
            use crate::combat::ammo_types::AmmoHitBehavior::*;
            match behavior {
                SurfaceExplode { blast_radius, fragment_damage, .. } => Some((blast_radius * proj.caliber, fragment_damage)),
                ProximityBurst { fragment_radius, fragment_damage, .. } => Some((fragment_radius * proj.caliber, fragment_damage)),
                // A well pulls in whatever is swimming past it, same as frag
                // catches a swarm — the reason it's worth firing at a shoal.
                Implode { crush_radius, crush_damage } => Some((crush_radius * proj.caliber, crush_damage * 0.5)),
                _ => None,
            }
        });

// Broad phase has to cover the whole step, not the end of it: the
        // grid query is centred on the segment's midpoint and widened by half
        // its length. Small creatures are where the old point test hurt most
        // — a VoidDrifter's 12-unit radius is a 24-unit window against a
        // railgun's 150-unit stride, so roughly five shots in six missed a
        // target they were aimed straight at.
        let sweep_mid = (proj.prev_pos + proj_pos) * 0.5;
        let sweep_half = proj.prev_pos.distance(proj_pos) * 0.5;
        for (creature_entity, _) in creature_grid.0.nearby(sweep_mid, sweep_half + MAX_CREATURE_HIT_RADIUS) {
            let Ok((creature_transform, mut creature)) = creature_query.get_mut(creature_entity) else { continue };
            if creature.health <= 0.0 { continue; }

            let creature_pos = creature_transform.translation.truncate();
            let hit_radius = match creature.creature_type {
                CreatureType::Leviathan => 90.0,
                CreatureType::Stalker => 30.0,
                CreatureType::ParasiteSwarm => 15.0,
                CreatureType::VoidDrifter => 12.0,
            };

            let (dist, _) = segment_closest(proj.prev_pos, proj_pos, creature_pos);
            if dist > hit_radius { continue; }

            // HIT!
            creature.health -= proj.damage;
            drop(creature);

            // Impact spark
            spawn_hit_effect(&mut commands, proj_pos, Color::srgb(1.0, 0.8, 0.3), 8.0);
            spawn_floating_damage(&mut commands, proj_pos, proj.damage, Color::srgb(1.0, 0.4, 0.2));

            // Fragment splash to everything else in the burst radius
            if let Some((radius, frag_damage)) = creature_splash {
                for (other_entity, _) in creature_grid.0.nearby(proj_pos, radius) {
                    if other_entity == creature_entity { continue; }
                    let Ok((other_transform, mut other)) = creature_query.get_mut(other_entity) else { continue };
                    if other.health <= 0.0 { continue; }
                    let other_pos = other_transform.translation.truncate();
                    if proj_pos.distance(other_pos) < radius {
                        other.health -= frag_damage;
                        spawn_floating_damage(&mut commands, other_pos, frag_damage, Color::srgb(1.0, 0.7, 0.3));
                    }
                }
                spawn_explosion(&mut commands, &fx, proj_pos, radius * IMPACT_BLAST_SCALE, Color::srgb(1.0, 0.6, 0.15));
            }

            // Despawn projectile (unless it penetrates)
            if proj.penetration < 30.0 || proj.has_penetrated {
                commands.entity(proj_entity).try_despawn();
            }
            // High penetration projectiles continue through

            break; // One hit per frame per projectile
        }
    }
}

/// How big a round each kinetic weapon actually throws — scales the ammo's
/// on-hit effects. A gatling firing APHE is a hail of small grenades; a
/// cannon firing it is a shell.
pub fn caliber_scale(module_type: ModuleType) -> f32 {
    match module_type {
        ModuleType::Gatling => 0.45,
        ModuleType::Coilgun => 0.75,
        ModuleType::Cannon => 1.0,
        ModuleType::Railgun => 1.25,
        _ => 1.0,
    }
}

/// Blast damage to every block within `radius` of the impact, except the
/// primary block (it already took the direct hit).
/// Fragments off the back of a breached plate, driven INWARD along the round's
/// path in a cone.
///
/// Deliberately not `splash_blocks`. That's a radius — it treats a bulkhead as
/// empty air and sprays the same in every direction. Spall is directional and
/// it STOPS at the first thing it meets, so what's behind the plate matters:
/// a reactor sitting right behind thin armour is in real danger, and a layer
/// of junk in front of it genuinely shields it.
///
/// Each fragment is a short walk on the same grid hit resolution uses, so it
/// respects the ship's actual layout rather than a distance check.
fn spall_blocks(
    commands: &mut Commands,
    fx: &crate::vfx::effect_textures::EffectTextures,
    grid: &crate::building::ShipGrid,
    module_query: &mut Query<(&mut Module, &GlobalTransform), Without<DestroyedModule>>,
    hull_query: &mut Query<(&mut HullSegment, &GlobalTransform), Without<crate::components::HullDestroyed>>,
    profile: crate::combat::ammo_types::SpallProfile,
    from_cell: IVec2,
    dir_local: Vec2,
    damage: f32,
    exclude: Entity,
    world_at: Vec2,
    world_dir: Vec2,
) {
    if profile.fragments == 0 || damage <= 0.0 || dir_local == Vec2::ZERO {
        return;
    }
    let origin = from_cell.as_vec2();
    let half = profile.cone_degrees.to_radians();
    for i in 0..profile.fragments {
        // Spread evenly across the cone rather than randomly, so a narrow
        // profile reads as a focused jet instead of three coin flips.
        let t = if profile.fragments == 1 {
            0.0
        } else {
            (i as f32 / (profile.fragments - 1) as f32) * 2.0 - 1.0
        };
        let jitter = (rand::random::<f32>() - 0.5) * 0.15;
        let heading = Vec2::from_angle(t * half + jitter).rotate(dir_local);
        for step in grid.walk(origin, origin + heading * profile.reach) {
            if step.entity == exclude { continue; }
            let frag = damage * profile.damage_frac;
            if let Ok((mut module, gt)) = module_query.get_mut(step.entity) {
                module.health = (module.health - frag).max(0.0);
                spawn_floating_damage(commands, gt.translation().truncate(), frag, Color::srgb(0.95, 0.85, 0.6));
            } else if let Ok((mut hull, gt)) = hull_query.get_mut(step.entity) {
                hull.health = (hull.health - frag).max(0.0);
                spawn_floating_damage(commands, gt.translation().truncate(), frag, Color::srgb(0.95, 0.85, 0.6));
            } else {
                continue;
            }
            // Stops at the first thing it hits — that's the whole point.
            break;
        }
    }
    // Sparks blowing INWARD, so a breach reads differently from a bounce
    // (which sprays back out along the round's new heading).
    spawn_impact_sparks(commands, fx, world_at, world_dir, 0.5, 4 + profile.fragments as usize);
}

fn splash_blocks(
    commands: &mut Commands,
    children: &Children,
    module_query: &mut Query<(&mut Module, &GlobalTransform), Without<DestroyedModule>>,
    hull_query: &mut Query<(&mut HullSegment, &GlobalTransform), Without<crate::components::HullDestroyed>>,
    exclude: Entity,
    center: Vec2,
    radius: f32,
    damage: f32,
    edge: f32,
) {
    // `edge`: the share of the damage a block at the rim takes (1.0: the
    // same all the way out).
    let at = |pos: Vec2| damage * (1.0 + (edge - 1.0) * (center.distance(pos) / radius.max(1.0)));
    for child in children.iter() {
        if child == exclude { continue; }
        if let Ok((mut module, gt)) = module_query.get_mut(child) {
            let pos = gt.translation().truncate();
            if center.distance(pos) < radius {
                let damage = at(pos);
                module.health = (module.health - damage).max(0.0);
                spawn_floating_damage(commands, pos, damage, Color::srgb(1.0, 0.55, 0.2));
            }
        } else if let Ok((mut hull, gt)) = hull_query.get_mut(child) {
            let pos = gt.translation().truncate();
            if center.distance(pos) < radius {
                let damage = at(pos);
                hull.health = (hull.health - damage).max(0.0);
                spawn_floating_damage(commands, pos, damage, Color::srgb(1.0, 0.45, 0.25));
            }
        }
    }
}

/// Ticks incendiary burn on blocks: damage over time until the fire burns
/// out. Reports zero-amount AiShipDamaged so aggregate hull integrity
/// (process_ai_ship_damage_system) recalculates from the burned health.
pub fn tick_burning_blocks(
    time: Res<Time>,
    mut commands: Commands,
    mut burning_query: Query<(Entity, &mut BlockBurning, Option<&mut Module>, Option<&mut HullSegment>)>,
    mut ai_damage_events: MessageWriter<crate::events::AiShipDamaged>,
) {
    let dt = time.delta_secs();
    for (entity, mut burning, module, hull) in burning_query.iter_mut() {
        burning.remaining -= dt;
        let tick_damage = burning.dps * dt;
        if let Some(mut module) = module {
            module.health = (module.health - tick_damage).max(0.0);
        } else if let Some(mut hull) = hull {
            hull.health = (hull.health - tick_damage).max(0.0);
        }
        ai_damage_events.write(crate::events::AiShipDamaged {
            target: burning.ship,
            source: crate::events::DamageSource::Fire,
            amount: 0.0,
            position: None,
            direction: None,
            attacker: None,
        });
        if burning.remaining <= 0.0 {
            commands.entity(entity).remove::<BlockBurning>();
        }
    }
}

#[cfg(test)]
mod sweep_tests {
    use super::*;

    // Real numbers from the bug: cells are 66 units, the per-block hit radius
    // is 45.0, and base_projectile_speed puts a Railgun at 9000 u/s — 150
    // units per frame at 60fps, or 225 with APFSDS's 1.5x multiplier.
    const HIT_RADIUS: f32 = 45.0;
    const RAILGUN_STEP: f32 = 150.0;
    const APFSDS_STEP: f32 = 225.0;

    /// The tunneling case. A block 100 units along a 150-unit step was 100
    /// away when the frame began and 50 away when it ended — outside the
    /// 45-unit radius at BOTH sample points, so the old point test never saw
    /// it and the round flew through solid armour.
    #[test]
    fn fast_round_does_not_tunnel_through_a_block() {
        let (prev, cur) = (Vec2::ZERO, Vec2::new(RAILGUN_STEP, 0.0));
        let block = Vec2::new(100.0, 0.0);

        // What the old point test saw at each frame boundary: nothing.
        assert!(prev.distance(block) > HIT_RADIUS);
        assert!(cur.distance(block) > HIT_RADIUS);

        // What the sweep sees: a dead-centre hit, two thirds along.
        let (dist, t) = segment_closest(prev, cur, block);
        assert!(dist < HIT_RADIUS, "swept test still missed the block");
        assert!((t - 100.0 / RAILGUN_STEP).abs() < 1e-4);
    }

    /// APFSDS is the worst offender — highest speed, and the round whose whole
    /// purpose is punching armour. A 225-unit stride leaves a 135-unit blind
    /// spot, and cells are 66 apart, so consecutive cells could both vanish.
    #[test]
    fn apfsds_stride_covers_every_cell_it_crosses() {
        let (prev, cur) = (Vec2::ZERO, Vec2::new(APFSDS_STEP, 0.0));
        for block_x in [66.0, 132.0, 198.0] {
            let (dist, _) = segment_closest(prev, cur, Vec2::new(block_x, 0.0));
            assert!(dist < HIT_RADIUS, "cell at {block_x} was skipped");
        }
    }

    /// Ranked by entry order, not proximity. Over a long step the round should
    /// strike the outer plate it crossed first; the old "nearest centre to the
    /// current position" rank could pick a block BEHIND that plate.
    #[test]
    fn hits_are_ordered_by_entry_not_proximity() {
        let (prev, cur) = (Vec2::ZERO, Vec2::new(APFSDS_STEP, 0.0));
        let plate = Vec2::new(40.0, 10.0);
        let core = Vec2::new(200.0, 0.0);

        let (_, t_plate) = segment_closest(prev, cur, plate);
        let (_, t_core) = segment_closest(prev, cur, core);
        assert!(t_plate < t_core, "outer plate must be struck before the core");

        // ...and the old rank would have chosen the core.
        assert!(cur.distance(core) < cur.distance(plate));
    }

    /// A just-spawned round has a zero-length segment. It must degrade to a
    /// plain point test rather than dividing by zero.
    #[test]
    fn zero_length_step_falls_back_to_a_point_test() {
        let p = Vec2::new(10.0, 20.0);
        let (dist, t) = segment_closest(p, p, Vec2::new(10.0, 50.0));
        assert!((dist - 30.0).abs() < 1e-4);
        assert_eq!(t, 0.0);
    }

    /// The reported tunnelling case: a railgun step that starts outside the
    /// bubble and ENDS past the centre. The end point sits on the far side,
    /// outside a front-facing arc, so the old end-point test let it through;
    /// the crossing point is on the near side, inside the arc.
    #[test]
    fn a_fast_round_meets_the_shield_where_it_crossed() {
        let center = Vec2::ZERO;
        let radius = 200.0;
        let (prev, cur) = (Vec2::new(-210.0, 0.0), Vec2::new(15.0, 0.0));
        let at = shield_contact(prev, cur, center, radius).expect("the step crossed the bubble");
        assert!((at - Vec2::new(-200.0, 0.0)).length() < 1e-3, "entered at {at:?}");
        // A shield lit toward -x covers the entry, not the end point.
        let facing = Vec2::NEG_X;
        assert!(facing.dot((at - center).normalize()) > 0.9);
        assert!(facing.dot((cur - center).normalize()) < 0.0);
    }

    /// A step that never reaches the bubble isn't in it, and one that started
    /// inside is judged where it finishes, as it always was.
    #[test]
    fn shield_contact_misses_and_insides() {
        let r = 100.0;
        assert_eq!(shield_contact(Vec2::new(-300.0, 150.0), Vec2::new(300.0, 150.0), Vec2::ZERO, r), None);
        assert_eq!(shield_contact(Vec2::new(-300.0, 0.0), Vec2::new(-150.0, 0.0), Vec2::ZERO, r), None);
        let inside_end = Vec2::new(10.0, 0.0);
        assert_eq!(shield_contact(Vec2::new(-10.0, 0.0), inside_end, Vec2::ZERO, r), Some(inside_end));
        assert_eq!(shield_contact(Vec2::new(-10.0, 0.0), Vec2::new(300.0, 0.0), Vec2::ZERO, r), None);
    }

    /// Closest approach is clamped to the segment: a block behind the muzzle or
    /// beyond this frame's end is not hit by this step.
    #[test]
    fn sweep_does_not_extend_past_the_segment() {
        let (prev, cur) = (Vec2::ZERO, Vec2::new(RAILGUN_STEP, 0.0));

        let (d_behind, t_behind) = segment_closest(prev, cur, Vec2::new(-200.0, 0.0));
        assert_eq!(t_behind, 0.0);
        assert!(d_behind > HIT_RADIUS);

        let (d_ahead, t_ahead) = segment_closest(prev, cur, Vec2::new(400.0, 0.0));
        assert_eq!(t_ahead, 1.0);
        assert!(d_ahead > HIT_RADIUS);
    }
}

/// One resolver for every ship: an AI round into the PLAYER's hull, through
/// the real check_projectile_hits.
#[cfg(test)]
mod shared_hit_tests {
    use super::*;
    use crate::ai_ship::components::AiShip;
    use crate::building::ShipGrid;
    use crate::combat::shields::ShipShield;
    use crate::components::{HullLayer, HullMaterial};
    use bevy::platform::collections::HashMap;

    fn shield_down() -> ShipShield {
        ShipShield {
            current: 0.0, max: 0.0, base_max: 0.0, recharge_rate: 0.0, recharge_delay: 0.0,
            since_hit: 999.0, radius: 400.0, center_offset: Vec2::ZERO, flash: 0.0, enabled: true,
            surge: 0.0, arc_facing: 0.0, arc_half: std::f32::consts::PI, arc_traverse: 0.0, arc_active: false,
        }
    }

    fn plate(cell: IVec2) -> HullSegment {
        HullSegment {
            health: 100.0, max_health: 100.0, radiation_shielding: 0.0, is_depressurized: false,
            depressurization_level: 0.0, hull_layer: HullLayer::Outer, material: HullMaterial::Steel,
            grid_position: cell,
        }
    }

    /// A player ship with a row of plate at y = 0, x = 0..3; an AI gun; one
    /// of its rounds about to cross cell (0, 0) head-on from below.
    fn setup(god_mode: bool) -> (App, Entity, Vec<Entity>) {
        let mut app = App::new();
        app.insert_resource(crate::vfx::effect_textures::EffectTextures::empty());
        app.init_resource::<crate::spatial::CreatureGrid>();
        app.init_resource::<crate::building::rooms::RoomMap>();
        app.insert_resource(crate::debug::DebugTuning { god_mode, ..default() });
        app.add_message::<crate::events::AiShipDamaged>();
        app.add_message::<crate::events::ShipDamaged>();
        app.add_message::<crate::events::HullBreached>();
        app.add_message::<crate::events::RoomDepressurized>();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, check_projectile_hits);

        let player = app.world_mut()
            .spawn((Ship, Transform::default(), GlobalTransform::default(), shield_down()))
            .id();
        let mut cells = HashMap::new();
        let mut plates = Vec::new();
        for x in 0..3 {
            let cell = IVec2::new(x, 0);
            let local = crate::building::grid_to_local(cell).extend(0.0);
            let id = app.world_mut().spawn((
                plate(cell), Transform::from_translation(local), GlobalTransform::from_translation(local), ChildOf(player),
            )).id();
            cells.insert(cell, id);
            plates.push(id);
        }
        app.world_mut().entity_mut(player).insert(ShipGrid { cells });

        let enemy = app.world_mut().spawn((AiShip, Transform::from_xyz(0.0, -5000.0, 0.0), GlobalTransform::default(), shield_down())).id();
        let gun = app.world_mut().spawn(ChildOf(enemy)).id();
        app.world_mut().spawn((
            Sprite::default(),
            Transform::from_translation(crate::building::grid_to_local(IVec2::new(0, 1)).extend(0.5)),
            Projectile {
                damage: 40.0, speed: 2000.0, lifetime: 1.0, max_lifetime: 1.0, owner: gun,
                damage_type: ProjectileDamageType::Kinetic, penetration: 20.0, has_penetrated: false,
                ammo: None, effect: None, caliber: 1.0, last_hit: None,
                // Last frame it was two cells below the row: this step crosses (0, 0).
                prev_pos: crate::building::grid_to_local(IVec2::new(0, -2)),
                bounces: 0,
                flak: None,
            },
            Velocity(Vec2::new(0.0, 2000.0)),
        ));
        (app, player, plates)
    }

    #[test]
    fn an_enemy_round_damages_the_player_plate_it_crosses_and_says_so() {
        let (mut app, _, plates) = setup(false);
        app.update();
        let hp = app.world().get::<HullSegment>(plates[0]).unwrap().health;
        assert!(hp < 100.0, "the plate on the round's line took the hit ({hp})");
        assert_eq!(app.world().get::<HullSegment>(plates[1]).unwrap().health, 100.0, "its neighbour did not");
        let hits: Vec<_> = app.world_mut().resource_mut::<Messages<crate::events::ShipDamaged>>().drain().collect();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].applied, "already applied, so process_ship_damage won't apply it twice");
        assert!(matches!(hits[0].source, crate::events::DamageSource::Gunfire));
    }

    /// A turret standing on the player's ship, its centre over hull cell
    /// (1, 0), so its cell (-1, 0) is over hull cell (0, 0) -- the round's line.
    fn add_turret(app: &mut App, player: Entity, plated: bool, barrel: bool) -> (Entity, Option<Entity>, Option<Entity>) {
        use crate::combat::turret_deck::{TurretBody, TurretPlate};
        let at = crate::building::grid_to_local(IVec2::new(1, 0)).extend(1.2);
        let body = app.world_mut().spawn((
            TurretBody::new(Entity::PLACEHOLDER, IVec2::ZERO, 1),
            Transform::from_translation(at), GlobalTransform::from_translation(at), ChildOf(player),
        )).id();
        let cell = IVec2::new(-1, 0);
        let world = at + Vec3::new(-66.0, 0.0, 0.0);
        let mut cells = HashMap::new();
        let barrel_e = barrel.then(|| {
            let e = app.world_mut().spawn((
                Module {
                    module_type: ModuleType::GunBarrel, health: 50.0, max_health: 50.0, power_consumption: 0.0,
                    power_generation: 0.0, is_active: true, grid_position: cell, size: IVec2::ONE, rotation: Rotation::East,
                },
                GlobalTransform::from_translation(world), ChildOf(body),
            )).id();
            cells.insert(cell, e);
            e
        });
        let plate_e = plated.then(|| {
            let e = app.world_mut().spawn((plate(cell), TurretPlate, GlobalTransform::from_translation(world), ChildOf(body))).id();
            cells.insert(cell, e);
            e
        });
        app.world_mut().entity_mut(body).insert(ShipGrid { cells });
        (body, plate_e, barrel_e)
    }

    #[test]
    fn where_a_turret_stands_over_the_hull_the_round_meets_the_turret() {
        let (mut app, player, plates) = setup(false);
        let (_, turret_plate, _) = add_turret(&mut app, player, true, false);
        app.update();
        let hp = app.world().get::<HullSegment>(turret_plate.unwrap()).unwrap().health;
        assert!(hp < 100.0, "the turret's plating took it ({hp})");
        assert_eq!(app.world().get::<HullSegment>(plates[0]).unwrap().health, 100.0, "the hull under it did not");
    }

    #[test]
    fn a_barrel_reaching_over_the_hull_is_no_cover() {
        let (mut app, player, plates) = setup(false);
        let (_, _, barrel) = add_turret(&mut app, player, false, true);
        app.update();
        assert!(app.world().get::<HullSegment>(plates[0]).unwrap().health < 100.0, "the round went on to the hull");
        assert_eq!(app.world().get::<Module>(barrel.unwrap()).unwrap().health, 50.0);
    }

    #[test]
    fn god_mode_spares_the_player() {
        let (mut app, _, plates) = setup(true);
        app.update();
        assert_eq!(app.world().get::<HullSegment>(plates[0]).unwrap().health, 100.0);
    }
}

#[cfg(test)]
mod pierce_tests {
    use super::pierce_onward;

    /// Blocks of 100 health, steel-plated (15), hit by the Halyard's 280 mm
    /// round: 434 damage, ~142 penetration, 0.2 lost per hole.
    fn blocks_through(mut damage: f32, mut pen: f32, health: f32, plate: f32) -> u32 {
        let mut n = 0;
        let mut first = true;
        loop {
            n += 1;
            let overkill = (damage - health).max(0.0);
            pen -= plate.max(super::PIERCE_COST_MIN);
            match pierce_onward(damage, overkill, pen, first, 0.2) {
                Some(d) if n < 50 => damage = d,
                _ => return n,
            }
            first = false;
        }
    }

    #[test]
    fn a_heavy_round_bores_through_several_blocks() {
        assert!(blocks_through(434.0, 142.0, 100.0, 15.0) >= 4);
    }

    #[test]
    fn a_light_round_still_gets_its_second_block_and_no_more() {
        assert_eq!(blocks_through(87.0, 47.0, 100.0, 15.0), 2);
    }

    #[test]
    fn armour_stops_what_damage_alone_would_not() {
        // The same round into 40-thick plate runs out of penetration first.
        assert!(blocks_through(434.0, 142.0, 30.0, 40.0) <= 5);
        assert!(blocks_through(434.0, 142.0, 30.0, 0.0) > 5);
    }
}
