use bevy::prelude::*;
use crate::components::*;
use crate::resources::*;
use crate::events::*;

// Helper functions to get effective engine stats (CalculatedStats or base Engine)
fn get_engine_thrust(calculated: Option<&CalculatedStats>, engine: &Engine) -> f32 {
    calculated
        .and_then(|c| c.engine.as_ref())
        .map(|e| e.thrust)
        .unwrap_or(engine.thrust)
}


/// Handles ship input
pub fn ship_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut input_state: ResMut<InputState>,
) {
    let mut movement = Vec2::ZERO;

    // W/S: throttle forward/reverse
    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        movement.y = 1.0; // forward thrust
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        movement.y = -1.0; // reverse
    }

    // A/D: strafe left/right (facing follows the mouse cursor)
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        movement.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        movement.x += 1.0;
    }

    // Q/E used to drive "vertical thrusters" — a submarine-era holdover that
    // shoved the ship along world +Y/-Y regardless of facing, duplicating the
    // A/D strafe. Gone: WASD is the whole translation control now.

    input_state.movement = movement;
    // Shift: brake — retro-thrust against whatever direction we're drifting
    input_state.brake = keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight);
}

/// Cells of the hull the mass, drag and thrust were tuned on (the starter as
/// first drawn). A bigger hull is heavier and broader in proportion, so
/// speed comes from engines per block -- the way an AI ship's does
/// (ai_ship::movement) -- and doubling a ship with its engines doesn't
/// make it four times as quick.
const TUNED_CELLS: f32 = 210.0;

/// How heavy a hull of this many cells flies, against the 210 cells a ship's
/// mass and drag were tuned on.
pub fn heft_of(cells: usize) -> f32 {
    (cells as f32 / TUNED_CELLS).max(0.25)
}

/// Converts raw engine thrust into usable acceleration. Without this, the
/// starter ship's 1200 thrust / 1200 mass gave 1 unit/s² — one grid cell
/// per second per second, i.e. imperceptible. 180x puts cruising speed a
/// few seconds of burn away while keeping engine count meaningful.
const THRUST_SCALE: f32 = 180.0;

/// Lowest fraction of thrust a fully power-starved engine channel still puts
/// out. Routing power away from engines slows the ship, but never strands it —
/// you can always crawl home. See `ship_movement` / `update_power_allocation`.
const ENGINE_POWER_FLOOR: f32 = 0.25;

/// Max yaw rate at full deflection (rad/s). ~26°/s — a heavy warship, not a
/// fighter. Deliberately slow: the turrets aim independently now, so the hull
/// turns for positioning/thrust, not to point the guns. (Was 0.68, ~39°/s:
/// the doubled starter swung round too readily.)
const MAX_TURN_RATE: f32 = 0.45;
/// How quickly angular velocity reaches the target rate (per second). Lower =
/// more rotational inertia — the ship takes a moment to build and shed its spin
/// instead of snapping to the target rate.
const TURN_RESPONSE: f32 = 3.2;
/// Turn rate per radian of aim error. Lower so the ship eases onto heading
/// rather than darting after every small cursor movement.
const TURN_GAIN: f32 = 4.5;
/// When coasting (no thrust input, no brake), speed halves every this many
/// seconds. Pure Newtonian drift meant the ship "kept flying forward
/// forever" after any tap of W.
const COAST_HALF_LIFE: f32 = 1.4;
/// Flight assist: while thrusting forward, existing velocity is gently swung
/// toward the ship's facing, so turns become course changes instead of
/// endless sideways drift. 0.0 = pure Newtonian.
const VELOCITY_ALIGN_RATE: f32 = 1.2;
/// Share of full thrust the range-holding helm uses (ship::keep_range): about
/// what main drive and strafe jets give between them off-axis.
const AUTOPILOT_SHARE: f32 = 0.75;

/// How much of the full turn rate (and how quickly it builds) a hull of
/// these cells gets. Turning is about the hull's spread round its middle --
/// its moment of inertia, every cell weighing the same -- so a long ship
/// turns slower than a compact one of the same size. The starter-sized ship
/// keeps the full rate; nothing turns faster than that.
pub fn turn_scale(cells: impl Iterator<Item = IVec2> + Clone) -> f32 {
    /// The starter's hull, drawn twice as big (840 cells; tools/ship-scale):
    /// it turns like the 210-cell one the turn constants were tuned on.
    const STARTER_INERTIA: f32 = 13_700.0 * 16.0;
    let n = cells.clone().count();
    if n == 0 {
        return 1.0;
    }
    let mid = cells.clone().fold(Vec2::ZERO, |a, c| a + c.as_vec2()) / n as f32;
    let inertia: f32 = cells.map(|c| (c.as_vec2() - mid).length_squared()).sum();
    // Gentler than physics (1/I) would have it, so a big ship is heavy to
    // swing round but never a chore: ~0.4 for a hull four times the starter's.
    (STARTER_INERTIA / inertia.max(1.0)).powf(0.35).clamp(0.2, 1.0)
}

/// Applies space physics to ship movement (no drag, inertial flight)
pub fn ship_movement(
    time: Res<Time>,
    input_state: Res<InputState>,
    _config: Res<GameConfig>,
    camera_state: Res<crate::camera::CameraState>,
    // Without<OwnedByAiShip>: AI ships now carry real Engine/ModuleEfficiency
    // data too (see ai_ship::crew) — unscoped, this sum would let nearby AI
    // ships' staffed engines add thrust to the PLAYER's own ship the moment
    // any AI ship has crew (same class of leak the projectile-ownership and
    // staffing-HUD work already had to guard against elsewhere).
    //
    // ChildOf as well: a shot-off engine is no longer an AI block but is no
    // longer ours either. Without the parent check a severed engine kept
    // pushing the player from wherever it had drifted to, for free (as
    // update_fuel_consumption already knew: it never billed for it).
    engine_query: Query<(&Engine, &Module, Option<&CalculatedStats>, Option<&ModuleEfficiency>, &ChildOf), Without<crate::ai_ship::components::OwnedByAiShip>>,
    mut ship_query: Query<(Entity, &mut Transform, &mut Velocity, &mut ShipPhysics, &crate::building::layout::LayoutVersion, Option<&Children>), With<Ship>>,
    hull_cells: Query<&HullSegment>,
    // The hull's turn scale and heft (its size against TUNED_CELLS), and the
    // layout they were worked out for.
    mut turning: Local<Option<(crate::building::layout::LayoutVersion, f32, f32)>>,
    windows_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<crate::camera::MainCamera>>,
    debug_tuning: Res<crate::debug::DebugTuning>,
    power_channels: Res<crate::resources::PowerChannels>,
) {
    let Ok((player_ship, mut transform, mut velocity, mut physics, layout, children)) = ship_query.single_mut() else {
        return;
    };
    let (turn, heft) = match *turning {
        Some((v, scale, heft)) if v == *layout => (scale, heft),
        _ => {
            let cells: Vec<IVec2> = children
                .into_iter()
                .flatten()
                .filter_map(|c| hull_cells.get(*c).ok())
                .map(|h| h.grid_position)
                .collect();
            let scale = turn_scale(cells.iter().copied());
            let heft = heft_of(cells.len());
            *turning = Some((*layout, scale, heft));
            (scale, heft)
        }
    };
    let mass = physics.mass * heft;

    let dt = time.delta_secs();

    // Routed engine power scales thrust. Floored so a starved (or reactor-down)
    // ship can always limp instead of getting stranded dead in space.
    let engine_power = power_channels.engines_mult.max(ENGINE_POWER_FLOOR);

    // Calculate total thrust from active engines
    let total_thrust: f32 = engine_query
        .iter()
        .filter(|(_, module, _, _, parent)| module.is_active && parent.parent() == player_ship)
        .map(|(engine, module, calculated_stats, eff, _)| {
            let efficiency = effective_efficiency(module, eff);
            get_engine_thrust(calculated_stats, engine) * efficiency
        })
        .sum::<f32>() * debug_tuning.speed_mult * engine_power;

    // --- FACING: nose follows the aim source ---
    // Controller right stick when it has aim (see InputState.gamepad_aim),
    // the mouse cursor otherwise. Proportional controller: turn rate scales
    // with how far off-target the nose is, capped at MAX_TURN_RATE, so the
    // ship settles on the target smoothly instead of oscillating past it.
    physics.rudder = input_state.movement.x;
    // While free-looking (holding T), the cursor is being used to pan the
    // camera, not aim — freezing the turn here stops the ship spinning to
    // face wherever the player happens to be looking.
    if !camera_state.free_look_active {
        let target_angle = if let Some(aim) = input_state.gamepad_aim {
            Some(aim.y.atan2(aim.x))
        } else if let (Ok(window), Ok((camera, cam_gt))) =
            (windows_query.single(), camera_query.single())
        {
            window.cursor_position()
                .and_then(|cursor| camera.viewport_to_world_2d(cam_gt, cursor).ok())
                .map(|cursor_world| cursor_world - transform.translation.truncate())
                .filter(|to_cursor| to_cursor.length_squared() > 4.0)
                .map(|to_cursor| to_cursor.y.atan2(to_cursor.x))
        } else {
            None
        };

        if let Some(target_angle) = target_angle {
            let mut diff = target_angle - physics.rotation;
            while diff > std::f32::consts::PI { diff -= std::f32::consts::TAU; }
            while diff < -std::f32::consts::PI { diff += std::f32::consts::TAU; }

            let target_rate = (diff * TURN_GAIN).clamp(-MAX_TURN_RATE * turn, MAX_TURN_RATE * turn);
            let blend = (TURN_RESPONSE * turn * dt).min(1.0);
            physics.angular_velocity += (target_rate - physics.angular_velocity) * blend;
        }
    } else {
        // Decay any leftover turn rate instead of leaving it frozen —
        // otherwise the ship keeps coasting on whatever angular velocity
        // it had the instant free-look was pressed.
        let blend = (TURN_RESPONSE * dt).min(1.0);
        physics.angular_velocity -= physics.angular_velocity * blend;
    }
    physics.rotation += physics.angular_velocity * dt;

    // The ship root actually rotates — previously facing only existed in the
    // physics math and the hull visual just mirrored left/right (side-view
    // submarine holdover). Rotating the root carries all hull/module children.
    transform.rotation = Quat::from_rotation_z(physics.rotation);

    // --- THROTTLE ---
    let throttle_input = input_state.movement.y;
    physics.throttle = physics.throttle + (throttle_input - physics.throttle) * 3.0 * dt;

    // Direction ship faces
    let facing = Vec2::new(physics.rotation.cos(), physics.rotation.sin());

    // Thrust force: forward/reverse along facing plus lateral strafe (A/D).
    // Strafe runs at 50% main thrust — maneuvering jets, not the main drive.
    let right = Vec2::new(facing.y, -facing.x);
    let thrust_force = facing * total_thrust * physics.throttle * THRUST_SCALE
        + right * total_thrust * input_state.movement.x * 0.5 * THRUST_SCALE
        // Holding range on a locked ship: the drive and the jets together,
        // whichever way the nose happens to point.
        + input_state.autopilot * total_thrust * AUTOPILOT_SHARE * THRUST_SCALE;

    // --- SPACE DRAG (minimal — just light dampening for gameplay) ---
    let v_sq = velocity.0.length_squared();
    let drag_magnitude = 0.5 * physics.drag_coefficient * v_sq * physics.frontal_area * heft * 0.00002;
    let drag_force = if v_sq > 0.001 {
        -velocity.0.normalize() * drag_magnitude
    } else {
        Vec2::ZERO
    };

    // --- NET FORCE ---
    // (No vertical-thruster term any more — see ship_input: Q/E is gone.)
    let net_force = thrust_force + drag_force;
    let acceleration = net_force / mass;

    // Update velocity
    velocity.0 += acceleration * dt;
    if std::env::var("DEPTHS_HELM_TRACE").is_ok() && (time.elapsed_secs() % 1.0) < dt {
        let mine: Vec<_> = engine_query.iter().filter(|e| e.4.parent() == player_ship).collect();
        info!(
            "HELM engines {} active of {}, efficiency {:?}",
            mine.iter().filter(|e| e.1.is_active).count(),
            mine.len(),
            mine.iter().map(|e| (effective_efficiency(e.1, e.3) * 100.0) as i32).collect::<Vec<_>>()
        );
        info!(
            "HELM thrust {:.0}, mass {:.0}, accel {:.0} (drag {:.0}), speed {:.0}, autopilot {:?}, throttle {:.2}",
            total_thrust, mass, acceleration.length(), drag_force.length() / mass, velocity.0.length(), input_state.autopilot, physics.throttle
        );
    }

    // Brake (Shift): retro-thrust straight against the drift. In a dragless
    // void this is the only way to actually stop.
    if input_state.brake {
        let speed = velocity.0.length();
        if speed > 1.0 {
            let decel = (total_thrust * THRUST_SCALE / mass) * dt;
            let new_speed = (speed - decel).max(0.0);
            velocity.0 = velocity.0 * (new_speed / speed);
        } else {
            velocity.0 = Vec2::ZERO;
        }
    }

    // Flight assist: under forward thrust, swing existing momentum toward
    // the ship's facing so a turn actually changes course (see const doc).
    if !input_state.brake && physics.throttle > 0.1 && VELOCITY_ALIGN_RATE > 0.0 {
        let speed = velocity.0.length();
        if speed > 1.0 {
            let t = (VELOCITY_ALIGN_RATE * physics.throttle * dt).min(1.0);
            velocity.0 = velocity.0.lerp(facing * speed, t);
        }
    }

    // Coast damping: with no thrust input and no brake, bleed speed off
    // automatically (arcade handling — release W and the ship settles).
    // The range-holding helm counts as a hand on the stick: damping under it
    // capped its speed far below the ship's, so anything faster outran it.
    if settles(input_state.movement + input_state.autopilot, input_state.brake, total_thrust) {
        let decay = (0.5_f32).powf(dt / COAST_HALF_LIFE);
        velocity.0 *= decay;
        if velocity.0.length_squared() < 4.0 {
            velocity.0 = Vec2::ZERO;
        }
    }

    // Apply velocity to position
    transform.translation.x += velocity.0.x * dt;
    transform.translation.y += velocity.0.y * dt;

    // (The old left/right sprite mirror is gone — the root now truly rotates.)
    transform.scale.x = transform.scale.x.abs();
}

/// Whether the arcade coast damping applies this frame: no stick and no brake
/// -- or no lit engine at all. Holding W with a dead drive does nothing, and
/// it used to switch the damping off as well, so a ship that ran dry at speed
/// kept every bit of it: one autoplay run coasted into Haven Station at
/// ~1,000 u/s with no way to brake and lost nine crew to the breach.
fn settles(movement: Vec2, brake: bool, total_thrust: f32) -> bool {
    total_thrust <= 0.0 || (!brake && movement.y.abs() < 0.05 && movement.x.abs() < 0.05)
}

/// Tracks how far the ship is from Haven Station (the origin). Displayed in km
/// on the HUD; also drives zone progression, radiation and spawn tables.
pub fn update_depth(
    mut ship_query: Query<(&Transform, &mut Depth), With<Ship>>,
) {
    let Ok((transform, mut depth)) = ship_query.single_mut() else {
        return;
    };

    // Distance from the world origin. This is what the depth vignette, the
    // HUD range readout and the camera are all calibrated against, so it stays
    // as it is.
    //
    // It is NOT a meaningful measure of "how far out am I" now that the galaxy
    // exists — every system but Haven centres hundreds of thousands of units
    // away — but the fix for that belongs in the handful of systems that need
    // a real answer, not here. Redefining this one field to mean distance from
    // the nearest station blacked out the entire screen: the vignette treats
    // anything past ~20 as deep space, and the ship starts ~800 units from
    // Haven's berth. See world::distance_from_safety for the honest measure.
    depth.0 = transform.translation.truncate().length();
}

/// An engine that went dark because the tank ran dry, as opposed to one that
/// is damaged or switched off. Only these relight on their own.
#[derive(Component)]
pub struct FuelStarved;

/// Fuel an empty tank trickles back up to inside a system with stations:
/// enough for a short dash toward one, little enough that running dry still
/// costs you the trip.
const EMERGENCY_RESERVE: f32 = 30.0;

/// Consumes fuel from engines and deactivates them when fuel runs out.
/// PLAYER ENGINES ONLY: AI ships reuse the same Engine components, and an
/// unscoped query made every spawned AI ship's engines drain the player's
/// fuel tank (a raider wave emptied it in seconds).
pub fn update_fuel_consumption(
    time: Res<Time>,
    mut fuel_state: ResMut<FuelState>,
    mut engine_query: Query<(Entity, &Engine, &mut Module, &ChildOf, Has<FuelStarved>)>,
    mut commands: Commands,
    ship_query: Query<Entity, With<Ship>>,
    physics_query: Query<&ShipPhysics, With<Ship>>,
    stations: Res<crate::world::home_base::SystemStations>,
    mut notifications: MessageWriter<ShowNotification>,
    mut warned_25: Local<bool>,
    mut warned_10: Local<bool>,
    mut emergency: Local<bool>,
    debug_tuning: Res<crate::debug::DebugTuning>,
) {
    let Ok(player_ship) = ship_query.single() else { return };
    let Ok(physics) = physics_query.single() else { return };
    let dt = time.delta_secs();
    let mut total_consumption = 0.0;

    // Burn scales with how hard the ship is actually being driven.
    //
    // This used to key off `module.is_active`, which means POWERED, not
    // throttled — so five standard engines drank 4 fuel a second while the
    // ship sat still, emptying a 1500 tank in about six minutes of doing
    // nothing. The tutorial tells the player to "watch FUEL tick down as you
    // burn", which was not what happened.
    //
    // A small idle draw remains: engines that are lit still cost something,
    // and shutting them down is a real decision.
    const IDLE_FRACTION: f32 = 0.12;
    let drive = physics.throttle.abs().clamp(0.0, 1.0);
    let draw = IDLE_FRACTION + (1.0 - IDLE_FRACTION) * drive;
    for (_, engine, module, parent, _) in engine_query.iter() {
        if parent.parent() != player_ship { continue; }
        if module.is_active {
            total_consumption +=
                engine.fuel_consumption * fuel_state.fuel_consumption_rate * draw * dt;
        }
    }

    if total_consumption > 0.0 && !debug_tuning.infinite_fuel {
        fuel_state.current_fuel = (fuel_state.current_fuel - total_consumption).max(0.0);
    }
    // Taken before either trickle below tops the tank up, or the shutdown
    // check at the end would never see it empty.
    let ran_dry = fuel_state.current_fuel <= 0.0;

    // Stranding guard. A blind warp can land in genuinely empty space: no
    // system, so no station, so no refuel. With jump cost now rising as the
    // square of distance, a player who arrives there low on fuel could be
    // unable to leave at all — a hard lock with nothing on screen explaining
    // it. Trickle back up to just enough for the cheapest possible jump, and
    // only ever when there is nowhere to dock.
    const STRANDED_RESERVE: f32 = 90.0; // a little over INTERSTELLAR_BASE_FUEL
    if stations.sites.is_empty() && fuel_state.current_fuel < STRANDED_RESERVE {
        fuel_state.current_fuel =
            (fuel_state.current_fuel + 6.0 * dt).min(STRANDED_RESERVE);
    }
    // And inside a system: a dry tank with stations out of reach was a hard
    // lock too -- no thrust, no way to a berth. A slow emergency trickle,
    // only once the tank is truly empty, gives back enough for a short dash.
    if !stations.sites.is_empty() && ran_dry {
        *emergency = true;
    }
    if *emergency {
        if fuel_state.current_fuel < EMERGENCY_RESERVE {
            fuel_state.current_fuel = (fuel_state.current_fuel + 1.0 * dt).min(EMERGENCY_RESERVE);
        } else {
            *emergency = false;
        }
    }

    let fuel_pct = if fuel_state.max_fuel > 0.0 {
        fuel_state.current_fuel / fuel_state.max_fuel
    } else {
        1.0
    };

    // Warning at 25%
    if fuel_pct <= 0.25 && fuel_pct > 0.10 && !*warned_25 {
        *warned_25 = true;
        notifications.write(ShowNotification {
            message: "Fuel at 25%! Consider conserving engine power.".into(),
            notification_type: NotificationType::Warning,
            duration: 3.0,
        });
    }
    if fuel_pct > 0.30 {
        *warned_25 = false;
    }

    // Warning at 10%
    if fuel_pct <= 0.10 && fuel_pct > 0.0 && !*warned_10 {
        *warned_10 = true;
        notifications.write(ShowNotification {
            message: "FUEL CRITICAL (10%)! Engines will shut down soon!".into(),
            notification_type: NotificationType::Danger,
            duration: 4.0,
        });
    }
    if fuel_pct > 0.15 {
        *warned_10 = false;
    }

    // Deactivate the player's engines when fuel runs out -- and light them
    // again when there is fuel. The relight was missing: the only thing that
    // ever set an engine active again was the station's Repair Modules, which
    // does nothing on an undamaged ship, so a tank run dry once left the ship
    // unable to thrust for good, refuelled or not.
    if ran_dry {
        let mut shut = false;
        for (entity, _engine, mut module, parent, _) in engine_query.iter_mut() {
            if parent.parent() != player_ship { continue; }
            if module.is_active {
                module.is_active = false;
                commands.entity(entity).try_insert(FuelStarved);
                shut = true;
            }
        }
        if shut {
            notifications.write(ShowNotification {
                message: "Engines shut down - no fuel remaining!".into(),
                notification_type: NotificationType::Danger,
                duration: 4.0,
            });
        }
    } else if !*emergency {
        // Not while the emergency reserve is still filling: lit engines idle
        // at 12% draw and would burn the trickle straight back to empty,
        // flickering between "shut down" and "relit". They relight once the
        // reserve is in, or as soon as the ship is properly refuelled.
        let mut relit = false;
        for (entity, _engine, mut module, parent, starved) in engine_query.iter_mut() {
            if !starved || parent.parent() != player_ship { continue; }
            commands.entity(entity).remove::<FuelStarved>();
            if module.health > 0.0 {
                module.is_active = true;
                relit = true;
            }
        }
        if relit {
            notifications.write(ShowNotification {
                message: "Fuel in the lines - engines relit.".into(),
                notification_type: NotificationType::Success,
                duration: 3.0,
            });
        }
    }
}

#[cfg(test)]
mod fuel_relight_tests {
    use super::*;
    use std::time::Duration;

    fn app(with_station: bool) -> (App, Entity) {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.insert_resource(FuelState { current_fuel: 0.0, ..FuelState::default() });
        let mut stations = crate::world::home_base::SystemStations::default();
        if with_station {
            stations.sites.push(crate::world::home_base::StationSite {
                index: 0,
                system_id: 0,
                pos: Vec2::new(90_000.0, 0.0),
                name: "Haven Station".into(),
                kind: crate::world::station_types::station_type(0),
            });
        }
        app.insert_resource(stations);
        app.insert_resource(crate::debug::DebugTuning::default());
        app.add_message::<ShowNotification>();
        app.add_systems(Update, update_fuel_consumption);
        let ship = app.world_mut().spawn((Ship, ShipPhysics::default())).id();
        let engine = app.world_mut().spawn((
            Engine { thrust: 100.0, fuel_consumption: 1.0, noise_level: 0.0 },
            Module {
                module_type: ModuleType::StandardEngine,
                health: 100.0,
                max_health: 100.0,
                power_consumption: 0.0,
                power_generation: 0.0,
                is_active: true,
                grid_position: IVec2::ZERO,
                size: IVec2::ONE,
                rotation: Rotation::default(),
            },
            ChildOf(ship),
        )).id();
        (app, engine)
    }

    fn step(app: &mut App, secs: f32) {
        app.world_mut().resource_mut::<Time>().advance_by(Duration::from_secs_f32(secs));
        app.update();
    }

    fn lit(app: &App, engine: Entity) -> bool {
        app.world().get::<Module>(engine).unwrap().is_active
    }

    /// A tank run dry and then refilled must leave a ship that can thrust.
    #[test]
    fn engines_relight_when_fuel_comes_back() {
        let (mut app, engine) = app(false);
        step(&mut app, 0.1);
        assert!(!lit(&app, engine), "engine still lit on an empty tank");
        app.world_mut().resource_mut::<FuelState>().current_fuel = 260.0; // docking top-up
        step(&mut app, 0.1);
        step(&mut app, 0.1);
        assert!(lit(&app, engine), "refuelled ship still can't thrust");
    }

    /// Dry inside a system: the reserve trickles in with the engines dark,
    /// then they relight - no flickering in between.
    #[test]
    fn a_dry_tank_in_a_system_comes_back_to_a_short_hop() {
        let (mut app, engine) = app(true);
        step(&mut app, 0.1);
        assert!(!lit(&app, engine));
        for _ in 0..20 {
            step(&mut app, 1.0);
            assert!(!lit(&app, engine), "relit before the reserve was in");
        }
        for _ in 0..15 {
            step(&mut app, 1.0);
        }
        assert!(lit(&app, engine), "never relit");
        assert!(app.world().resource::<FuelState>().current_fuel > 20.0);
    }
}

#[cfg(test)]
mod settle_tests {
    use super::*;

    #[test]
    fn hands_off_the_stick_settles() {
        assert!(settles(Vec2::ZERO, false, 500.0));
        assert!(!settles(Vec2::Y, false, 500.0));
        assert!(!settles(Vec2::ZERO, true, 500.0), "the brake does its own work");
    }

    /// Thrust held on a dead drive still lets the ship settle.
    #[test]
    fn a_dead_drive_settles_whatever_is_held() {
        assert!(settles(Vec2::Y, false, 0.0));
        assert!(settles(Vec2::Y, true, 0.0));
    }
}

#[cfg(test)]
mod turn_scale_tests {
    use super::turn_scale;
    use bevy::prelude::IVec2;

    fn block(w: i32, h: i32, x0: i32) -> Vec<IVec2> {
        (0..w).flat_map(|x| (0..h).map(move |y| IVec2::new(x0 + x, y))).collect()
    }

    #[test]
    fn a_starter_sized_hull_turns_at_the_full_rate() {
        assert_eq!(turn_scale(block(28, 30, -14).into_iter()), 1.0);
        assert_eq!(turn_scale(block(14, 15, -7).into_iter()), 1.0, "nothing turns faster");
    }

    #[test]
    fn a_long_destroyer_turns_at_well_under_half() {
        let s = turn_scale(block(114, 28, -56).into_iter());
        assert!((0.3..0.5).contains(&s), "{s}");
    }

    #[test]
    fn where_the_grid_starts_doesnt_change_how_it_turns() {
        let a = turn_scale(block(57, 14, -28).into_iter());
        let b = turn_scale(block(57, 14, 0).into_iter());
        assert!((a - b).abs() < 1e-4);
    }
}
