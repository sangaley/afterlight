use bevy::prelude::*;
use crate::components::*;
use crate::events::*;
use crate::celestial::components::{GravityAffected, GravityForce};

// ============================================================================
// SECTION SEVERANCE
// When the connection between ship sections is destroyed, the detached
// section breaks off with ship velocity + impact force.
// Becomes debris. If it had reactor/ammo = tumbling bomb.
// ============================================================================

/// Marker for a detached ship section — flying debris with momentum
#[derive(Component)]
pub struct DetachedSection {
    pub has_reactor: bool,
    pub has_ammo: bool,
    pub has_fuel: bool,
    pub meltdown_timer: Option<f32>,
    pub mass: f32,
    /// Radians a second it tumbles: what the blast's off-centre shove gave
    /// it. A chunk spins; half a ship barely turns.
    pub spin: f32,
    /// Seconds adrift, so a long fight doesn't pile up debris for good.
    pub age: f32,
    /// Off the player's own ship, so its alarms are the player's business.
    pub ours: bool,
}

/// Blocks a section needs before it holds together: a smaller one with a
/// reactor or ammo aboard is a bomb, a bigger one is half a ship adrift.
const BOMB_BLOCKS_MAX: f32 = 40.0;
/// Mass of one block, for how a blast's shove shares out.
const BLOCK_MASS: f32 = 8.0;
/// The shove of a section's joints letting go, with no blast to speak of:
/// two halves of a thousand blocks part at about 50 u/s each.
const BREAK_IMPULSE: f32 = 400_000.0;
/// What each block the blast just tore out adds to the shove: the hole is the
/// one measure of a blast that every weapon leaves alike.
const BLAST_IMPULSE_PER_BLOCK: f32 = 30_000.0;
/// Most blocks that count, so a cook-off doesn't fling half a ship.
const BLAST_BLOCKS_CAP: f32 = 40.0;
/// Fastest a chunk leaves the ship it came from.
const MAX_PARTING_SPEED: f32 = 320.0;
/// Fastest a section tumbles.
const MAX_SPIN: f32 = 1.5;
/// Seconds a section stays adrift before it's cleared away once out of
/// sight: a chunk goes soon, a real piece of ship lingers.
const CHUNK_LIFE: f32 = 30.0;
const SECTION_LIFE: f32 = 180.0;
/// Blocks a piece needs to count as a real piece of ship.
const CHUNK_BLOCKS_MAX: f32 = 12.0;
/// Nothing is cleared within this of the player: it would pop out of view.
const CLEAR_DISTANCE: f32 = 9_000.0;
/// How often an AI ship is checked for breaking up: finding what still joins
/// what walks every block, and one under fire changes nearly every frame.
const AI_CHECK_EVERY_S: f32 = 0.2;

fn shove(blast_blocks: f32) -> f32 {
    BREAK_IMPULSE + blast_blocks.clamp(0.0, BLAST_BLOCKS_CAP) * BLAST_IMPULSE_PER_BLOCK
}

/// How two parts of a ship part: the blast's shove, the same on both, so
/// each goes off at that over its own mass -- a chunk flies, half a ship
/// drifts, the rest of the ship recoils. A chunk at the speed limit carries
/// only the shove that got it there, so the ship doesn't recoil from a loose
/// plate as if from half its hull. Returns the section's and the main body's
/// change in speed (along the way the section goes, and against it).
pub fn parting_speeds(section_blocks: usize, main_blocks: usize, blast_blocks: f32) -> (f32, f32) {
    let impulse = shove(blast_blocks);
    let section_mass = section_blocks.max(1) as f32 * BLOCK_MASS;
    let main_mass = main_blocks.max(1) as f32 * BLOCK_MASS;
    let section = (impulse / section_mass).min(MAX_PARTING_SPEED);
    (section, section * section_mass / main_mass)
}

/// How fast a section tumbles: the shove, off its centre by `arm` units,
/// against its turning inertia (a disc of its size).
pub fn parting_spin(section_blocks: usize, radius: f32, arm: f32, blast_blocks: f32) -> f32 {
    let impulse = shove(blast_blocks);
    let mass = section_blocks.max(1) as f32 * BLOCK_MASS;
    let inertia = 0.5 * mass * radius.max(66.0).powi(2);
    (impulse * arm / inertia).clamp(-MAX_SPIN, MAX_SPIN)
}

/// The blocks a ship lost just now, and where in the ship the last of them
/// stood: the blast that breaks a section off.
#[derive(Clone, Copy, Default)]
struct RecentBlast {
    blocks: f32,
    at: Option<Vec2>,
}

/// Marks a block that broke away with a severed section. It is still a hull
/// segment or module -- it draws, burns and cooks off like one -- but it is
/// no longer the player's, so saving leaves it out.
#[derive(Component)]
pub struct SeveredBlock;

/// A section breaks away when nothing joins it to the ship any more.
///
/// After any block of a ship is destroyed, its live blocks are grouped by
/// what touches what -- sides AND corners, because hulls step diagonally and
/// a wedge meets its neighbour at a corner. The biggest group is the ship.
/// Every other group that was joined to it a moment ago breaks off as ONE
/// body: a new root carrying the blocks in their places, its own collider,
/// and the blast's shove shared between the pieces by their mass
/// (`parting_speeds`), the main body recoiling the other way.
///
/// Every ship, the player's and the AI's, and the wrecks they leave: an enemy
/// cut in half used to stay one rigid body with a gap in it. Only destruction
/// severs; removing blocks in build mode never does.
#[allow(clippy::too_many_arguments)]
pub fn check_section_severance(
    mut commands: Commands,
    time: Res<Time>,
    mut ships: Query<
        (Entity, &Transform, &mut Velocity, Option<&mut crate::ship::collision::KnockVelocity>, &crate::building::layout::LayoutVersion, &Children, Option<&crate::ai_ship::components::AiShipState>),
        Or<(With<Ship>, With<crate::ai_ship::components::AiShip>)>,
    >,
    fresh_destroyed: Query<(&ChildOf, &Transform), Or<(Added<HullDestroyed>, Added<DestroyedModule>)>>,
    hulls: Query<(&HullSegment, &Transform), Without<HullDestroyed>>,
    modules: Query<(&Module, &Transform), Without<DestroyedModule>>,
    mut state: Local<SeveranceState>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let now = time.elapsed_secs();
    let dt = time.delta_secs();
    let state = &mut *state;

    // What each ship has just lost, fading in about a second.
    for blast in state.blasts.values_mut() {
        blast.blocks *= (-dt / 0.5).exp();
    }
    for (parent, transform) in fresh_destroyed.iter() {
        let blast = state.blasts.entry(parent.parent()).or_default();
        blast.blocks += 1.0;
        blast.at = Some(transform.translation.truncate());
        state.pending.insert(parent.parent());
    }

    let mut alive = bevy::platform::collections::HashSet::new();
    for (ship, ship_transform, mut ship_velocity, mut knock, version, children, ai_state) in ships.iter_mut() {
        alive.insert(ship);
        let entry = state.ships.entry(ship).or_default();
        let destroyed_now = state.pending.contains(&ship);
        let known = entry.attached.is_some();
        if known && entry.seen == Some(*version) && !destroyed_now {
            continue;
        }
        // An AI ship waits its turn; the player's breaks up on the frame.
        if ai_state.is_some() && known && now - entry.checked < AI_CHECK_EVERY_S {
            continue;
        }
        entry.seen = Some(*version);
        entry.checked = now;
        state.pending.remove(&ship);

        // Live blocks by cell, and where each block sits in the ship.
        // A cell can hold several: floor, plate, and the module standing on them.
        let mut cell_block: bevy::platform::collections::HashMap<IVec2, Vec<Entity>> = Default::default();
        let mut local_pos: bevy::platform::collections::HashMap<Entity, Vec2> = Default::default();
        for child in children.iter() {
            if let Ok((hull, transform)) = hulls.get(child) {
                cell_block.entry(hull.grid_position).or_default().push(child);
                local_pos.insert(child, transform.translation.truncate());
            } else if let Ok((module, transform)) = modules.get(child) {
                let footprint = crate::building::footprints::footprint_override(module.module_type);
                for cell in crate::building::ShipGrid::cells_for(module.grid_position, module.size, module.rotation, footprint) {
                    cell_block.entry(cell).or_default().push(child);
                }
                local_pos.insert(child, transform.translation.truncate());
            }
        }

        let groups = touching_groups(&cell_block);
        let Some(main) = groups.iter().max_by_key(|g| g.len()) else { continue };

        // First look at this ship, or nothing destroyed: learn what is
        // attached, sever nothing.
        let Some(attached) = entry.attached.as_ref().filter(|_| destroyed_now) else {
            entry.attached = Some(main.clone());
            continue;
        };

        let centre_of = |group: &bevy::platform::collections::HashSet<Entity>| -> Vec2 {
            let points: Vec<Vec2> = group.iter().filter_map(|e| local_pos.get(e).copied()).collect();
            points.iter().sum::<Vec2>() / points.len().max(1) as f32
        };
        let main_centre = centre_of(main);
        let blast = state.blasts.get(&ship).copied().unwrap_or_default();
        for group in &groups {
            if std::ptr::eq(group, main) || !group.iter().any(|e| attached.contains(e)) {
                continue;
            }
            let blocks: Vec<(Entity, Vec2)> = group.iter().filter_map(|e| local_pos.get(e).map(|p| (*e, *p))).collect();
            let module_types: Vec<ModuleType> = group.iter().filter_map(|e| modules.get(*e).ok().map(|(m, _)| m.module_type)).collect();
            let recoil = break_off(
                &mut commands,
                if ai_state.is_none() { Some(&mut notifications) } else { None },
                ai_state.is_some(),
                ship_transform,
                &ship_velocity,
                &blocks,
                &module_types,
                main_centre,
                main.len(),
                blast,
            );
            // An AI ship's engines own its velocity and would swallow this;
            // the knock channel is how anything shoves one.
            match knock.as_deref_mut() {
                Some(knock) => knock.0 += recoil,
                None => ship_velocity.0 += recoil,
            }
        }
        entry.attached = Some(main.clone());
    }
    state.ships.retain(|e, _| alive.contains(e));
    state.blasts.retain(|e, _| alive.contains(e));
    state.pending.retain(|e| alive.contains(e));
}

/// What the severance pass remembers from frame to frame.
#[derive(Default)]
pub struct SeveranceState {
    ships: bevy::platform::collections::HashMap<Entity, ShipWatch>,
    blasts: bevy::platform::collections::HashMap<Entity, RecentBlast>,
    /// Ships that lost a block since their last check.
    pending: bevy::platform::collections::HashSet<Entity>,
}

/// One ship, as last checked.
#[derive(Default)]
struct ShipWatch {
    seen: Option<crate::building::layout::LayoutVersion>,
    checked: f32,
    /// The blocks joined to its main body the last time this looked. Only
    /// these can break away: a piece a designer left floating on purpose was
    /// never attached, so losing a plate elsewhere doesn't fling it off.
    attached: Option<bevy::platform::collections::HashSet<Entity>>,
}

/// Groups of blocks joined by a side or a corner.
fn touching_groups(
    cell_block: &bevy::platform::collections::HashMap<IVec2, Vec<Entity>>,
) -> Vec<bevy::platform::collections::HashSet<Entity>> {
    let mut seen_cells: bevy::platform::collections::HashSet<IVec2> = Default::default();
    let mut groups = Vec::new();
    for &start in cell_block.keys() {
        if !seen_cells.insert(start) {
            continue;
        }
        let mut group: bevy::platform::collections::HashSet<Entity> = Default::default();
        let mut stack = vec![start];
        while let Some(cell) = stack.pop() {
            group.extend(cell_block[&cell].iter().copied());
            for dx in -1..=1 {
                for dy in -1..=1 {
                    let next = cell + IVec2::new(dx, dy);
                    if cell_block.contains_key(&next) && seen_cells.insert(next) {
                        stack.push(next);
                    }
                }
            }
        }
        groups.push(group);
    }
    groups
}

/// Re-root one severed group under a new body, in place, and send it off.
/// Returns the main body's recoil.
#[allow(clippy::too_many_arguments)]
fn break_off(
    commands: &mut Commands,
    notifications: Option<&mut MessageWriter<ShowNotification>>,
    from_ai: bool,
    ship_transform: &Transform,
    ship_velocity: &Velocity,
    blocks: &[(Entity, Vec2)],
    module_types: &[ModuleType],
    main_centre: Vec2,
    main_blocks: usize,
    blast: RecentBlast,
) -> Vec2 {
    if blocks.is_empty() {
        return Vec2::ZERO;
    }
    let centroid = blocks.iter().map(|(_, p)| *p).sum::<Vec2>() / blocks.len() as f32;
    let has_reactor = module_types.iter().any(|t| matches!(t,
        ModuleType::SmallReactor | ModuleType::StandardReactor | ModuleType::LargeReactor | ModuleType::FusionReactor));
    let has_ammo = module_types.iter().any(|t| matches!(t, ModuleType::AmmoFeedUnit | ModuleType::WarheadBay));
    let has_fuel = module_types.contains(&ModuleType::FuelTank);
    let mass = (blocks.len() as f32 * BLOCK_MASS).max(50.0);
    let bomb = (blocks.len() as f32) <= BOMB_BLOCKS_MAX;

    // The new root sits where the section's centre is, turned like the ship,
    // so the blocks keep their exact places and it tumbles about its middle.
    let world_centre = ship_transform.transform_point(centroid.extend(0.0));
    let world_main = ship_transform.transform_point(main_centre.extend(0.0)).truncate();
    // Away from the rest of the ship, and away from the blast.
    let from_main = (world_centre.truncate() - world_main).normalize_or_zero();
    let blast_at = blast.at.map(|at| ship_transform.transform_point(at.extend(0.0)).truncate());
    let from_blast = blast_at.map_or(Vec2::ZERO, |at| (world_centre.truncate() - at).normalize_or_zero());
    let away = (from_main + from_blast * 0.5).normalize_or(Vec2::X);
    let (section_speed, main_speed) = parting_speeds(blocks.len(), main_blocks, blast.blocks);
    let velocity = ship_velocity.0 + away * section_speed;
    // Spin: how far off the section's centre the shove came from, and which way.
    let radius = blocks.iter().map(|(_, p)| p.distance(centroid)).fold(66.0, f32::max);
    let lever = blast_at.map_or(0.0, |at| (at - world_centre.truncate()).perp_dot(away));
    let spin = parting_spin(blocks.len(), radius, lever, blast.blocks);

    let points: Vec<Vec2> = blocks.iter().map(|(_, p)| *p - centroid).collect();
    let root = commands.spawn((
        Name::new("Severed section"),
        Transform { translation: world_centre, rotation: ship_transform.rotation, scale: Vec3::ONE },
        Visibility::default(),
        DetachedSection {
            has_reactor: has_reactor && bomb,
            has_ammo: has_ammo && bomb,
            has_fuel,
            meltdown_timer: if has_reactor && bomb { Some(10.0) } else { None },
            mass,
            spin,
            age: 0.0,
            ours: !from_ai,
        },
        Velocity(velocity),
        GravityAffected { mass },
        GravityForce::default(),
        // A real body: it caroms off hulls and rocks instead of ghosting
        // through. Shots still pass through debris (collision::TerrainFilter).
        crate::ship::collision::fit_ship_collider(&points, mass),
    )).id();
    for (block, local) in blocks {
        let at = (*local - centroid).extend(0.0);
        commands
            .entity(*block)
            .try_insert((SeveredBlock, ChildOf(root)))
            // Drawn again if the far view had it hidden behind the ship's
            // picture.
            .remove::<(bevy::camera::visibility::RenderLayers, crate::vfx::far_art::FarHidden)>()
            .entry::<Transform>()
            .and_modify(move |mut t| {
                t.translation.x = at.x;
                t.translation.y = at.y;
            });
    }

    // Still an AI ship's block to everything asking "is this the player's?",
    // but owned by the wreckage: not counted, powered or fired by the ship it
    // left.
    if from_ai {
        for (block, _) in blocks {
            commands.entity(*block).try_insert(crate::ai_ship::components::OwnedByAiShip { root });
        }
    }

    if crate::combat::gun_assembly::trace() {
        info!("SEVERED {} blocks off a ship of {}: parting {:.0} u/s, recoil {:.1}, spin {:.2}, blast of {:.0} blocks", blocks.len(), main_blocks, section_speed, main_speed, spin, blast.blocks);
    }

    if let Some(notifications) = notifications {
        notifications.write(ShowNotification {
            message: format!("SECTION SEVERED! {} blocks broke away", blocks.len()),
            notification_type: NotificationType::Danger,
            duration: 4.0,
        });
        if has_reactor && bomb {
            notifications.write(ShowNotification {
                message: "WARNING: Severed section contains reactor! Meltdown in 10s!".into(),
                notification_type: NotificationType::Danger,
                duration: 5.0,
            });
        }
    }
    -away * main_speed
}

/// Test hook: DEPTHS_CUT_AT=<secs> cuts a band of blocks across the waist of
/// the nearest AI ship (DEPTHS_CUT=player: your own), to see a ship come apart
/// without waiting for a fight to happen to do it.
pub fn cut_hook_on() -> bool {
    std::env::var_os("DEPTHS_CUT_AT").is_some()
}

pub fn cut_test_hook(
    time: Res<Time>,
    mut commands: Commands,
    player: Query<(Entity, &Transform, &Children), With<Ship>>,
    ai: Query<(Entity, &Transform, &Children), With<crate::ai_ship::components::AiShip>>,
    hulls: Query<&HullSegment, Without<HullDestroyed>>,
    mut modules: Query<&mut Module, Without<DestroyedModule>>,
    mut done: Local<bool>,
) {
    let at = std::env::var("DEPTHS_CUT_AT").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0);
    if *done || time.elapsed_secs() < at {
        return;
    }
    let Ok((own, own_tf, own_children)) = player.single() else { return };
    let target = if std::env::var("DEPTHS_CUT").is_ok_and(|v| v == "player") {
        Some((own, own_children))
    } else {
        ai.iter()
            .min_by_key(|(_, t, _)| t.translation.distance(own_tf.translation) as i64)
            .map(|(e, _, c)| (e, c))
    };
    let Some((ship, children)) = target else { return };
    *done = true;
    let cells: Vec<IVec2> = children.iter().filter_map(|c| hulls.get(c).ok().map(|h| h.grid_position)).collect();
    let (Some(min), Some(max)) = (cells.iter().map(|c| c.x).min(), cells.iter().map(|c| c.x).max()) else { return };
    let waist = (min + max) / 2;
    let mut cut = 0;
    for child in children.iter() {
        let in_band = |x: i32| (x - waist).abs() <= 4;
        if hulls.get(child).is_ok_and(|h| in_band(h.grid_position.x)) {
            commands.entity(child).try_insert((HullDestroyed, PendingRemoval { timer: Timer::from_seconds(0.5, TimerMode::Once) }));
            cut += 1;
        } else if let Ok(mut module) = modules.get_mut(child) {
            if in_band(module.grid_position.x) {
                module.health = 0.0;
                commands.entity(child).try_insert((DestroyedModule { original_type: module.module_type }, PendingRemoval { timer: Timer::from_seconds(0.5, TimerMode::Once) }));
                cut += 1;
            }
        }
    }
    info!("CUT {cut} blocks across the waist of {ship:?}");
}

/// A turret's blocks hang off its turret, not the deck, so when its ring goes
/// off with a section (`turret_deck` moves the turret) they still name the
/// ship they left, and would go on counting as its guns.
pub fn severed_turrets_change_hands(
    sections: Query<(Entity, &Children), (With<DetachedSection>, Changed<Children>)>,
    turrets: Query<&Children, With<crate::combat::turret_deck::TurretBody>>,
    mut owned: Query<&mut crate::ai_ship::components::OwnedByAiShip>,
) {
    for (section, children) in sections.iter() {
        for child in children.iter() {
            let Ok(parts) = turrets.get(child) else { continue };
            for part in parts.iter() {
                if let Ok(mut owner) = owned.get_mut(part) {
                    owner.root = section;
                }
            }
        }
    }
}

/// Move detached sections — they tumble and drift, affected by gravity
pub fn move_detached_sections(
    time: Res<Time>,
    mut commands: Commands,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    mut section_query: Query<(Entity, &mut DetachedSection, &mut Transform, &Velocity)>,
    player: Query<&Transform, (With<Ship>, Without<DetachedSection>)>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let dt = time.delta_secs();
    let player_pos = player.single().ok().map(|t| t.translation.truncate());

    for (entity, mut section, mut transform, velocity) in section_query.iter_mut() {
        // Move. Gravity is already in Velocity (apply_gravity_to_velocity
        // integrates it for everything GravityAffected); adding the raw
        // acceleration to POSITION here was a second pull in the wrong units.
        transform.translation.x += velocity.0.x * dt;
        transform.translation.y += velocity.0.y * dt;

        // Tumble, as hard as the blast turned it.
        transform.rotation *= Quat::from_rotation_z(section.spin * dt);

        // Cleared away in time, so a long fight doesn't pile up debris.
        section.age += dt;
        if crate::combat::gun_assembly::trace() && (section.age * 0.5).floor() != ((section.age - dt) * 0.5).floor() {
            let off = player_pos.map_or(0.0, |p| p.distance(transform.translation.truncate()));
            info!("SECTION {:?} age {:.0} mass {:.0} speed {:.0} {} from you", entity, section.age, section.mass, velocity.0.length(), off as i32);
        }
        let life = if section.mass <= CHUNK_BLOCKS_MAX * BLOCK_MASS { CHUNK_LIFE } else { SECTION_LIFE };
        let in_sight = player_pos.is_some_and(|p| p.distance(transform.translation.truncate()) < CLEAR_DISTANCE);
        if section.age > life && !in_sight {
            commands.entity(entity).try_despawn();
            continue;
        }

        // Reactor meltdown countdown
        if let Some(ref mut timer) = section.meltdown_timer {
            *timer -= dt;
            if *timer <= 0.0 {
                // BOOM
                if section.ours {
                    notifications.write(ShowNotification {
                        message: "Severed reactor EXPLODED!".into(),
                        notification_type: NotificationType::Danger,
                        duration: 3.0,
                    });
                }

                let pos = transform.translation.truncate();
                super::spawn_explosion(&mut commands, &fx, pos, 90.0, Color::srgb(1.0, 0.5, 0.1));

                commands.entity(entity).try_despawn();
                // This section is gone; don't fall through to the ammo
                // cook-off below and queue a second despawn for the same
                // entity. A severed reactor section usually has ammo aboard
                // too, so both branches fired on the same debris and the
                // second one errored ("Entity despawned") in the middle of a
                // fight.
                continue;
            }
        }

        // Ammo cook-off (random chance each second)
        if section.has_ammo && rand::random::<f32>() < 0.02 * dt {
            let pos = transform.translation.truncate();
            super::spawn_explosion(&mut commands, &fx, pos, 36.0, Color::srgb(1.0, 0.4, 0.1));
            if section.ours {
                notifications.write(ShowNotification {
                    message: "Ammo cook-off in debris!".into(),
                    notification_type: NotificationType::Warning,
                    duration: 2.0,
                });
            }
            commands.entity(entity).try_despawn();
        }
    }
}

/// Detached sections can collide with creatures and damage them
pub fn debris_collision(
    mut commands: Commands,
    section_query: Query<(Entity, &DetachedSection, &Transform, &Velocity)>,
    mut creature_query: Query<(Entity, &Transform, &mut Creature), Without<DetachedSection>>,
) {
    for (section_entity, section, section_transform, section_vel) in section_query.iter() {
        let section_pos = section_transform.translation.truncate();
        let speed = section_vel.0.length();

        if speed < 20.0 { continue; } // Not moving fast enough to hurt

        for (_creature_entity, creature_transform, mut creature) in creature_query.iter_mut() {
            if creature.health <= 0.0 { continue; }

            let creature_pos = creature_transform.translation.truncate();
            let dist = section_pos.distance(creature_pos);

            if dist < 60.0 {
                // COLLISION! Damage based on speed and mass
                let impact_damage = speed * section.mass * 0.01;
                creature.health -= impact_damage;

                super::spawn_hit_effect(&mut commands, section_pos, Color::srgb(0.8, 0.6, 0.2), 20.0);
                super::spawn_floating_damage(&mut commands, section_pos, impact_damage, Color::srgb(0.7, 0.5, 0.2));

                // Destroy the debris on impact
                commands.entity(section_entity).try_despawn();
                break;
            }
        }
    }
}

#[cfg(test)]
mod severance_tests {
    use super::*;

    fn hull_at(x: i32, y: i32) -> (HullSegment, Transform) {
        (
            HullSegment {
                health: 100.0,
                max_health: 100.0,
                radiation_shielding: 0.0,
                is_depressurized: false,
                depressurization_level: 0.0,
                hull_layer: HullLayer::Inner,
                material: HullMaterial::Steel,
                grid_position: IVec2::new(x, y),
            },
            Transform::from_xyz(x as f32 * 66.0, y as f32 * 66.0 - 33.0, 0.0),
        )
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(crate::building::layout::LayoutPlugin);
        app.add_message::<ShowNotification>();
        app.init_resource::<Time>();
        app.add_systems(Update, check_section_severance);
        app
    }

    /// A ship that is a row of hull at x = 0..len, plus any extra cells.
    fn ship(app: &mut App, len: i32, extra: &[(i32, i32)]) -> (Entity, Vec<Entity>) {
        let ship = app.world_mut().spawn((Ship, Transform::default(), Velocity(Vec2::ZERO))).id();
        let mut blocks: Vec<Entity> = (0..len).map(|x| app.world_mut().spawn((hull_at(x, 0), ChildOf(ship))).id()).collect();
        for &(x, y) in extra {
            blocks.push(app.world_mut().spawn((hull_at(x, y), ChildOf(ship))).id());
        }
        app.update(); // learn the attached body
        (ship, blocks)
    }

    fn sections(app: &mut App) -> Vec<Entity> {
        app.world_mut().query_filtered::<Entity, With<DetachedSection>>().iter(app.world()).collect()
    }

    #[test]
    fn cutting_a_row_in_two_sends_the_smaller_end_off_as_one_piece() {
        let mut app = app();
        let (ship, blocks) = ship(&mut app, 7, &[]);
        app.world_mut().entity_mut(blocks[4]).insert(HullDestroyed);
        app.update();
        let found = sections(&mut app);
        assert_eq!(found.len(), 1, "one severed section, not one per block");
        for &b in &blocks[5..] {
            assert_eq!(app.world().get::<ChildOf>(b).unwrap().parent(), found[0]);
            assert!(app.world().get::<SeveredBlock>(b).is_some());
        }
        for &b in &blocks[..4] {
            assert_eq!(app.world().get::<ChildOf>(b).unwrap().parent(), ship, "the bigger part stays the ship");
        }
    }

    #[test]
    fn pieces_part_by_their_mass_and_the_blast() {
        let (chunk, big) = parting_speeds(10, 1000, 0.0);
        assert!(chunk > big * 10.0, "a chunk flies, the main body barely recoils: {chunk} {big}");
        assert!((chunk * 10.0 - big * 1000.0).abs() < 1.0, "what one gains the other pays");
        let (half_a, half_b) = parting_speeds(1000, 1000, 0.0);
        assert!((half_a - half_b).abs() < 0.01 && half_a > 30.0 && half_a < 100.0, "two halves drift apart alike: {half_a}");
        let (blasted, _) = parting_speeds(1000, 1000, 20.0);
        assert!(blasted > half_a * 1.5, "a bigger blast parts them harder: {blasted}");
        assert_eq!(parting_speeds(1, 1000, 100.0).0, MAX_PARTING_SPEED);
        // A chunk spins; half a ship barely turns.
        assert!(parting_spin(10, 100.0, 80.0, 0.0).abs() > parting_spin(1000, 1000.0, 80.0, 0.0).abs() * 10.0);
    }

    #[test]
    fn the_section_goes_one_way_and_the_ship_recoils_the_other() {
        let mut app = app();
        let (ship, blocks) = ship(&mut app, 7, &[]);
        app.world_mut().entity_mut(blocks[4]).insert(HullDestroyed);
        app.update();
        let section = sections(&mut app)[0];
        let section_v = app.world().get::<Velocity>(section).unwrap().0;
        let ship_v = app.world().get::<Velocity>(ship).unwrap().0;
        assert!(section_v.x > 0.0 && ship_v.x < 0.0, "{section_v} {ship_v}");
        assert!(section_v.length() > ship_v.length(), "the smaller piece goes faster");
    }

    #[test]
    fn an_ai_ship_breaks_up_too_and_its_blocks_stop_being_its() {
        use crate::ai_ship::components::{AiShip, AiShipState, OwnedByAiShip};
        let mut app = app();
        let ship = app.world_mut().spawn((AiShip, AiShipState::default(), Transform::default(), Velocity(Vec2::ZERO), crate::ship::collision::KnockVelocity::default())).id();
        let blocks: Vec<Entity> = (0..7).map(|x| app.world_mut().spawn((hull_at(x, 0), OwnedByAiShip { root: ship }, ChildOf(ship))).id()).collect();
        app.update();
        app.world_mut().entity_mut(blocks[4]).insert(HullDestroyed);
        // An AI ship waits its turn between checks.
        app.world_mut().resource_mut::<Time>().advance_by(std::time::Duration::from_secs(1));
        app.update();
        let found = sections(&mut app);
        assert_eq!(found.len(), 1);
        assert_eq!(app.world().get::<ChildOf>(blocks[6]).unwrap().parent(), found[0]);
        assert_eq!(app.world().get::<OwnedByAiShip>(blocks[6]).unwrap().root, found[0], "the wreckage owns it now");
        assert_eq!(app.world().get::<ChildOf>(blocks[0]).unwrap().parent(), ship);
        assert_eq!(app.world().get::<OwnedByAiShip>(blocks[0]).unwrap().root, ship);
        assert!(app.world().get::<crate::ship::collision::KnockVelocity>(ship).unwrap().0.x < 0.0, "it recoils");
    }

    #[test]
    fn a_big_half_is_no_bomb() {
        // Half a ship with a reactor aboard drifts; a chunk with one melts down.
        let mut app = app();
        let (_, blocks) = ship(&mut app, 201, &[]);
        let root = app.world().get::<ChildOf>(blocks[0]).unwrap().parent();
        app.world_mut().spawn((Module {
            module_type: ModuleType::StandardReactor, health: 1.0, max_health: 1.0, power_consumption: 0.0, power_generation: 1.0,
            is_active: true, grid_position: IVec2::new(150, 1), size: IVec2::ONE, rotation: Rotation::North,
        }, Transform::from_xyz(150.0 * 66.0, 33.0, 0.0), ChildOf(root)));
        app.update();
        app.world_mut().entity_mut(blocks[100]).insert(HullDestroyed);
        app.update();
        let found = sections(&mut app);
        assert_eq!(found.len(), 1);
        let section = app.world().get::<DetachedSection>(found[0]).unwrap();
        assert!(section.meltdown_timer.is_none() && !section.has_reactor);
    }

    #[test]
    fn everything_standing_in_a_cell_goes_with_it() {
        // Floor and plate share a cell. Grouping by cell used to keep only one
        // of them, and the other stayed behind hanging in the air.
        let mut app = app();
        let (ship, blocks) = ship(&mut app, 7, &[(6, 0), (5, 0), (1, 0)]);
        app.world_mut().entity_mut(blocks[4]).insert(HullDestroyed);
        app.update();
        let section = sections(&mut app)[0];
        for block in [blocks[5], blocks[6], blocks[7], blocks[8]] {
            assert_eq!(app.world().get::<ChildOf>(block).unwrap().parent(), section);
        }
        for block in [blocks[0], blocks[1], blocks[9]] {
            assert_eq!(app.world().get::<ChildOf>(block).unwrap().parent(), ship);
        }
    }

    #[test]
    fn blocks_joined_only_at_a_corner_hold_together() {
        let mut app = app();
        // A diagonal step off the end of the row: (5,1) touches (4,0) at a corner.
        let (_, blocks) = ship(&mut app, 5, &[(5, 1), (6, 2)]);
        app.world_mut().entity_mut(blocks[0]).insert(HullDestroyed);
        app.update();
        assert!(sections(&mut app).is_empty());
    }

    #[test]
    fn a_piece_built_apart_on_purpose_is_never_flung_off() {
        let mut app = app();
        // (0,5) never touched the hull.
        let (ship, blocks) = ship(&mut app, 5, &[(0, 5)]);
        app.world_mut().entity_mut(blocks[0]).insert(HullDestroyed);
        app.update();
        assert!(sections(&mut app).is_empty());
        assert_eq!(app.world().get::<ChildOf>(blocks[5]).unwrap().parent(), ship);
    }

    #[test]
    fn removing_a_block_without_destroying_it_severs_nothing() {
        let mut app = app();
        let (_, blocks) = ship(&mut app, 7, &[]);
        app.world_mut().entity_mut(blocks[4]).despawn(); // a build-mode delete
        app.update();
        assert!(sections(&mut app).is_empty());
    }
}
