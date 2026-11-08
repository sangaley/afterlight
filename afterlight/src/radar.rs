use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::events::*;
use crate::states::{GameState, RadarSet};
use crate::ai_ship::components::{AiShip, AiShipType, AiShipRadarContact, BountyTarget, WorldSimulation};
use crate::contracts::{ContractObjective, ContractState};
use crate::world::home_base::SystemStations;
use crate::celestial::resources::{GalaxyMap, SystemStreamingManager, SystemDiscovery};
use crate::celestial::galaxy::GALAXY_SCAN_RANGE;
use crate::camera::MainCamera;
use std::collections::HashMap;

/// Marker for the radar radar display UI
#[derive(Component)]
pub struct RadarDisplay;

/// Marker for a radar blip (dot on radar)
#[derive(Component)]
pub struct RadarBlip {
    pub lifetime: Timer,
}

/// A persistent radar dot bound to one AI ship entity. Unlike RadarBlip (a
/// transient sweep-spawned dot), this one is created once per ship and moved
/// every frame to track it, so a ship reads as a single sliding dot instead of
/// a smear of stale blips as the player moves.
#[derive(Component)]
pub struct RadarShipDot(pub Entity);

/// Radar dot color by your standing with the ship's faction (see diplomacy):
/// red hostile, orange unfriendly, white neutral, green friendly, blue allied.
fn radar_ship_color(tier: crate::diplomacy::StandingTier) -> Color {
    use crate::diplomacy::StandingTier::*;
    match tier {
        Hostile => Color::srgb(1.0, 0.25, 0.25),
        Unfriendly => Color::srgb(0.95, 0.55, 0.20),
        Neutral => Color::srgb(0.90, 0.90, 0.95),
        Friendly => Color::srgb(0.35, 0.85, 0.50),
        Allied => Color::srgb(0.35, 0.65, 1.0),
    }
}

fn radar_ship_size(faction: AiShipType) -> f32 {
    match faction {
        AiShipType::Shepherd | AiShipType::EternalHegemony => 12.0, // bosses stand out
        _ => 8.0,
    }
}

/// Radar sweep line angle
#[derive(Resource)]
pub struct RadarSweepState {
    pub angle: f32,
    pub sweep_speed: f32,
    pub display_visible: bool,
}

impl Default for RadarSweepState {
    fn default() -> Self {
        Self {
            angle: 0.0,
            sweep_speed: std::f32::consts::TAU / 4.0, // Full rotation every 4 seconds
            display_visible: false,
        }
    }
}

pub struct RadarPlugin;

impl Plugin for RadarPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<RadarSweepState>()
            .configure_sets(Update, RadarSet::Input.run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, RadarSet::Update.after(RadarSet::Input).run_if(in_state(GameState::Exploring)))
            .configure_sets(Update, RadarSet::Visibility.after(RadarSet::Update).run_if(in_state(GameState::Exploring)))
            .add_systems(
                Update,
                (
                    radar_ping_system.in_set(RadarSet::Input),
                    toggle_radar_display.in_set(RadarSet::Input),
                    update_radar_pings.in_set(RadarSet::Update),
                    draw_radar_pings.in_set(RadarSet::Update).after(update_radar_pings),
                    update_radar_revealed.in_set(RadarSet::Update),
                    update_radar.in_set(RadarSet::Update),
                    update_depth_visibility.in_set(RadarSet::Visibility),
                ),
            );
    }
}

/// Toggle radar radar display with Tab key
fn toggle_radar_display(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut sweep_state: ResMut<RadarSweepState>,
    mut commands: Commands,
    existing_display: Query<Entity, With<RadarDisplay>>,
    radar_query: Query<(&Radar, &Module)>,
    // On the map Tab is its own (ui::map).
    map_open: Query<(), With<crate::ui::MapOverlay>>,
) {
    // Tab toggles radar display (Z is for ping)
    if map_open.is_empty() && keyboard.just_pressed(KeyCode::Tab) {
        let has_radar = radar_query.iter().any(|(_, m)| m.is_active);
        if !has_radar {
            return;
        }

        if sweep_state.display_visible {
            // Hide display
            sweep_state.display_visible = false;
            for entity in existing_display.iter() {
                commands.entity(entity).despawn();
            }
        } else {
            // Show display - spawn the radar circle
            sweep_state.display_visible = true;
            spawn_radar_display(&mut commands);
        }
    }
}

/// Spawn the circular radar display UI in the corner
fn spawn_radar_display(commands: &mut Commands) {
    let radar_size = 180.0;

    // Main radar container
    commands.spawn((
        (Node {
                position_type: PositionType::Absolute,
                right: Val::Px(10.0),
                bottom: Val::Px(10.0),
                width: Val::Px(radar_size),
                height: Val::Px(radar_size),
                ..default()
            }, BackgroundColor(Color::srgba(0.0, 0.1, 0.0, 0.7)), ZIndex(50)),
        RadarDisplay,
    )).with_children(|parent| {
        // Title
        parent.spawn((Text::new("RADAR"), TextFont { font_size: FontSize::Px(12.0), ..default() }, TextColor(Color::srgb(0.2, 0.8, 0.2)), Node {
                position_type: PositionType::Absolute,
                left: Val::Px(2.0),
                top: Val::Px(2.0),
                ..default()
            }));

        // Center dot (ship position)
        parent.spawn((Node {
                position_type: PositionType::Absolute,
                left: Val::Px(radar_size / 2.0 - 3.0),
                top: Val::Px(radar_size / 2.0 - 3.0),
                width: Val::Px(6.0),
                height: Val::Px(6.0),
                ..default()
            }, BackgroundColor(Color::srgb(0.3, 1.0, 0.3))));

        // Range rings (inner and outer)
        for ring_frac in [0.33, 0.66, 1.0] {
            let ring_size = radar_size * ring_frac;
            let offset = (radar_size - ring_size) / 2.0;
            parent.spawn((Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(offset),
                    top: Val::Px(offset),
                    width: Val::Px(ring_size),
                    height: Val::Px(ring_size),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                }, BorderColor::all(Color::srgba(0.1, 0.4, 0.1, 0.5)), BackgroundColor(Color::NONE)));
        }
    });
}

/// Update the radar radar display - sweep line and blips
pub fn update_radar(
    time: Res<Time>,
    mut sweep_state: ResMut<RadarSweepState>,
    radar_display: Query<Entity, With<RadarDisplay>>,
    radar_query: Query<(&Radar, &Module)>,
    ship_query: Query<&Transform, With<Ship>>,
    creature_query: Query<(&Transform, &Creature, Option<&RadarRevealed>), Without<Ship>>,
    poi_query: Query<(&Transform, &PointOfInterest), Without<Ship>>,
    ai_ship_query: Query<(Entity, &Transform, &AiShipType), With<AiShip>>,
    mut radar_ship_dot_query: Query<(Entity, &mut Node, &mut BackgroundColor, &RadarShipDot)>,
    contract_state: Res<ContractState>,
    sim: Res<WorldSimulation>,
    bounty_ship_query: Query<(&Transform, &BountyTarget), With<AiShip>>,
    camera_query: Query<&Projection, With<MainCamera>>,
    stations: Res<SystemStations>,
    mut commands: Commands,
    standing: Res<crate::diplomacy::FactionStanding>,
) {
    if !sweep_state.display_visible {
        return;
    }

    let Ok(radar_entity) = radar_display.single() else { return };
    let Ok(ship_transform) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();

    // Get radar range from the best equipped module...
    let module_range = radar_query.iter()
        .filter(|(_, m)| m.is_active)
        .map(|(s, _)| s.range)
        .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(500.0);
    // ...but the scope must ALWAYS out-reach what the player can see, or you
    // spot ships with your eyes before they blip. The visible area scales with
    // camera zoom, so a fixed range loses the instant you zoom out — take the
    // on-screen half-diagonal (center→corner) and lead it by RADAR_VISUAL_LEAD.
    const RADAR_VISUAL_LEAD: f32 = 1.5;
    let visible_lead = camera_query.single().ok()
        .and_then(|proj| match proj {
            Projection::Orthographic(o) => Some(o.area.size().length() * 0.5 * RADAR_VISUAL_LEAD),
            _ => None,
        })
        .unwrap_or(0.0);
    let radar_range = module_range.max(visible_lead);

    let radar_size = 180.0;
    let radar_half = radar_size / 2.0;

    // Advance sweep angle
    sweep_state.angle += sweep_state.sweep_speed * time.delta_secs();
    if sweep_state.angle > std::f32::consts::TAU {
        sweep_state.angle -= std::f32::consts::TAU;
    }

    // Note: blip lifetime ticking and despawning is handled by update_radar_revealed

    // Spawn blips for entities near the sweep line
    let sweep_tolerance = 0.15; // Radians - how wide the sweep detection is

    // Check creatures
    for (c_transform, creature, radar_revealed) in creature_query.iter() {
        let c_pos = c_transform.translation.truncate();
        let offset = c_pos - ship_pos;
        let dist = offset.length();

        if dist > radar_range {
            continue;
        }

        // Check if entity is near the sweep line
        let entity_angle = offset.y.atan2(offset.x);
        let angle_diff = (entity_angle - sweep_state.angle).abs();
        let angle_diff = angle_diff.min(std::f32::consts::TAU - angle_diff);

        if angle_diff < sweep_tolerance || radar_revealed.is_some() {
            // Map world position to radar position
            let radar_x = (offset.x / radar_range) * radar_half + radar_half;
            let radar_y = radar_half - (offset.y / radar_range) * radar_half; // Flip Y

            // Blip color based on creature threat
            let blip_color = match creature.creature_type {
                CreatureType::Leviathan => Color::srgb(1.0, 0.2, 0.2),
                CreatureType::Stalker => Color::srgb(1.0, 0.6, 0.2),
                CreatureType::ParasiteSwarm => Color::srgb(1.0, 1.0, 0.3),
                CreatureType::VoidDrifter => Color::srgb(0.5, 0.8, 0.5),
            };

            // Blip size based on creature threat
            let blip_size = match creature.creature_type {
                CreatureType::Leviathan => 8.0,
                CreatureType::Stalker => 5.0,
                _ => 3.0,
            };

            commands.spawn((
                (Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(radar_x - blip_size / 2.0),
                        top: Val::Px(radar_y - blip_size / 2.0),
                        width: Val::Px(blip_size),
                        height: Val::Px(blip_size),
                        ..default()
                    }, BackgroundColor(blip_color), ZIndex(51)),
                RadarBlip {
                    lifetime: Timer::from_seconds(4.0, TimerMode::Once),
                },
            )).insert(ChildOf(radar_entity));
        }
    }

    // Check POIs - show as cyan blips
    for (poi_transform, _poi) in poi_query.iter() {
        let p_pos = poi_transform.translation.truncate();
        let offset = p_pos - ship_pos;
        let dist = offset.length();

        if dist > radar_range {
            continue;
        }

        let entity_angle = offset.y.atan2(offset.x);
        let angle_diff = (entity_angle - sweep_state.angle).abs();
        let angle_diff = angle_diff.min(std::f32::consts::TAU - angle_diff);

        if angle_diff < sweep_tolerance {
            let radar_x = (offset.x / radar_range) * radar_half + radar_half;
            let radar_y = radar_half - (offset.y / radar_range) * radar_half;

            commands.spawn((
                (Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(radar_x - 3.0),
                        top: Val::Px(radar_y - 3.0),
                        width: Val::Px(6.0),
                        height: Val::Px(6.0),
                        ..default()
                    }, BackgroundColor(Color::srgb(0.2, 0.8, 1.0)), ZIndex(51)),
                RadarBlip {
                    lifetime: Timer::from_seconds(4.0, TimerMode::Once),
                },
            )).insert(ChildOf(radar_entity));
        }
    }

    // Check this system's stations — green beacons,
    // revealed by the sweep line like POIs (not every frame — that would
    // spam a fresh blip entity per station every frame instead of the
    // periodic sweep-triggered trickle every other blip type uses).
    for station_pos in stations.positions() {
        let offset = station_pos - ship_pos;
        let dist = offset.length();

        if dist > radar_range {
            continue;
        }

        let entity_angle = offset.y.atan2(offset.x);
        let angle_diff = (entity_angle - sweep_state.angle).abs();
        let angle_diff = angle_diff.min(std::f32::consts::TAU - angle_diff);
        if angle_diff >= sweep_tolerance {
            continue;
        }

        let radar_x = (offset.x / radar_range) * radar_half + radar_half;
        let radar_y = radar_half - (offset.y / radar_range) * radar_half;

        commands.spawn((
            (Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(radar_x - 4.0),
                    top: Val::Px(radar_y - 4.0),
                    width: Val::Px(8.0),
                    height: Val::Px(8.0),
                    ..default()
                }, BackgroundColor(Color::srgb(0.25, 1.0, 0.35)), ZIndex(51)),
            RadarBlip {
                lifetime: Timer::from_seconds(4.0, TimerMode::Once),
            },
        )).insert(ChildOf(radar_entity));
    }

    // Active bounty targets — bright red marker over whatever's at that
    // position, resolved from the real spawned entity if in range, else the
    // off-screen simulated position.
    let bounty_positions: Vec<Vec2> = contract_state.active_contracts.iter()
        .filter_map(|c| match &c.objective {
            ContractObjective::DestroyShip { target_id, destroyed: false, .. } => {
                bounty_ship_query.iter()
                    .find(|(_, b)| b.0 == *target_id)
                    .map(|(t, _)| t.translation.truncate())
                    .or_else(|| sim.bounty_position(*target_id))
            }
            _ => None,
        })
        .collect();
    for bounty_pos in bounty_positions {
        let offset = bounty_pos - ship_pos;
        let dist = offset.length();

        if dist > radar_range {
            continue;
        }

        let entity_angle = offset.y.atan2(offset.x);
        let angle_diff = (entity_angle - sweep_state.angle).abs();
        let angle_diff = angle_diff.min(std::f32::consts::TAU - angle_diff);
        if angle_diff >= sweep_tolerance {
            continue;
        }

        let radar_x = (offset.x / radar_range) * radar_half + radar_half;
        let radar_y = radar_half - (offset.y / radar_range) * radar_half;

        commands.spawn((
            (Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(radar_x - 5.0),
                    top: Val::Px(radar_y - 5.0),
                    width: Val::Px(10.0),
                    height: Val::Px(10.0),
                    ..default()
                }, BackgroundColor(Color::srgb(1.0, 0.15, 0.15)), ZIndex(52)),
            RadarBlip {
                lifetime: Timer::from_seconds(4.0, TimerMode::Once),
            },
        )).insert(ChildOf(radar_entity));
    }

    // AI ships — one PERSISTENT dot per ship, repositioned and recolored every
    // frame so each ship reads as a single dot sliding with it, instead of the
    // trail of stale sweep-blips the old per-frame spawn left as the player
    // moved. Colored by disposition (red hostile / white neutral; blue ally is
    // reserved — no player-allied ships exist yet).
    let mut desired: HashMap<Entity, (f32, f32, Color, f32)> = HashMap::new();
    for (ai_entity, ai_transform, ai_ship_type) in ai_ship_query.iter() {
        let offset = ai_transform.translation.truncate() - ship_pos;
        if offset.length() > radar_range {
            continue;
        }
        let radar_x = (offset.x / radar_range) * radar_half + radar_half;
        let radar_y = radar_half - (offset.y / radar_range) * radar_half;
        desired.insert(
            ai_entity,
            (radar_x, radar_y, radar_ship_color(standing.tier(*ai_ship_type)), radar_ship_size(*ai_ship_type)),
        );
    }
    // Move existing dots to their ship's current spot; drop dots whose ship left
    // range or despawned.
    for (dot_entity, mut node, mut bg, dot) in radar_ship_dot_query.iter_mut() {
        if let Some((x, y, color, size)) = desired.remove(&dot.0) {
            node.left = Val::Px(x - size / 2.0);
            node.top = Val::Px(y - size / 2.0);
            node.width = Val::Px(size);
            node.height = Val::Px(size);
            *bg = color.into();
        } else {
            commands.entity(dot_entity).despawn();
        }
    }
    // Spawn a dot for any ship that doesn't have one yet.
    for (ship, (x, y, color, size)) in desired {
        commands.spawn((
            (Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(x - size / 2.0),
                    top: Val::Px(y - size / 2.0),
                    width: Val::Px(size),
                    height: Val::Px(size),
                    ..default()
                }, BackgroundColor(color), ZIndex(51)),
            RadarShipDot(ship),
        )).insert(ChildOf(radar_entity));
    }
}

/// Handles Z key radar ping - spawns expanding ring, reveals entities, generates noise
fn radar_ping_system(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    // Our radar. Unscoped, the first active radar in the world answered --
    // which could be an enemy's, with its range.
    radar_query: Query<(&Radar, &Module), Without<crate::ai_ship::components::OwnedByAiShip>>,
    ship_query: Query<&Transform, With<Ship>>,
    mut noise_state: ResMut<NoiseState>,
    mut notifications: MessageWriter<ShowNotification>,
    mut galaxy_map: ResMut<GalaxyMap>,
    streaming: Res<SystemStreamingManager>,
) {
    if !keyboard.just_pressed(KeyCode::KeyZ) {
        return;
    }

    let Ok(ship_transform) = ship_query.single() else { return };

    // The longest-reaching active radar answers, so a better one fitted
    // beside the old one (a signature build) actually pings further.
    let radar_data = radar_query.iter()
        .filter(|(_, module)| module.is_active)
        .max_by(|a, b| a.0.range.total_cmp(&b.0.range))
        .map(|(radar, _)| (radar.range, radar.noise_on_ping));

    let Some((range, noise)) = radar_data else {
        notifications.write(ShowNotification {
            message: "No active radar module!".into(),
            notification_type: NotificationType::Warning,
            duration: 2.0,
        });
        return;
    };

    // Interstellar scan: reveal Unknown galaxy systems within scan range as
    // Located (Phase 5 active discovery — longer reach than passive proximity).
    let scan_pos = streaming.current_galaxy_pos;
    let mut newly_found = 0;
    for system in galaxy_map.systems.iter_mut() {
        if system.discovery == SystemDiscovery::Unknown
            && system.galaxy_pos.distance(scan_pos) <= GALAXY_SCAN_RANGE
        {
            system.discovery = SystemDiscovery::Located;
            newly_found += 1;
        }
    }
    if newly_found > 0 {
        notifications.write(ShowNotification {
            message: format!("Scan: {} new system{} detected - check the galaxy map (M).",
                newly_found, if newly_found == 1 { "" } else { "s" }),
            notification_type: NotificationType::Success,
            duration: 3.0,
        });
    } else {
        notifications.write(ShowNotification {
            message: "Scan complete - no new systems in range.".into(),
            notification_type: NotificationType::Info,
            duration: 2.0,
        });
    }

    // The ping itself: drawn as a line by draw_radar_pings, not a sprite.
    commands.spawn((
        Transform::from_translation(ship_transform.translation),
        RadarPing {
            radius: 0.0,
            max_radius: range,
            speed: 400.0,
            age: 0.0,
        },
    ));

    // Noise spike - attracts creatures
    noise_state.noise_level += noise;

    // No separate "PING!" notice: the ring going out is the ping, and the
    // scan result above is posted on the same press -- two notices a ping
    // pushed everything else up the stack.
}

/// Expands radar ping rings and reveals entities they pass through
/// Pale cyan, the HUD's own accent, rather than the old saturated green.
const PING_COLOR: (f32, f32, f32) = (0.55, 0.85, 0.90);
/// Peak opacity of the leading ring, at the moment it leaves the hull.
const PING_PEAK_ALPHA: f32 = 0.5;
/// The faint echo ring trails the leading edge at this fraction of its radius.
const PING_ECHO: f32 = 0.88;

/// Where a ping is at `age` seconds: (radius, opacity, finished).
///
/// It eases out -- quick off the hull, slowing as it spreads -- and fades as
/// it goes, so the far edge of the sweep arrives as a whisper. The old ring
/// was a sprite stretched to the ping's diameter, which stretched its line
/// along with it: a hairline at the hull became a band hundreds of units
/// thick at full range, bright green, over half the screen.
pub fn ping_shape(age: f32, max_radius: f32, speed: f32) -> (f32, f32, bool) {
    let lifetime = (max_radius / speed.max(1.0)).max(0.1);
    let t = (age / lifetime).clamp(0.0, 1.0);
    let eased = 1.0 - (1.0 - t).powi(3);
    let alpha = PING_PEAK_ALPHA * (1.0 - t).powf(1.5);
    (max_radius * eased, alpha, t >= 1.0)
}

/// Did the ring's edge cross `dist` between two frames? Swept rather than a
/// band test at one radius: the edge moves fastest just off the hull, and a
/// slow frame could otherwise step clean over a contact without seeing it.
fn ring_crossed(prev_radius: f32, radius: f32, dist: f32) -> bool {
    const EDGE: f32 = 30.0;
    dist >= prev_radius - EDGE && dist <= radius + EDGE
}

fn update_radar_pings(
    mut commands: Commands,
    time: Res<Time>,
    mut ping_query: Query<(Entity, &mut RadarPing, &Transform), Without<Creature>>,
    creature_query: Query<(Entity, &Transform, &Creature), Without<RadarPing>>,
    ai_ship_ping_query: Query<(Entity, &Transform), (With<AiShip>, Without<RadarPing>, Without<Creature>)>,
) {
    for (ping_entity, mut ping, transform) in ping_query.iter_mut() {
        ping.age += time.delta_secs();
        let prev_radius = ping.radius;
        let (radius, _, finished) = ping_shape(ping.age, ping.max_radius, ping.speed);
        ping.radius = radius;

        // Reveal creatures the ring passes through
        let ping_pos = transform.translation.truncate();
        for (c_entity, c_transform, _creature) in creature_query.iter() {
            let dist = c_transform.translation.truncate().distance(ping_pos);
            if ring_crossed(prev_radius, radius, dist) {
                commands.entity(c_entity).insert(RadarRevealed {
                    timer: Timer::from_seconds(3.0, TimerMode::Once),
                });
            }
        }

        // Reveal AI ships the ring passes through
        for (ai_entity, ai_transform) in ai_ship_ping_query.iter() {
            let dist = ai_transform.translation.truncate().distance(ping_pos);
            if ring_crossed(prev_radius, radius, dist) {
                commands.entity(ai_entity).insert(AiShipRadarContact {
                    noise_signature: 50.0,
                    revealed_timer: Timer::from_seconds(3.0, TimerMode::Once),
                });
            }
        }

        if finished {
            commands.entity(ping_entity).despawn();
        }
    }
}

/// Draws each ping as a thin line of constant on-screen width (a gizmo, so it
/// stays a hairline at any zoom), with a fainter echo trailing just inside it.
fn draw_radar_pings(mut gizmos: Gizmos, pings: Query<(&RadarPing, &Transform)>) {
    let (r, g, b) = PING_COLOR;
    for (ping, transform) in &pings {
        let (radius, alpha, _) = ping_shape(ping.age, ping.max_radius, ping.speed);
        if radius <= 1.0 || alpha <= 0.0 {
            continue;
        }
        let at = Isometry2d::from_translation(transform.translation.truncate());
        gizmos.circle_2d(at, radius, Color::srgba(r, g, b, alpha)).resolution(160);
        gizmos.circle_2d(at, radius * PING_ECHO, Color::srgba(r, g, b, alpha * 0.35)).resolution(160);
    }
}

/// Fades radar-revealed markers and ticks blip lifetimes
fn update_radar_revealed(
    mut commands: Commands,
    time: Res<Time>,
    mut revealed_query: Query<(Entity, &mut RadarRevealed)>,
    mut blip_query: Query<(Entity, &mut RadarBlip)>,
) {
    for (entity, mut revealed) in revealed_query.iter_mut() {
        revealed.timer.tick(time.delta());
        if revealed.timer.is_finished() {
            commands.entity(entity).remove::<RadarRevealed>();
        }
    }

    // Tick blip lifetimes
    for (entity, mut blip) in blip_query.iter_mut() {
        blip.lifetime.tick(time.delta());
        if blip.lifetime.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Depth-based visibility: dims/hides entities beyond visibility range
fn update_depth_visibility(
    ship_state: Res<DepthState>,
    ship_query: Query<&Transform, With<Ship>>,
    light_query: Query<(&ShipLight, &Module)>,
    mut creature_query: Query<(&Transform, &mut Sprite, Option<&RadarRevealed>), (With<Creature>, Without<Ship>, Without<PointOfInterest>, Without<WorldDecoration>)>,
    mut poi_query: Query<(&Transform, &mut Sprite, Option<&RadarRevealed>), (With<PointOfInterest>, Without<Creature>, Without<Ship>, Without<WorldDecoration>)>,
    mut deco_query: Query<(&Transform, &mut Sprite), (With<WorldDecoration>, Without<Creature>, Without<PointOfInterest>, Without<Ship>)>,
) {
    let Ok(ship_transform) = ship_query.single() else { return };
    let ship_pos = ship_transform.translation.truncate();
    let depth = ship_state.current_depth;

    // Calculate visibility range based on depth zone
    let base_visibility = match depth {
        d if d < 200.0 => 800.0,
        d if d < 500.0 => 400.0,
        d if d < 1000.0 => 200.0,
        _ => 100.0,
    };

    // Sub lights extend visibility
    let light_bonus: f32 = light_query.iter()
        .filter(|(_, m)| m.is_active)
        .map(|(light, _)| light.range * light.intensity)
        .sum();

    let visibility_range = base_visibility + light_bonus * 0.5;

    // Apply to creatures
    for (transform, mut sprite, radar_revealed) in creature_query.iter_mut() {
        let dist = transform.translation.truncate().distance(ship_pos);

        if radar_revealed.is_some() {
            sprite.color.set_alpha(1.0);
        } else if dist > visibility_range {
            sprite.color.set_alpha(0.0);
        } else {
            let fade = 1.0 - ((dist / visibility_range).powi(2));
            sprite.color.set_alpha(fade.clamp(0.1, 1.0));
        }
    }

    // Apply to POIs
    for (transform, mut sprite, radar_revealed) in poi_query.iter_mut() {
        let dist = transform.translation.truncate().distance(ship_pos);

        if radar_revealed.is_some() {
            sprite.color.set_alpha(1.0);
        } else if dist > visibility_range {
            sprite.color.set_alpha(0.0);
        } else {
            let fade = 1.0 - ((dist / visibility_range).powi(2));
            sprite.color.set_alpha(fade.clamp(0.1, 1.0));
        }
    }

    // Apply to decorations
    for (transform, mut sprite) in deco_query.iter_mut() {
        let dist = transform.translation.truncate().distance(ship_pos);

        if dist > visibility_range {
            sprite.color.set_alpha(0.0);
        } else {
            let fade = 1.0 - ((dist / visibility_range).powi(2));
            sprite.color.set_alpha(fade.clamp(0.1, 1.0));
        }
    }

}

#[cfg(test)]
mod ping_tests {
    use super::*;

    /// Out quickly, then slower; brightest at the hull, gone at the edge.
    #[test]
    fn a_ping_eases_out_and_fades() {
        let (max, speed) = (4000.0, 400.0); // a ten-second sweep
        let (r1, a1, _) = ping_shape(1.0, max, speed);
        let (r2, a2, _) = ping_shape(2.0, max, speed);
        let (r9, _, _) = ping_shape(9.0, max, speed);
        let (r10, a10, done) = ping_shape(10.0, max, speed);
        assert!(r1 > r2 - r1, "first second should cover more ground than the second");
        assert!(r10 - r9 < r2 - r1, "should be slowing by the end");
        assert!(a1 > a2 && a2 > 0.0);
        assert!((r10 - max).abs() < 0.01 && a10 == 0.0 && done);
    }

    /// A slow frame that carries the edge past a contact still reveals it.
    #[test]
    fn a_contact_the_edge_jumps_over_is_still_seen() {
        assert!(ring_crossed(1000.0, 1300.0, 1150.0));
        assert!(!ring_crossed(1000.0, 1300.0, 1600.0));
        assert!(!ring_crossed(1000.0, 1300.0, 900.0));
    }
}
