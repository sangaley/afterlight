use bevy::prelude::*;
use rand::Rng;
use std::collections::HashSet;

use crate::components::*;
use crate::events::*;
use crate::resources::*;
use crate::ai_ship::components::AiShipWreck;

// ============================================================================
// EVA SALVAGE — Cosmoteer-style crew looting.
// F near a wreck dispatches a detail of idle crew. Each crew member
// CLAIMS A SPECIFIC BLOCK on the hulk (farthest from the core first, so
// the wreck peels outside-in), flies to it around the hull rather than
// through it, spends a moment prying it loose — the block is physically
// removed, chunks fly — then ferries the haul home. Trip after trip
// until the wreck's loot is stripped or they're recalled (F again).
// Loot only banks when a crew member makes it back aboard; a death
// mid-ferry loses the carried haul, and wreck explosions (death
// rattles, hot-wreck cook-offs) hurt crew working the hulk.
// ============================================================================

/// How close the ship must be to a wreck's NEAREST BLOCK to dispatch a
/// detail — root-origin distance made big hulks (whose origin sits far
/// from their rim) demand parking inside the wreck.
// Ship root to nearest wreck block. Hulls are solid now (see ship::collision),
// so a big ship parked against a wreck can still have its root ~500+ units
// from the contact point — 500 made dispatch unreachable nose-on.
const ORDER_RANGE: f32 = 3000.0;
/// How many gun posts the default skeleton crew holds back when a detail goes
/// out. Enough to answer something that wanders in, few enough that the wreck
/// still gets stripped at a sensible rate.
const SKELETON_GUNS: usize = 2;
/// Ship drifting further than this from the worksite recalls the detail.
/// Scales WITH ORDER_RANGE — if recall is tighter than dispatch, a detail sent
/// out at the limit gets called straight back and salvage reads as broken.
///
/// It did exactly that. ORDER_RANGE went to 3000, measured to the nearest
/// block, while this stayed at 1800 measured to the wreck's centre -- so any
/// order given from between those two was dispatched and recalled in the same
/// frame ("Salvage detail EVA: 4 crew" and "breaking off - out of range" on
/// one tick). It is what stalled the salvage step of flight training. Both are
/// now measured to the wreck's rim (`worksite_distance`), and this sits a
/// margin past the order range so a little drift after the order is free.
const BREAK_RANGE: f32 = ORDER_RANGE + 1500.0;
/// Crew stranded further than this from the ship emergency-board instantly.
/// Past the break range plus the span of a big hulk, so nobody working the
/// far side of a wreck within range gets yanked home mid-trip.
const TELEPORT_RANGE: f32 = BREAK_RANGE + 2000.0;
const EVA_SPEED: f32 = 130.0;
const GRAB_SECONDS: f32 = 1.2;
const ARRIVE_WRECK: f32 = 16.0;
const ARRIVE_SHIP: f32 = 60.0;
/// Final-approach standoff: crew aim just outside their block, then hop in.
const APPROACH_DIST: f32 = 45.0;
/// Blast radius/scale for wreck explosions hitting EVA crew. Suits soak
/// half the blast — a final-boom at point blank hurts badly but is
/// survivable for a healthy crew member.
const BLAST_RADIUS: f32 = 90.0;
const BLAST_SCALE: f32 = 0.5;

const EVA_TINT: Color = Color::srgb(0.75, 0.85, 1.0);
/// Slightly amber while hauling a block home — reads as "carrying".
const CARRY_TINT: Color = Color::srgb(1.0, 0.85, 0.55);

pub enum EvaPhase {
    Outbound,
    Grabbing(Timer),
    Returning,
}

/// A crew member currently on salvage EVA. While this is present the
/// normal in-ship crew systems (needs, room mapping, AI, reparenting)
/// leave the crew member alone — the suit keeps them alive out there.
#[derive(Component)]
pub struct EvaSalvaging {
    pub wreck: Entity,
    /// The specific block this crew member has claimed for dismantling
    /// (the wreck root as fallback for block-less world wrecks). Other
    /// crew skip claimed blocks, so everyone works their own corner.
    pub grab_target: Entity,
    pub phase: EvaPhase,
    pub carrying: Option<ItemType>,
    pub recalled: bool,
    /// Local transform aboard ship, restored when they board.
    pub home_local: Vec3,
    pub base_color: Color,
}

type BlockQuery<'w, 's> = Query<
    'w,
    's,
    (&'static GlobalTransform, &'static Sprite),
    (Or<(With<Module>, With<HullSegment>)>, Without<CrewMember>),
>;

/// Within this of a wreck (to its nearest block, from the ship's root), F
/// salvages it even inside a station's docking range: about a hull-length,
/// alongside it.
pub const ALONGSIDE: f32 = 1_500.0;

/// Inside a station's docking range, whether F docks rather than sending a
/// detail to a wreck `wreck_dist` off, with the station's structure `edge`
/// away.
///
/// F docks, unless the ship is alongside the wreck and nearer to it than to
/// the station. Plain "nearer wins" was fine with 1.8 km of docking range;
/// at 7 km it gave F to any wreck within 3 km, and round Haven, whose
/// approach is full of chunk-layer wrecks, F almost never docked. Nearer
/// still matters alongside, or parking on a wreck beside a station would
/// berth the ship with the detail on the hull.
pub fn station_takes_the_press(edge: f32, wreck_dist: f32) -> bool {
    wreck_dist > ALONGSIDE || edge < wreck_dist
}

/// F key: dispatch a salvage detail at the nearest lootable wreck in
/// range, or recall the detail that's already out.
pub fn order_salvage_detail(
    mut press: ResMut<crate::resources::InteractPress>,
    stations: Res<crate::world::home_base::SystemStations>,
    mut commands: Commands,
    ship_query: Query<(Entity, &GlobalTransform), With<Ship>>,
    wreck_query: Query<(Entity, &GlobalTransform, &Wreck, &PointOfInterest)>,
    mut active_eva: Query<&mut EvaSalvaging>,
    mut crew_query: Query<
        (Entity, &mut CrewMember, &mut Transform, &GlobalTransform, &mut Sprite),
        Without<EvaSalvaging>,
    >,
    mut station_query: Query<(Entity, &mut CrewStation)>,
    station_info: Query<(&Module, Has<KeepManned>), With<CrewStation>>,
    reactor_marker: Query<(), With<Reactor>>,
    children_query: Query<&Children>,
    block_query: BlockQuery,
    airlocks: Query<(&Module, &AirlockComp, &ChildOf), Without<DestroyedModule>>,
    already_suited: Query<
        (),
        (With<Suited>, Without<crate::ai_ship::components::OwnedByAiShip>),
    >,
    mut notifications: MessageWriter<ShowNotification>,
    faction_stations: Query<
        (&GlobalTransform, &crate::diplomacy::stations::StationAnchor),
        With<crate::diplomacy::stations::StationCore>,
    >,
) {
    if !press.pending() {
        return;
    }

    // A detail is already out - F recalls it, ahead of every other use of the
    // key. Crew on the hull outrank anything else the press could mean.
    if !active_eva.is_empty() {
        if !press.claim() {
            return;
        }
        for mut eva in active_eva.iter_mut() {
            eva.recalled = true;
        }
        notifications.write(ShowNotification {
            message: "Salvage detail recalled.".into(),
            notification_type: NotificationType::Info,
            duration: 2.0,
        });
        return;
    }

    let Ok((ship_entity, ship_gt)) = ship_query.single() else { return };
    let ship_pos = ship_gt.translation().truncate();

    // Nearest wreck in dispatch range — distance measured to whichever
    // of its blocks is closest, not to the root origin.
    let mut best: Option<(Entity, Vec2, f32)> = None;
    let mut nearest_any = f32::INFINITY;
    for (entity, gt, wreck, poi) in wreck_query.iter() {
        if poi.poi_type != PoiType::Wreck {
            continue;
        }
        // Worth a detail while it has cargo OR plating to break into scrap --
        // the same test run_salvage_detail uses to send crew back for another
        // trip. Requiring cargo here meant a gutted hulk (loot spent, hull
        // still there) couldn't be ordered at all, and F said nothing.
        let has_blocks = children_query
            .get(entity)
            .is_ok_and(|children| children.iter().any(|c| block_query.get(c).is_ok()));
        if wreck.loot_remaining == 0 && !has_blocks {
            continue;
        }
        let root_pos = gt.translation().truncate();
        let mut dist = ship_pos.distance(root_pos);
        if let Ok(children) = children_query.get(entity) {
            for child in children.iter() {
                if let Ok((block_gt, _)) = block_query.get(child) {
                    dist = dist.min(ship_pos.distance(block_gt.translation().truncate()));
                }
            }
        }
        nearest_any = nearest_any.min(dist);
        if dist < ORDER_RANGE && best.map_or(true, |(_, _, d)| dist < d) {
            best = Some((entity, root_pos, dist));
        }
    }
    let Some((wreck_entity, wreck_center, wreck_dist)) = best else {
        // A wreck just past reach used to swallow F without a word: no
        // detail, no reason. Say how far, and leave the press for a dock or
        // settlement that may want it -- unless a station is in range, whose
        // own prompt is already the one on screen.
        if nearest_any < ORDER_RANGE * 2.0 && stations.nearest_in_range(ship_pos).is_none() {
            notifications.write(ShowNotification {
                message: format!(
                    "Wreck out of reach ({}) - close within {} to send a detail.",
                    crate::ui::format_range_km(nearest_any),
                    crate::ui::format_range_km(ORDER_RANGE),
                ),
                notification_type: NotificationType::Info,
                duration: 2.5,
            });
        }
        return;
    };

    // Yield to docking when a station can take the press (see
    // `station_takes_the_press`). Only to a station actually in range,
    // though, or F would do nothing at all between the two.
    if let Some(site) = stations.nearest_in_range(ship_pos) {
        // To the station's edge, not its centre: stations are kilometres
        // across, and their centre is "farther" than a wreck the ship is
        // nowhere near.
        let edge = (ship_pos.distance(site.pos) - site.radius()).max(0.0);
        if station_takes_the_press(edge, wreck_dist) {
            return;
        }
    }
    // The same courtesy to a faction station you're beside.
    for (gt, anchor) in faction_stations.iter() {
        let edge = (ship_pos.distance(gt.translation().truncate()) - anchor.extent).max(0.0);
        if edge < crate::diplomacy::stations::DOCK_MARGIN && station_takes_the_press(edge, wreck_dist) {
            return;
        }
    }
    if !press.claim() {
        return;
    }

    // Which posts keep their operator? Player-pinned stations
    // (right-click a crew station to toggle), or the default skeleton
    // crew when nothing is pinned: helm + one reactor — a gunner is no
    // use if the unmanned reactor can't power the gun.
    let mut pinned_posts: Vec<Entity> = Vec::new();
    let mut default_helm: Option<Entity> = None;
    let mut default_reactor: Option<Entity> = None;
    let mut default_weapons: Vec<Entity> = Vec::new();
    if let Ok(children) = children_query.get(ship_entity) {
        for child in children.iter() {
            if let Ok((module, is_pinned)) = station_info.get(child) {
                if is_pinned {
                    pinned_posts.push(child);
                }
                if default_helm.is_none() && module.module_type == ModuleType::HelmStation {
                    default_helm = Some(child);
                }
                if default_reactor.is_none() && reactor_marker.get(child).is_ok() {
                    default_reactor = Some(child);
                }
                if module.module_type.category() == ModuleCategory::Weapons {
                    default_weapons.push(child);
                }
            }
        }
    }
    if pinned_posts.is_empty() {
        pinned_posts.extend(default_helm.into_iter().chain(default_reactor));
        // ...and leave the battery crewed. A gun IS its own crew station, so
        // a helm-and-reactor skeleton emptied every one of them: a fully
        // crewed warship would sit at 1/20 posts with all guns silent
        // (combat::weapon_is_crewed) the whole time a detail was out, which
        // made stripping a wreck the most dangerous thing you could do.
        // Two posts is enough to answer a scavenger without gutting the
        // detail; pin more by hand to keep them.
        pinned_posts.extend(default_weapons.into_iter().take(SKELETON_GUNS));
    }

    // Manually-assigned crew hold their posts. Auto-assigned crew are fair
    // game — the auto-assigner staffs every free hand within seconds, so
    // "unassigned idler" is an empty set on any crewed ship. Pull them the
    // way emergency dispatch does; auto-assign restaffs once they board.
    let mut manual: HashSet<Entity> = HashSet::new();
    let mut auto_assigned: std::collections::HashMap<Entity, Entity> = std::collections::HashMap::new();
    for (station_entity, station) in station_query.iter() {
        if let Some(crew_entity) = station.assigned_crew {
            if station.manually_assigned {
                manual.insert(crew_entity);
            } else {
                auto_assigned.insert(crew_entity, station_entity);
            }
        }
    }

    // Crew reserved to man the pinned posts. Keep whoever's already
    // there; an unstaffed pinned post claims an idle hand on the spot
    // so the reserve is real, not aspirational.
    let mut reserved: HashSet<Entity> = HashSet::new();
    for &post in &pinned_posts {
        if let Ok((_, station)) = station_query.get(post) {
            if let Some(crew_entity) = station.assigned_crew {
                reserved.insert(crew_entity);
            }
        }
    }
    let mut idle_pool: Vec<Entity> = crew_query
        .iter()
        .filter(|(entity, crew, ..)| {
            crew.health > 0.0
                && crew.state == CrewState::Idle
                && !manual.contains(entity)
                && !reserved.contains(entity)
        })
        .map(|(entity, ..)| entity)
        .collect();
    for &post in &pinned_posts {
        let Ok((_, mut station)) = station_query.get_mut(post) else { continue };
        if station.assigned_crew.is_none() {
            if let Some(crew_entity) = idle_pool.pop() {
                station.assigned_crew = Some(crew_entity);
                reserved.insert(crew_entity);
            }
        }
    }

    // Blocks sorted farthest-from-core first: crew_0 gets the outermost,
    // crew_1 the next, and so on — everyone flies somewhere different.
    let mut blocks: Vec<(Entity, f32)> = children_query
        .get(wreck_entity)
        .map(|children| {
            children
                .iter()
                .filter_map(|c| {
                    block_query
                        .get(c)
                        .ok()
                        .map(|(gt, _)| (c, gt.translation().truncate().distance_squared(wreck_center)))
                })
                .collect()
        })
        .unwrap_or_default();
    blocks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Nobody goes outside without a suit on.
    //
    // The fiction always said they had one -- eva_salvage's own comment says
    // "the suit keeps them alive out there" and the tutorial says crew suit up
    // before crossing -- but nothing backed it. Now the same rack that keeps
    // people alive through a breach is what a salvage detail draws from, so one
    // rule covers both instead of two contradictory ones.
    let racked: u32 = airlocks
        .iter()
        .filter(|(module, _, parent)| {
            parent.parent() == ship_entity && module.is_active && module.health > 0.0
        })
        .map(|(_, rack, _)| rack.suits)
        .sum();
    let mut free_suits = racked.saturating_sub(already_suited.iter().count() as u32);
    if free_suits == 0 {
        // The press was claimed above. Claiming it a second time here always
        // failed and returned first, so "no suits" was never said -- F just
        // did nothing.
        notifications.write(ShowNotification {
            message: if racked == 0 {
                "No airlock aboard - nowhere to suit up for EVA.".into()
            } else {
                format!("All {racked} suits are already out. Nobody can go EVA.")
            },
            notification_type: NotificationType::Warning,
            duration: 3.0,
        });
        return;
    }

    // Everyone not holding a post ships out.
    let mut dispatched = 0usize;
    let (mut panicking, mut busy, mut held) = (0u32, 0u32, 0u32);
    for (entity, mut crew, mut transform, gt, mut sprite) in crew_query.iter_mut() {
        if crew.health <= 0.0 {
            continue;
        }
        if free_suits == 0 {
            break; // rack is empty; the rest stay aboard
        }
        // Tally WHY nobody is available — a bare "no idle crew" hides
        // whether the problem is panic, emergencies, or manned posts.
        if crew.state == CrewState::Panicking || crew.state == CrewState::Unconscious {
            panicking += 1;
            continue;
        }
        if manual.contains(&entity) || reserved.contains(&entity) {
            held += 1;
            continue;
        }
        if crew.state != CrewState::Idle {
            busy += 1;
            continue;
        }
        if let Some(&station_entity) = auto_assigned.get(&entity) {
            if let Ok((_, mut station)) = station_query.get_mut(station_entity) {
                station.assigned_crew = None;
            }
        }

        let home_local = transform.translation;
        let world = gt.translation();
        transform.translation = Vec3::new(world.x, world.y, world.z + 1.0);
        transform.rotation = Quat::IDENTITY;
        crew.state = CrewState::Salvaging;
        crew.oxygen = 100.0;
        commands.entity(entity).try_insert(Suited);
        free_suits -= 1;
        let base_color = sprite.color;
        sprite.color = EVA_TINT;

        let grab_target = blocks
            .get(dispatched)
            .map(|(e, _)| *e)
            .unwrap_or(wreck_entity);

        commands
            .entity(entity)
            .try_remove::<ChildOf>()
            .try_remove::<CrewRoomLocation>()
            .try_insert(EvaSalvaging {
                wreck: wreck_entity,
                grab_target,
                phase: EvaPhase::Outbound,
                carrying: None,
                recalled: false,
                home_local,
                base_color,
            });
        dispatched += 1;
    }

    if dispatched > 0 {
        // F docks too, and goes to whichever is nearer. Beside a station with
        // a fresh kill next to it, a player pressing F to dock got a detail
        // out, then a recall, then another detail -- and never docked, with
        // "press F to dock" still on screen. Say why it wasn't a dock.
        let (message, duration) = match stations.nearest_in_range(ship_pos) {
            Some(site) => (
                format!(
                    "Salvage detail EVA: {} crew - you're alongside a wreck. F recalls them; pull off it to dock at {}.",
                    dispatched, site.name
                ),
                5.0,
            ),
            None => (format!("Salvage detail EVA: {} crew (F to recall)", dispatched), 2.5),
        };
        notifications.write(ShowNotification {
            message,
            notification_type: NotificationType::Info,
            duration,
        });
    } else {
        notifications.write(ShowNotification {
            message: format!(
                "No crew free for salvage ({} panicking, {} busy, {} at manned posts).",
                panicking, busy, held
            ),
            notification_type: NotificationType::Warning,
            duration: 3.0,
        });
    }
}

/// Flies each EVA crew member through their trip loop: swing around the
/// hulk to their claimed block → pry it loose (block pops off as debris)
/// → haul it home around the hull → deposit → claim the next block.
pub fn run_salvage_detail(
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    time: Res<Time>,
    mut commands: Commands,
    ship_query: Query<(Entity, &GlobalTransform), With<Ship>>,
    mut eva_query: Query<(Entity, &mut Transform, &mut CrewMember, &mut EvaSalvaging, &mut Sprite)>,
    mut wreck_query: Query<(&GlobalTransform, &mut Wreck, &mut PointOfInterest, Option<&AiShipWreck>)>,
    children_query: Query<&Children>,
    block_query: BlockQuery,
    mut inventory: ResMut<Inventory>,
    mut statistics: ResMut<Statistics>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let Ok((ship_entity, ship_gt)) = ship_query.single() else { return };
    let ship_pos = ship_gt.translation().truncate();
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    // Blocks already claimed by someone this frame — nobody double-books.
    let reserved: HashSet<Entity> = eva_query.iter().map(|(.., eva, _)| eva.grab_target).collect();

    let generic_loot = [
        ItemType::ScrapMetal,
        ItemType::Crystal,
        ItemType::FuelCell,
        ItemType::RareAlloy,
        ItemType::AmmoCrate,
    ];

    // One break-off is one event, not one per spacewalker. The recall check
    // below runs per crew member, so a four-hand detail used to fire four
    // identical warnings for a single drift-out -- the noisiest message in the
    // game by a wide margin.
    let mut announced_break_off = false;

    for (entity, mut transform, mut crew, eva, mut sprite) in eva_query.iter_mut() {
        let eva = eva.into_inner();
        if crew.health <= 0.0 {
            continue; // death event handling despawns them
        }
        let pos = transform.translation.truncate();

        // Wreck geometry for avoidance steering (None once it's gone)
        let bounds = wreck_query
            .get(eva.wreck)
            .ok()
            .map(|(gt, ..)| wreck_bounds(gt, eva.wreck, &children_query, &block_query));

        // Ship drifted too far from the worksite — break off.
        if !eva.recalled {
            if let Some((center, radius)) = bounds {
                if worksite_distance(ship_pos, center, radius) > BREAK_RANGE {
                    eva.recalled = true;
                    if !announced_break_off {
                        announced_break_off = true;
                        notifications.write(ShowNotification {
                            message: "Salvage detail breaking off - out of range.".into(),
                            notification_type: NotificationType::Warning,
                            duration: 2.5,
                        });
                    }
                }
            }
        }

        match &mut eva.phase {
            EvaPhase::Outbound => {
                let Some((center, radius)) = bounds else {
                    eva.phase = EvaPhase::Returning;
                    continue;
                };
                if eva.recalled {
                    eva.phase = EvaPhase::Returning;
                    continue;
                }
                // Claimed block gone (burned, popped, dismantled by a
                // faster colleague)? Claim a fresh one.
                let block_pos = match block_query.get(eva.grab_target) {
                    Ok((gt, _)) => Some(gt.translation().truncate()),
                    Err(_) if eva.grab_target == eva.wreck => Some(center),
                    Err(_) => None,
                };
                let Some(block_pos) = block_pos else {
                    match pick_block(eva.wreck, center, &children_query, &block_query, &reserved) {
                        Some(next) => eva.grab_target = next,
                        None => eva.grab_target = eva.wreck,
                    }
                    continue;
                };

                // Aim for a standoff point on the block's outward side and
                // swing around the hull to get there; the last short hop
                // goes straight in. The avoidance radius is capped just
                // inside the standoff so the orbit always converges onto
                // it, even for blocks buried deeper than the hulk's rim.
                let outward = (block_pos - center).normalize_or_zero();
                let standoff = block_pos + outward * APPROACH_DIST;
                let avoid_radius = radius.min(standoff.distance(center) - 15.0).max(30.0);
                let final_approach = pos.distance(block_pos) < APPROACH_DIST + 40.0;
                if final_approach {
                    fly_towards(&mut transform, block_pos, None, dt);
                } else {
                    fly_towards(&mut transform, standoff, Some((center, avoid_radius)), dt);
                }
                if transform.translation.truncate().distance(block_pos) < ARRIVE_WRECK {
                    eva.phase = EvaPhase::Grabbing(Timer::from_seconds(GRAB_SECONDS, TimerMode::Once));
                }
            }
            EvaPhase::Grabbing(timer) => {
                timer.tick(time.delta());
                if !timer.is_finished() {
                    continue;
                }
                if let Ok((_, mut wreck, mut poi, ai_wreck)) = wreck_query.get_mut(eva.wreck) {
                    let dismantling_block = eva.grab_target != eva.wreck;

                    if wreck.loot_remaining > 0 {
                        wreck.loot_remaining -= 1;
                        eva.carrying = Some(match ai_wreck {
                            Some(aw) => crate::ai_ship::wreck::roll_wreck_loot(
                                aw.ship_type,
                                aw.intact_frac,
                                &mut rng,
                            ),
                            None => generic_loot[rng.gen_range(0..generic_loot.len())],
                        });

                        if wreck.loot_remaining == 0 {
                            poi.discovered = true;
                            wreck.is_explored = true;
                            statistics.wrecks_salvaged += 1;
                            notifications.write(ShowNotification {
                                message: "Cargo stripped - hauling hull scrap now (F to recall).".into(),
                                notification_type: NotificationType::Info,
                                duration: 3.0,
                            });
                        }
                    } else if dismantling_block {
                        // Cargo's gone — the hull metal itself is the haul.
                        // Crew keep breaking the carcass down into scrap
                        // until recalled or nothing's left.
                        eva.carrying = Some(ItemType::ScrapMetal);
                    }
                    if eva.carrying.is_some() {
                        sprite.color = CARRY_TINT;
                    }

                    // DISMANTLE: the claimed block physically comes off
                    // the hulk — flash + chunks in its own color.
                    if dismantling_block {
                        if let Ok((block_gt, block_sprite)) = block_query.get(eva.grab_target) {
                            let block_pos = block_gt.translation().truncate();
                            crate::combat::spawn_hit_effect(
                                &mut commands,
                                block_pos,
                                Color::srgb(0.7, 0.8, 1.0),
                                35.0,
                            );
                            crate::vfx::debris::spawn_chunks(
                                &mut commands,
                                &fx,
                                &mut rng,
                                block_pos,
                                block_sprite.color,
                                Vec2::ZERO,
                            );
                        }
                        commands.entity(eva.grab_target).try_despawn();

                        // Last block gone? The wreck ceases to exist.
                        let blocks_left = children_query
                            .get(eva.wreck)
                            .map(|children| {
                                children
                                    .iter()
                                    .filter(|c| *c != eva.grab_target && block_query.get(*c).is_ok())
                                    .count()
                            })
                            .unwrap_or(0);
                        if blocks_left == 0 {
                            commands.entity(eva.wreck).try_despawn();
                            notifications.write(ShowNotification {
                                message: "Wreck fully dismantled.".into(),
                                notification_type: NotificationType::Success,
                                duration: 3.0,
                            });
                        }
                    }
                }
                eva.phase = EvaPhase::Returning;
            }
            EvaPhase::Returning => {
                let dist = fly_towards(&mut transform, ship_pos, bounds, dt);
                if dist > TELEPORT_RANGE {
                    // Stranded — emergency board, keep the haul (they made it).
                    deposit(eva, &mut inventory, &mut notifications);
                    board_crew(&mut commands, ship_entity, entity, eva, &mut transform, &mut crew, &mut sprite);
                    continue;
                }
                if dist >= ARRIVE_SHIP {
                    continue;
                }

                deposit(eva, &mut inventory, &mut notifications);
                sprite.color = EVA_TINT;

                // Another trip, or board? Worth going back while the wreck
                // still has cargo OR blocks to break down into scrap.
                let center = bounds.map(|(c, _)| c).unwrap_or(ship_pos);
                let radius = bounds.map_or(0.0, |(_, r)| r);
                let next_block =
                    pick_block(eva.wreck, center, &children_query, &block_query, &reserved);
                let more_work = !eva.recalled
                    && wreck_query
                        .get_mut(eva.wreck)
                        .map(|(_, wreck, ..)| {
                            (wreck.loot_remaining > 0 || next_block.is_some())
                                && worksite_distance(ship_pos, center, radius) < BREAK_RANGE
                        })
                        .unwrap_or(false);

                if more_work {
                    // Root fallback only matters for block-less world
                    // wrecks, which still carry abstract cargo.
                    eva.grab_target = next_block.unwrap_or(eva.wreck);
                    eva.phase = EvaPhase::Outbound;
                } else {
                    board_crew(&mut commands, ship_entity, entity, eva, &mut transform, &mut crew, &mut sprite);
                }
            }
        }
    }
}

/// Wreck explosions (death rattles, hot-wreck cook-offs) hit crew working
/// the hulk. This is the risk that makes "loot it while it burns" a real
/// decision — the fire isn't just eating cargo, it's shooting at your people.
pub fn eva_blast_damage(
    mut explosions: MessageReader<AiModuleExploded>,
    mut eva_query: Query<(Entity, &Transform, &mut CrewMember), With<EvaSalvaging>>,
    mut damage_events: MessageWriter<CrewDamaged>,
) {
    for explosion in explosions.read() {
        for (entity, transform, mut crew) in eva_query.iter_mut() {
            if crew.health <= 0.0 {
                continue;
            }
            let dist = transform.translation.truncate().distance(explosion.position);
            if dist > BLAST_RADIUS {
                continue;
            }
            let damage = explosion.blast_damage * (1.0 - dist / BLAST_RADIUS) * BLAST_SCALE;
            if damage <= 0.0 {
                continue;
            }
            crew.health -= damage;
            // Reporting the wound is the whole job here. Whether it killed
            // them is `crew::report_crew_deaths`'s call, not this system's.
            damage_events.write(CrewDamaged {
                crew: entity,
                amount: damage,
                source: CrewDamageSource::Explosion,
            });
        }
    }
}

/// Wires `abort_eva_on_exit` to the end of a flight -- which a pause is not.
///
/// ESC pauses by leaving Exploring, so a plain OnExit(Exploring) teleported
/// the whole detail aboard mid-job every time the player opened the menu.
/// `State<GameState>` already holds the state being entered while OnExit
/// runs, so each condition reads where we are going. A pause that ends
/// anywhere but back in flight (quit to menu, load) recalls them on the way
/// out of Paused instead.
pub fn schedule_recall(app: &mut App) {
    use crate::states::GameState;
    app.add_systems(
        OnExit(GameState::Exploring),
        abort_eva_on_exit.run_if(not(in_state(GameState::Paused))),
    )
    .add_systems(
        OnExit(GameState::Paused),
        abort_eva_on_exit.run_if(not(in_state(GameState::Exploring))),
    );
}

/// Leaving Exploring (docking, game over, menu) instantly boards every
/// EVA crew member so nobody is left floating in a state we don't simulate.
pub fn abort_eva_on_exit(
    mut commands: Commands,
    ship_query: Query<Entity, With<Ship>>,
    mut eva_query: Query<(Entity, &mut Transform, &mut CrewMember, &mut EvaSalvaging, &mut Sprite)>,
    mut inventory: ResMut<Inventory>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    let Ok(ship) = ship_query.single() else { return };
    for (entity, mut transform, mut crew, mut eva, mut sprite) in eva_query.iter_mut() {
        deposit(&mut eva, &mut inventory, &mut notifications);
        board_crew(&mut commands, ship, entity, &eva, &mut transform, &mut crew, &mut sprite);
    }
}

/// Next block worth prying off: farthest surviving block from the core
/// that nobody else has claimed — the hulk peels outside-in.
fn pick_block(
    wreck: Entity,
    center: Vec2,
    children_query: &Query<&Children>,
    block_query: &BlockQuery,
    reserved: &HashSet<Entity>,
) -> Option<Entity> {
    let children = children_query.get(wreck).ok()?;
    children
        .iter()
        .filter(|c| !reserved.contains(c))
        .filter_map(|c| {
            block_query
                .get(c)
                .ok()
                .map(|(gt, _)| (c, gt.translation().truncate().distance_squared(center)))
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(entity, _)| entity)
}

/// Wreck center + bounding radius (with clearance margin) for steering.
fn wreck_bounds(
    wreck_gt: &GlobalTransform,
    wreck: Entity,
    children_query: &Query<&Children>,
    block_query: &BlockQuery,
) -> (Vec2, f32) {
    let center = wreck_gt.translation().truncate();
    let mut radius: f32 = 60.0;
    if let Ok(children) = children_query.get(wreck) {
        for child in children.iter() {
            if let Ok((gt, _)) = block_query.get(child) {
                radius = radius.max(gt.translation().truncate().distance(center) + 50.0);
            }
        }
    }
    (center, radius)
}

/// How far the ship is from the worksite: its root to the wreck's rim, never
/// below zero. The rim sits at or inside the nearest block the dispatch check
/// measures to, so anything close enough to order a detail out is at least as
/// close by this measure, and can't be recalled for the same distance.
fn worksite_distance(ship_pos: Vec2, wreck_center: Vec2, wreck_radius: f32) -> f32 {
    (ship_pos.distance(wreck_center) - wreck_radius).max(0.0)
}

/// Moves toward `target` at EVA speed, returns remaining distance.
/// Basic pathfinding: if the straight line ahead would cut through the
/// hulk's bounding circle, slide along the tangent instead — crew visibly
/// swing AROUND the wreck instead of clipping through its blocks.
fn fly_towards(
    transform: &mut Transform,
    target: Vec2,
    obstacle: Option<(Vec2, f32)>,
    dt: f32,
) -> f32 {
    let pos = transform.translation.truncate();
    let delta = target - pos;
    let dist = delta.length();
    if dist <= 1.0 {
        return dist;
    }
    let mut dir = delta / dist;

    if let Some((center, radius)) = obstacle {
        let from_center = pos - center;
        // Only dodge while we're outside the hull ourselves and the leg
        // ahead actually clips the circle; the final approach (handled by
        // the caller with obstacle = None) is allowed to go in.
        if from_center.length() > radius * 0.6 {
            let along = (center - pos).dot(dir);
            if along > 0.0 && along < dist {
                let closest = pos + dir * along;
                if closest.distance(center) < radius {
                    let side = if from_center.perp_dot(delta) >= 0.0 { 1.0 } else { -1.0 };
                    dir = (from_center.perp() * side).normalize_or_zero();
                }
            }
        }
    }

    let step = (EVA_SPEED * dt).min(dist);
    transform.translation.x += dir.x * step;
    transform.translation.y += dir.y * step;
    dist
}

/// Banks whatever this crew member is carrying into the cargo hold.
fn deposit(
    eva: &mut EvaSalvaging,
    inventory: &mut Inventory,
    notifications: &mut MessageWriter<ShowNotification>,
) {
    let Some(item) = eva.carrying.take() else { return };
    if inventory.add_item(item, 1) {
        notifications.write(ShowNotification {
            message: format!("Crew salvaged {}", item.name()),
            notification_type: NotificationType::Success,
            duration: 2.0,
        });
    } else {
        eva.recalled = true;
        notifications.write(ShowNotification {
            message: format!("Cargo hold full - {} jettisoned!", item.name()),
            notification_type: NotificationType::Warning,
            duration: 3.0,
        });
    }
}

fn board_crew(
    commands: &mut Commands,
    ship: Entity,
    entity: Entity,
    eva: &EvaSalvaging,
    transform: &mut Transform,
    crew: &mut CrewMember,
    sprite: &mut Sprite,
) {
    transform.translation = eva.home_local;
    transform.rotation = Quat::IDENTITY;
    sprite.color = eva.base_color;
    if crew.health > 0.0 {
        crew.state = CrewState::Idle;
    }
    // try_* — the crew member may have died (and been despawned) the
    // same frame they board; see the despawn-race pattern in wreck.rs.
    // Back through the lock, the suit goes back on its rack. Suits never came
    // off: every detail permanently took four out of the airlock, until "all
    // suits are already out" and salvage stopped working for the rest of the
    // run.
    commands
        .entity(entity)
        .try_insert(ChildOf(ship))
        .try_remove::<EvaSalvaging>()
        .try_remove::<Suited>();
}

#[cfg(test)]
mod range_tests {
    use super::*;

    /// The tutorial stall: an order accepted at the dispatch limit must not
    /// trip the break-off on the next tick. Wreck of radius 600 (bounds pad
    /// its farthest block by 50), ship parked right at ORDER_RANGE from its
    /// nearest block.
    #[test]
    fn a_detail_ordered_at_the_limit_is_not_called_straight_back() {
        let center = Vec2::ZERO;
        let radius = 600.0;
        let nearest_block = radius - 50.0;
        let ship = Vec2::new(nearest_block + ORDER_RANGE - 1.0, 0.0);
        assert!(worksite_distance(ship, center, radius) < BREAK_RANGE);
    }

    /// A ship that genuinely leaves still recalls them.
    #[test]
    fn flying_off_breaks_the_detail_off() {
        let ship = Vec2::new(600.0 + BREAK_RANGE + 10.0, 0.0);
        assert!(worksite_distance(ship, Vec2::ZERO, 600.0) > BREAK_RANGE);
    }

    /// Parked inside the wreck's bounding circle reads as zero, not negative.
    #[test]
    fn alongside_the_hulk_is_zero() {
        assert_eq!(worksite_distance(Vec2::new(100.0, 0.0), Vec2::ZERO, 600.0), 0.0);
    }

    /// Crew on the far side of a big hulk, with the ship at the break limit,
    /// are still on a normal trip -- not stranded.
    #[test]
    fn far_side_of_a_big_wreck_is_not_stranded() {
        let big_hulk_span = 2.0 * 900.0;
        assert!(BREAK_RANGE + big_hulk_span < TELEPORT_RANGE);
    }
}

#[cfg(test)]
mod out_of_reach_tests {
    use super::*;

    fn app(wreck_at: f32) -> App {
        let mut app = App::new();
        app.init_resource::<InteractPress>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<crate::world::home_base::SystemStations>();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, (crate::resources::refresh_interact_press, order_salvage_detail).chain());
        app.world_mut().spawn((Ship, GlobalTransform::default()));
        app.world_mut().spawn((
            GlobalTransform::from_xyz(wreck_at, 0.0, 0.0),
            Wreck { loot_remaining: 3, is_explored: false },
            PointOfInterest { poi_type: PoiType::Wreck, discovered: true },
        ));
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyF);
        app
    }

    fn notices(app: &App) -> Vec<String> {
        app.world().resource::<Messages<ShowNotification>>().iter_current_update_messages().map(|n| n.message.clone()).collect()
    }

    /// F near a wreck that's just too far says so, and leaves the press free.
    #[test]
    fn a_wreck_just_past_reach_is_explained() {
        let mut app = app(ORDER_RANGE + 400.0);
        app.update();
        assert_eq!(notices(&app), vec!["Wreck out of reach (3.4 km) - close within 3.0 km to send a detail.".to_string()]);
        assert!(app.world().resource::<InteractPress>().pending());
    }

    /// A hulk with its cargo gone but its plating still there is still work:
    /// the order goes through (here as far as the suit check, with no airlock
    /// aboard), instead of F doing nothing at all.
    #[test]
    fn a_gutted_hulk_with_plating_can_still_be_ordered() {
        let mut app = App::new();
        app.init_resource::<InteractPress>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<crate::world::home_base::SystemStations>();
        app.add_message::<ShowNotification>();
        app.add_systems(Update, (crate::resources::refresh_interact_press, order_salvage_detail).chain());
        app.world_mut().spawn((Ship, GlobalTransform::default()));
        let hulk = app.world_mut().spawn((
            GlobalTransform::from_xyz(800.0, 0.0, 0.0),
            Wreck { loot_remaining: 0, is_explored: true },
            PointOfInterest { poi_type: PoiType::Wreck, discovered: true },
        )).id();
        let plate = app.world_mut().spawn((
            HullSegment::default(),
            Sprite::default(),
            GlobalTransform::from_xyz(820.0, 0.0, 0.0),
        )).id();
        app.world_mut().entity_mut(hulk).add_child(plate);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyF);
        app.update();
        assert_eq!(notices(&app), vec!["No airlock aboard - nowhere to suit up for EVA.".to_string()]);
    }

    /// Nothing anywhere near: nothing to explain.
    /// Inside docking range F docks, unless the ship is alongside a wreck
    /// and nearer it than the station -- and a wreck a few km off, which
    /// used to win just by being nearer than the station's hull, doesn't.
    #[test]
    fn inside_docking_range_f_docks_unless_alongside_a_wreck() {
        // Haven's approach: 5 km off the hull, a chunk wreck 2.5 km away.
        assert!(station_takes_the_press(5_000.0, 2_500.0));
        // Alongside a wreck, well off the station: salvage.
        assert!(!station_takes_the_press(5_000.0, 600.0));
        // Alongside a wreck that sits against the hull, nearer the station: dock.
        assert!(station_takes_the_press(300.0, 900.0));
    }

    #[test]
    fn no_wreck_nearby_says_nothing() {
        let mut app = app(ORDER_RANGE * 3.0);
        app.update();
        assert!(notices(&app).is_empty());
    }
}

#[cfg(test)]
mod pause_recall_tests {
    use super::*;
    use crate::states::GameState;
    use bevy::state::app::StatesPlugin;

    /// One hand out on a wreck, in flight.
    fn app_with_detail_out() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.insert_state(GameState::Exploring);
        app.init_resource::<Inventory>();
        app.add_message::<ShowNotification>();
        schedule_recall(&mut app);
        app.world_mut().spawn(Ship);
        let wreck = app.world_mut().spawn_empty().id();
        let hand = app.world_mut().spawn((
            CrewMember {
                name: "Ash".into(),
                health: 100.0,
                max_health: 100.0,
                oxygen: 100.0,
                morale: 100.0,
                state: CrewState::Salvaging,
            },
            Transform::default(),
            Sprite::default(),
            EvaSalvaging {
                wreck,
                grab_target: wreck,
                phase: EvaPhase::Outbound,
                carrying: None,
                recalled: false,
                home_local: Vec3::ZERO,
                base_color: Color::WHITE,
            },
        )).id();
        app.update();
        (app, hand)
    }

    fn go(app: &mut App, state: GameState) {
        app.world_mut().resource_mut::<NextState<GameState>>().set(state);
        app.update();
    }

    fn out(app: &App, hand: Entity) -> bool {
        app.world().get::<EvaSalvaging>(hand).is_some()
    }

    #[test]
    fn pausing_leaves_the_detail_at_work() {
        let (mut app, hand) = app_with_detail_out();
        go(&mut app, GameState::Paused);
        assert!(out(&app, hand), "ESC recalled the salvage detail");
        go(&mut app, GameState::Exploring);
        assert!(out(&app, hand), "resuming recalled the salvage detail");
    }

    #[test]
    fn docking_recalls_the_detail() {
        let (mut app, hand) = app_with_detail_out();
        go(&mut app, GameState::StationDocked);
        assert!(!out(&app, hand));
    }

    /// Quit to menu from the pause screen never leaves Exploring again, so
    /// the recall has to happen on the way out of Paused.
    #[test]
    fn quitting_from_the_pause_menu_recalls_the_detail() {
        let (mut app, hand) = app_with_detail_out();
        go(&mut app, GameState::Paused);
        go(&mut app, GameState::MainMenu);
        assert!(!out(&app, hand));
    }
}
