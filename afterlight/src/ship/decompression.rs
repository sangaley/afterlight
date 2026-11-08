use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::events::*;
use crate::building::rooms::RoomMap;

/// Updates decompression on a per-room basis.
/// RoomDepressurized events mark rooms as breached, air escapes from breached rooms,
/// hull segments sync to their room's depressurization level via tile_to_room.
pub fn update_decompression(
    time: Res<Time>,
    ship_query: Query<Entity, With<Ship>>,
    hull_query: Query<(&HullSegment, &Transform, &ChildOf)>,
    room_map: Res<RoomMap>,
    mut oxygen_state: ResMut<OxygenState>,
    air: Res<crate::ship::air::AirField>,
    mut player_body: Query<(&GlobalTransform, &mut Velocity, &mut ShipPhysics), With<Ship>>,
) {
    // Player ship only: hull_query spans every ship in the world (this
    // component is shared with AI ships). Unscoped, an AI ship whose local
    // grid_position happened to numerically match one of the player's
    // breached tiles got flagged depressurized too, and its
    // depressurization_level was then summed into the player's oxygen
    // drain below — the player could lose all their air from damage
    // happening on a ship they've never even seen.
    let Ok(player_ship) = ship_query.single() else { return };

    // Air level and is_breached are owned by ship::air, which runs just
    // before this and derives both from the per-tile pressure field. What is
    // left here is everything downstream of them: the hull sync, the oxygen
    // drain, and vent thrust.
    let dt = time.delta_secs();

    // Hull breach state is NOT derived from room air any more. It used to be
    // `is_depressurized = air < 1.0` for every tile of the room, which meant a
    // single hole flagged all 64 plates around it as holed: 64 fog plumes, and
    // 64 tiles of vent thrust that never stopped because a partially-pressured
    // room never reaches the "fully drained" state the old model always got to
    // within seconds. `mark_breached_hull` sets it from HullBreached instead,
    // and crew sealing clears it, which is the loop the repair code expects.
    let mut total_air_loss = 0.0;
    for (hull, _transform, parent) in hull_query.iter() {
        if parent.parent() != player_ship { continue; }
        if hull.is_depressurized {
            total_air_loss += hull.depressurization_level;
        }
    }

    // Decompression drains oxygen — air escaping into the void
    for room in room_map.rooms.iter() {
        if room.is_breached {
            total_air_loss += (1.0 - room.air_level) * room.tiles.len() as f32;
        }
    }

    // Each unit of air loss drains oxygen at 3.0 per second — breach without bulkheads = crew dies
    let oxygen_drain = total_air_loss * 3.0 * dt;
    oxygen_state.current_oxygen = (oxygen_state.current_oxygen - oxygen_drain).max(0.0);

    // VENT THRUST — air jetting out of a breach is reaction mass, so the ship
    // gets pushed the other way and, the hole being off-centre, slowly yaws.
    //
    // Driven by the air actually leaving (ship::air's vent map) rather than by
    // a count of flagged plates. The old version summed one unit of thrust per
    // "depressurized" tile and stopped only once a room hit exactly zero air,
    // which under a diffusing field never happens — a partly-drained
    // compartment shoved the ship at a constant 153 u/s2 indefinitely. Thrust
    // now falls with the pressure behind it and reaches zero when the air is
    // gone, which is also what makes it feel like a jet running out.
    let mut vent_accel_local = Vec2::ZERO;
    let mut vent_torque = 0.0_f32;
    let mut total_flow = 0.0_f32;
    for (&tile, &(outward, conductance)) in air.vents.iter() {
        let pressure = air.pressure.get(&tile).copied().unwrap_or(0.0);
        if pressure <= 0.01 { continue; }
        let Some(out_dir) = outward.try_normalize() else { continue };

        // Mass leaving here per second, in the same units the vent system
        // drains with.
        let rate = pressure * conductance;
        total_flow += rate;

        let tile_local = crate::building::grid_to_local(tile);
        let thrust = -out_dir * rate; // jet goes out, ship goes the other way
        vent_accel_local += thrust;
        vent_torque += tile_local.x * thrust.y - tile_local.y * thrust.x;
    }

    if total_flow > 0.001 {
        if let Ok((gt, mut velocity, mut physics)) = player_body.single_mut() {
            // sqrt keeps a colander from out-thrusting a puncture: more holes
            // empty the ship faster, they do not push it harder in proportion.
            let strength = 45.0 * total_flow.sqrt();
            let direction = vent_accel_local.normalize_or_zero();
            let world_accel = gt.rotation() * (direction * strength).extend(0.0);
            velocity.0 += world_accel.truncate() * dt;
            physics.angular_velocity += (vent_torque * 0.0008).clamp(-0.25, 0.25) * dt;

            if (time.elapsed_secs() % 1.0) < dt {
                info!(
                    "[VENT] {} tile(s) venting, flow {:.2}, accel {:.1} u/s2",
                    air.vents.len(), total_flow, strength
                );
            }
        }
    }
}

/// Crew in Repairing state seal breaches in their room.
/// RepairBay modules in the same room multiply seal power.
/// HullSeal modules provide automated breach sealing independent of crew.
pub fn seal_breach_system(
    time: Res<Time>,
    ship_query: Query<Entity, With<Ship>>,
    crew_query: Query<(&CrewMember, &CrewRoomLocation)>,
    repair_bays: Query<(&Module, &RepairSystem, &ChildOf), Without<DestroyedModule>>,
    hull_seals: Query<(&HullSealComp, &Module, &ChildOf), Without<DestroyedModule>>,
    room_map: Res<RoomMap>,
    mut air: ResMut<crate::ship::air::AirField>,
) {
    let dt = time.delta_secs();
    // Our seals and bays only. Rooms and the air field are the player's, and
    // a module's grid_position is local to its own ship -- an enemy RepairBay
    // at the same cell numbers was speeding up our sealing. (Crew need no
    // filter: CrewRoomLocation only exists on crew with a Transform, which AI
    // crew never get.)
    let Ok(player_ship) = ship_query.single() else { return };

    // Build per-room seal power from repairing crew
    let mut room_seal_power: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();
    for (crew, location) in crew_query.iter() {
        if crew.state != CrewState::Repairing || crew.health <= 0.0 {
            continue;
        }
        if let Some(room_id) = location.room_id {
            *room_seal_power.entry(room_id).or_insert(0.0) += 0.03;
        }
    }

    // Automated HullSeal modules add seal power to their room
    for (seal, module, parent) in hull_seals.iter() {
        if !module.is_active || parent.parent() != player_ship { continue; }
        if let Some(&room_id) = room_map.tile_to_room.get(&module.grid_position) {
            *room_seal_power.entry(room_id).or_insert(0.0) += seal.seal_rate;
        }
    }

    if room_seal_power.is_empty() {
        return;
    }

    // Build per-room RepairBay boost
    let mut room_repair_boost: std::collections::HashMap<usize, f32> = std::collections::HashMap::new();
    for (module, repair_sys, parent) in repair_bays.iter() {
        if parent.parent() != player_ship { continue; }
        if let Some(&room_id) = room_map.tile_to_room.get(&module.grid_position) {
            let boost = room_repair_boost.entry(room_id).or_insert(1.0);
            *boost += repair_sys.repair_rate * 0.1;
        }
    }

    // Apply sealing to each room, restoring air to its tiles. This has to
    // write the pressure field rather than room.air_level: air::sync_room_air
    // republishes air_level from the tiles every frame, so a write here would
    // be gone before anything read it.
    for (room_id, seal_power) in room_seal_power.iter() {
        if let Some(room) = room_map.rooms.get(*room_id) {
            // Re-pressurising is only possible once the hull is actually shut.
            // Against a live hole this was manufacturing air out of nothing at
            // roughly the rate it was leaving, so a breached compartment sat
            // at a permanent stalemate and never emptied -- damage control as
            // an infinite atmosphere supply. Patching the plate itself is
            // crew_repair_system's job, via depressurization_level.
            if room.tiles.iter().any(|t| air.vents.contains_key(t)) {
                continue;
            }
            let boost = room_repair_boost.get(room_id).copied().unwrap_or(1.0);
            let restore = seal_power * boost * dt;
            for tile in &room.tiles {
                let pressure = air.pressure.entry(*tile).or_insert(1.0);
                *pressure = (*pressure + restore).min(1.0);
            }
        }
    }
}

/// Reads ToggleBulkhead events and seals/unseals bulkhead doors.
pub fn handle_bulkhead_toggle(
    mut commands: Commands,
    mut events: MessageReader<ToggleBulkhead>,
    mut hull_query: Query<(&HullSegment, &mut Sprite)>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    for event in events.read() {
        let Ok((hull, mut sprite)) = hull_query.get_mut(event.segment) else {
            continue;
        };
        if hull.hull_layer != HullLayer::BulkheadDoor {
            continue;
        }
        if event.seal {
            commands.entity(event.segment).insert(BulkheadSealed);
            sprite.color = Color::srgb(0.8, 0.2, 0.2); // Red = sealed
            notifications.write(ShowNotification {
                message: "Bulkhead sealed - section airtight!".into(),
                notification_type: NotificationType::Warning,
                duration: 2.0,
            });
        } else {
            commands.entity(event.segment).remove::<BulkheadSealed>();
            sprite.color = Color::srgb(0.9, 0.8, 0.7); // Default bulkhead color
            notifications.write(ShowNotification {
                message: "Bulkhead opened!".into(),
                notification_type: NotificationType::Info,
                duration: 2.0,
            });
        }
    }
}

/// F key + left click = toggle nearest bulkhead door within 50 world units.
pub fn bulkhead_seal_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform)>,
    hull_query: Query<(Entity, &HullSegment, &Transform, Option<&BulkheadSealed>)>,
    mut toggle_events: MessageWriter<ToggleBulkhead>,
) {
    if !keyboard.pressed(KeyCode::KeyF) || !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = windows.single() else { return };
    let Ok((camera, camera_transform)) = camera_query.single() else { return };

    let Some(cursor_world) = window.cursor_position()
        .and_then(|p| camera.viewport_to_world_2d(camera_transform, p).ok())
    else {
        return;
    };

    // Find nearest bulkhead door within 50 world units
    let mut nearest: Option<(Entity, f32, bool)> = None;
    for (entity, hull, transform, sealed) in hull_query.iter() {
        if hull.hull_layer != HullLayer::BulkheadDoor {
            continue;
        }
        let dist = transform.translation.truncate().distance(cursor_world);
        if dist < 50.0 {
            if nearest.map_or(true, |(_, d, _)| dist < d) {
                nearest = Some((entity, dist, sealed.is_some()));
            }
        }
    }

    if let Some((entity, _, is_sealed)) = nearest {
        toggle_events.write(ToggleBulkhead {
            segment: entity,
            seal: !is_sealed,
        });
    }
}
