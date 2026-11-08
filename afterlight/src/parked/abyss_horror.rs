use bevy::prelude::*;
use rand::Rng;

use crate::camera::{camera_shake_update, update_background_color, CameraState};
use crate::components::*;
use crate::creatures::ecosystem::ecosystem_ai_decisions;
use crate::events::{NotificationType, ShowNotification};
use crate::resources::*;
use crate::radar::{update_radar, RadarBlip, RadarDisplay};
use crate::states::GameState;
use crate::ui::{update_hud, update_hud_secondary, DepthText, HullText, NoiseText, OxygenText, PowerText};

// ============================================================================
// RESOURCE
// ============================================================================

#[derive(Resource)]
pub struct AbyssalHorror {
    pub intensity: f32,
    pub phase: u8,
    // Effect timers
    pub instrument_glitch_timer: Timer,
    pub phantom_blip_timer: Timer,
    pub watching_check_timer: Timer,
    pub sync_flee_timer: Timer,
    pub time_glitch_timer: Timer,
    pub notification_timer: Timer,
    pub camera_pulse_timer: Timer,
    // Physics distortion
    pub drift_direction: Vec2,
    pub drift_strength: f32,
    pub gravity_distortion: f32,
    // Glitch state
    pub glitch_active: bool,
    pub glitch_remaining: f32,
    // Time glitch state
    pub time_frozen: bool,
}

impl Default for AbyssalHorror {
    fn default() -> Self {
        Self {
            intensity: 0.0,
            phase: 0,
            instrument_glitch_timer: Timer::from_seconds(8.0, TimerMode::Repeating),
            phantom_blip_timer: Timer::from_seconds(12.0, TimerMode::Repeating),
            watching_check_timer: Timer::from_seconds(5.0, TimerMode::Repeating),
            sync_flee_timer: Timer::from_seconds(45.0, TimerMode::Repeating),
            time_glitch_timer: Timer::from_seconds(60.0, TimerMode::Repeating),
            notification_timer: Timer::from_seconds(20.0, TimerMode::Repeating),
            camera_pulse_timer: Timer::from_seconds(3.0, TimerMode::Repeating),
            drift_direction: Vec2::ZERO,
            drift_strength: 0.0,
            gravity_distortion: 1.0,
            glitch_active: false,
            glitch_remaining: 0.0,
            time_frozen: false,
        }
    }
}

// ============================================================================
// PLUGIN
// ============================================================================

pub struct AbyssHorrorPlugin;

impl Plugin for AbyssHorrorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AbyssalHorror>()
            .add_systems(
                Update,
                (
                    update_horror_intensity,
                    horror_creature_watching,
                    enforce_abyssal_watching.after(ecosystem_ai_decisions),
                    horror_synchronized_flee,
                    horror_time_glitch,
                    restore_time_glitch,
                    restore_creature_behavior,
                    spawn_phantom_radar_blips.after(update_radar),
                    update_phantom_blips,
                    horror_instrument_glitch
                        .after(update_hud)
                        .after(update_hud_secondary),
                    apply_physics_distortion,
                    apply_horror_camera_effects.after(camera_shake_update),
                    apply_horror_background_tint.after(update_background_color),
                    horror_notifications,
                )
                    .run_if(in_state(GameState::Exploring)),
            );
    }
}

// ============================================================================
// SYSTEM 1: UPDATE HORROR INTENSITY
// ============================================================================

fn update_horror_intensity(
    time: Res<Time>,
    depth_state: Res<DepthState>,
    mut horror: ResMut<AbyssalHorror>,
) {
    let depth = depth_state.current_depth;

    // Calculate phase and intensity from distance — rescaled for space
    // distances (old submarine thresholds made the HUD start glitching
    // "ERR"/"Time feels wrong" a few seconds from the starting station).
    // Horror now begins past the Asteroid Belt ring and peaks in the
    // Black Hole ring, matching the zone progression.
    let (phase, intensity) = if depth < 8000.0 {
        (0, 0.0)
    } else if depth < 15000.0 {
        (1, (depth - 8000.0) / 7000.0 * 0.25)
    } else if depth < 22000.0 {
        (2, 0.25 + (depth - 15000.0) / 7000.0 * 0.25)
    } else if depth < 30000.0 {
        (3, 0.5 + (depth - 22000.0) / 8000.0 * 0.25)
    } else {
        (4, (0.75 + (depth - 30000.0) / 15000.0 * 0.25).min(1.0))
    };

    horror.intensity = intensity;
    horror.phase = phase;

    // Adjust timer intervals based on intensity (more intense = more frequent)
    if phase >= 1 {
        let glitch_interval = 8.0 - intensity * 6.0; // 8s → 2s
        horror.instrument_glitch_timer.set_duration(std::time::Duration::from_secs_f32(glitch_interval.max(1.5)));

        let blip_interval = 12.0 - intensity * 9.0; // 12s → 3s
        horror.phantom_blip_timer.set_duration(std::time::Duration::from_secs_f32(blip_interval.max(2.0)));

        let notif_interval = 25.0 - intensity * 15.0; // 25s → 10s
        horror.notification_timer.set_duration(std::time::Duration::from_secs_f32(notif_interval.max(8.0)));
    }

    // Update drift direction slowly (rotating sine)
    let t = time.elapsed_secs();
    horror.drift_direction = Vec2::new(
        (t * 0.1).sin(),
        (t * 0.07).cos(),
    ).normalize_or_zero();

    // Update drift strength based on phase
    horror.drift_strength = match phase {
        0 | 1 => 0.0,
        2 => 3.0 + intensity * 8.0,
        3 => 8.0 + intensity * 8.0,
        _ => 12.0 + intensity * 6.0,
    };

    // Gravity distortion at phase 4
    horror.gravity_distortion = if phase >= 4 {
        let osc = (t * 0.3).sin();
        // Oscillates between 0.7 and -0.3
        0.2 + osc * 0.5
    } else if phase >= 3 {
        0.85 + (t * 0.2).sin() * 0.15 // subtle wobble
    } else {
        1.0
    };

    // Tick glitch remaining time
    if horror.glitch_active {
        horror.glitch_remaining -= time.delta_secs();
        if horror.glitch_remaining <= 0.0 {
            horror.glitch_active = false;
        }
    }
}

// ============================================================================
// SYSTEM 2: HORROR CREATURE WATCHING
// ============================================================================

fn horror_creature_watching(
    time: Res<Time>,
    mut horror: ResMut<AbyssalHorror>,
    ship_query: Query<Entity, With<Ship>>,
    mut creatures: Query<(Entity, &Creature, &mut CreatureAI, Option<&AbyssalInfluence>)>,
    mut commands: Commands,
) {
    if horror.phase < 2 { return; }

    horror.watching_check_timer.tick(time.delta());
    if !horror.watching_check_timer.just_finished() { return; }

    let Ok(ship_entity) = ship_query.single() else { return; };
    let mut rng = rand::thread_rng();

    for (entity, creature, mut ai, influence) in creatures.iter_mut() {
        // Skip if already watching
        if influence.is_some() { continue; }

        // Leviathans only at phase 3+
        if creature.creature_type == CreatureType::Leviathan && horror.phase < 3 {
            continue;
        }

        // Probability scales with intensity
        let prob = horror.intensity * 0.8;
        if rng.gen::<f32>() > prob { continue; }

        // Save original state and switch to watching
        let original_state = ai.state;
        ai.state = CreatureAIState::Observing;
        ai.target = Some(EcoTarget::Ship(ship_entity));

        commands.entity(entity).insert(AbyssalInfluence {
            watching: true,
            original_state,
        });
    }
}

// ============================================================================
// SYSTEM 3: ENFORCE ABYSSAL WATCHING
// ============================================================================

fn enforce_abyssal_watching(
    horror: Res<AbyssalHorror>,
    ship_query: Query<Entity, With<Ship>>,
    mut creatures: Query<(&mut CreatureAI, &AbyssalInfluence)>,
) {
    if horror.phase < 2 { return; }

    let Ok(ship_entity) = ship_query.single() else { return; };

    for (mut ai, influence) in creatures.iter_mut() {
        if influence.watching {
            ai.state = CreatureAIState::Observing;
            ai.target = Some(EcoTarget::Ship(ship_entity));
        }
    }
}

// ============================================================================
// SYSTEM 4: SYNCHRONIZED FLEE
// ============================================================================

fn horror_synchronized_flee(
    time: Res<Time>,
    mut horror: ResMut<AbyssalHorror>,
    mut creatures: Query<(Entity, &Transform, &mut CreatureAI, Option<&AbyssalInfluence>)>,
    mut commands: Commands,
    mut camera_state: ResMut<CameraState>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if horror.phase < 3 { return; }

    horror.sync_flee_timer.tick(time.delta());
    if !horror.sync_flee_timer.just_finished() { return; }

    let mut rng = rand::thread_rng();

    // Flee direction: mostly upward and away from "something below"
    let flee_dir = Vec2::new(
        rng.gen_range(-0.3..0.3),
        rng.gen_range(0.5..1.0),
    ).normalize_or_zero();

    let duration = rng.gen_range(5.0..8.0);

    let mut affected = 0;
    for (entity, transform, mut ai, influence) in creatures.iter_mut() {
        // Remove watching state temporarily
        if influence.is_some() {
            commands.entity(entity).remove::<AbyssalInfluence>();
        }

        let pos = transform.translation.truncate();
        let flee_target = pos + flee_dir * 600.0;

        ai.state = CreatureAIState::Fleeing;
        ai.target = Some(EcoTarget::Position(flee_target));

        commands.entity(entity).insert(SynchronizedFlee {
            flee_direction: flee_dir,
            duration: Timer::from_seconds(duration, TimerMode::Once),
        });

        affected += 1;
    }

    if affected > 0 {
        camera_state.shake_intensity = (camera_state.shake_intensity + 3.0).min(20.0);

        let messages = [
            "Something moved in the deep...",
            "The creatures flee as one...",
            "They sense something below...",
            "A presence stirs beneath you...",
        ];
        notifications.write(ShowNotification {
            message: messages[rng.gen_range(0..messages.len())].to_string(),
            notification_type: NotificationType::Danger,
            duration: 4.0,
        });
    }
}

// ============================================================================
// SYSTEM 5: TIME GLITCH (Phase 4)
// ============================================================================

fn horror_time_glitch(
    time: Res<Time>,
    mut horror: ResMut<AbyssalHorror>,
    mut creatures: Query<(Entity, &mut Velocity), (With<Creature>, Without<Ship>)>,
    mut ship_query: Query<(Entity, &mut Velocity), With<Ship>>,
    mut commands: Commands,
    mut camera_state: ResMut<CameraState>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if horror.phase < 4 { return; }
    if horror.time_frozen { return; }

    horror.time_glitch_timer.tick(time.delta());
    if !horror.time_glitch_timer.just_finished() { return; }

    let mut rng = rand::thread_rng();
    let freeze_duration = rng.gen_range(0.5..2.0);

    // Freeze all creatures
    for (entity, mut velocity) in creatures.iter_mut() {
        let saved = velocity.0;
        velocity.0 = Vec2::ZERO;
        commands.entity(entity).insert(TimeGlitchFrozen {
            duration: Timer::from_seconds(freeze_duration, TimerMode::Once),
            saved_velocity: saved,
        });
    }

    // Freeze ship
    if let Ok((entity, mut velocity)) = ship_query.single_mut() {
        let saved = velocity.0;
        velocity.0 = Vec2::ZERO;
        commands.entity(entity).insert(TimeGlitchFrozen {
            duration: Timer::from_seconds(freeze_duration, TimerMode::Once),
            saved_velocity: saved,
        });
    }

    horror.time_frozen = true;

    // Camera burst
    camera_state.shake_intensity = (camera_state.shake_intensity + 5.0).min(20.0);

    let messages = [
        "Time... skipped.",
        "A moment was lost.",
        "Reality stuttered.",
        "The deep holds time still.",
    ];
    notifications.write(ShowNotification {
        message: messages[rng.gen_range(0..messages.len())].to_string(),
        notification_type: NotificationType::Danger,
        duration: 3.0,
    });
}

// ============================================================================
// SYSTEM 5b: RESTORE FROM TIME GLITCH
// ============================================================================

fn restore_time_glitch(
    time: Res<Time>,
    mut horror: ResMut<AbyssalHorror>,
    mut frozen_query: Query<(Entity, &mut Velocity, &mut TimeGlitchFrozen)>,
    mut commands: Commands,
) {
    if !horror.time_frozen { return; }

    let mut any_frozen = false;
    for (entity, mut velocity, mut frozen) in frozen_query.iter_mut() {
        frozen.duration.tick(time.delta());
        if frozen.duration.is_finished() {
            velocity.0 = frozen.saved_velocity;
            commands.entity(entity).remove::<TimeGlitchFrozen>();
        } else {
            any_frozen = true;
        }
    }

    if !any_frozen {
        horror.time_frozen = false;
    }
}

// ============================================================================
// SYSTEM 6: RESTORE CREATURE BEHAVIOR (on ascending)
// ============================================================================

fn restore_creature_behavior(
    time: Res<Time>,
    horror: Res<AbyssalHorror>,
    mut creatures: Query<(Entity, &mut CreatureAI, &AbyssalInfluence)>,
    mut sync_creatures: Query<(Entity, &mut SynchronizedFlee, &mut CreatureAI), Without<AbyssalInfluence>>,
    mut commands: Commands,
) {
    // Restore watched creatures when ascending above phase 2
    if horror.phase < 2 {
        for (entity, mut ai, influence) in creatures.iter_mut() {
            ai.state = influence.original_state;
            commands.entity(entity).remove::<AbyssalInfluence>();
        }
    }

    // Tick synchronized flee durations and restore
    for (entity, mut sync, mut ai) in sync_creatures.iter_mut() {
        sync.duration.tick(time.delta());
        if sync.duration.is_finished() {
            ai.state = CreatureAIState::Wandering;
            ai.target = None;
            commands.entity(entity).remove::<SynchronizedFlee>();
        }
    }
}

// ============================================================================
// SYSTEM 7: SPAWN PHANTOM SONAR BLIPS
// ============================================================================

fn spawn_phantom_radar_blips(
    time: Res<Time>,
    mut horror: ResMut<AbyssalHorror>,
    radar_query: Query<Entity, With<RadarDisplay>>,
    mut commands: Commands,
) {
    if horror.phase < 1 { return; }

    horror.phantom_blip_timer.tick(time.delta());
    if !horror.phantom_blip_timer.just_finished() { return; }

    let Ok(radar_entity) = radar_query.single() else { return; };

    let mut rng = rand::thread_rng();

    // Number of phantom blips scales with phase
    let blip_count = match horror.phase {
        1 => 1,
        2 => rng.gen_range(1..=3),
        3 => rng.gen_range(2..=5),
        _ => rng.gen_range(3..=7),
    };

    let radar_size = 80.0; // half the 160px radar display

    for i in 0..blip_count {
        // Position on radar
        let (x, y) = if horror.phase >= 3 && i > 0 {
            // Geometric patterns at phase 3+: ring or triangle
            let angle = std::f32::consts::TAU * (i as f32) / (blip_count as f32);
            let radius = rng.gen_range(30.0..70.0);
            (angle.cos() * radius, angle.sin() * radius)
        } else {
            // Random positions, biased toward edges at phase 1
            let radius = if horror.phase == 1 {
                rng.gen_range(55.0..radar_size)
            } else {
                rng.gen_range(20.0..radar_size)
            };
            let angle = rng.gen_range(0.0..std::f32::consts::TAU);
            (angle.cos() * radius, angle.sin() * radius)
        };

        // Color based on phase — phase 2+ can show fake red (leviathan-sized) blips
        let (color, size) = if horror.phase >= 2 && rng.gen::<f32>() < 0.3 {
            (Color::srgba(1.0, 0.2, 0.2, 0.8), 7.0) // fake leviathan blip
        } else {
            (Color::srgba(1.0, 1.0, 0.3, 0.7), 4.0) // normal-looking blip
        };

        let lifetime = rng.gen_range(3.0..6.0);
        let drift = Vec2::new(
            rng.gen_range(-5.0..5.0),
            rng.gen_range(-5.0..5.0),
        );

        let blip_entity = commands
            .spawn((
                (Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(radar_size + x - size / 2.0),
                        top: Val::Px(radar_size - y - size / 2.0),
                        width: Val::Px(size),
                        height: Val::Px(size),
                        ..default()
                    }, BackgroundColor(color)),
                PhantomBlip {
                    lifetime: Timer::from_seconds(lifetime, TimerMode::Once),
                    drift,
                },
                RadarBlip {
                    lifetime: Timer::from_seconds(lifetime, TimerMode::Once),
                },
            ))
            .id();

        commands.entity(radar_entity).add_child(blip_entity);
    }
}

// ============================================================================
// SYSTEM 8: UPDATE PHANTOM BLIPS
// ============================================================================

fn update_phantom_blips(
    time: Res<Time>,
    horror: Res<AbyssalHorror>,
    mut blips: Query<(Entity, &mut PhantomBlip, &mut Node)>,
    mut commands: Commands,
) {
    for (entity, mut phantom, mut style) in blips.iter_mut() {
        phantom.lifetime.tick(time.delta());

        if phantom.lifetime.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }

        // Apply drift
        let dt = time.delta_secs();
        if let Val::Px(ref mut left) = style.left {
            *left += phantom.drift.x * dt;
        }
        if let Val::Px(ref mut top) = style.top {
            *top -= phantom.drift.y * dt;
        }

        // Phase 3+ pulse effect (scale oscillation via width/height)
        if horror.phase >= 3 {
            let t = time.elapsed_secs();
            let pulse = 1.0 + (t * 4.0).sin() * 0.3;
            let base_size = 4.0;
            style.width = Val::Px(base_size * pulse);
            style.height = Val::Px(base_size * pulse);
        }
    }
}

// ============================================================================
// SYSTEM 9: INSTRUMENT GLITCH
// ============================================================================

fn horror_instrument_glitch(
    time: Res<Time>,
    mut horror: ResMut<AbyssalHorror>,
    mut depth_query: Query<(&mut Text, &mut TextColor), (With<DepthText>, Without<PowerText>, Without<OxygenText>, Without<HullText>, Without<NoiseText>)>,
    mut hull_query: Query<(&mut Text, &mut TextColor), (With<HullText>, Without<DepthText>, Without<PowerText>, Without<OxygenText>, Without<NoiseText>)>,
    mut oxygen_query: Query<(&mut Text, &mut TextColor), (With<OxygenText>, Without<DepthText>, Without<PowerText>, Without<HullText>, Without<NoiseText>)>,
    mut power_query: Query<(&mut Text, &mut TextColor), (With<PowerText>, Without<DepthText>, Without<OxygenText>, Without<HullText>, Without<NoiseText>)>,
    mut noise_query: Query<(&mut Text, &mut TextColor), (With<NoiseText>, Without<DepthText>, Without<PowerText>, Without<OxygenText>, Without<HullText>)>,
) {
    if horror.phase < 1 { return; }

    // If a glitch is currently active, keep corrupting
    if horror.glitch_active {
        let mut rng = rand::thread_rng();

        match horror.phase {
            1 => {
                // Subtle: depth flickers
                if let Ok((mut text, _)) = depth_query.single_mut() {
                    // Must match the real readout's format (km) or the glitch
                    // reads as a UI bug instead of a haunted instrument.
                    let fake_depth = rng.gen_range(100.0..3000.0);
                    text.0 = crate::ui::format_range_km(fake_depth);
                }
            }
            2 => {
                // Moderate: oxygen false alarm, depth jumps
                if let Ok((mut text, mut text_color)) = oxygen_query.single_mut() {
                    if rng.gen::<f32>() < 0.5 {
                        text.0 = "O2: 0%".to_string();
                        text_color.0 = Color::srgb(1.0, 0.0, 0.0);
                    }
                }
                if let Ok((mut text, _)) = depth_query.single_mut() {
                    let fake = rng.gen_range(0.0..5000.0);
                    text.0 = crate::ui::format_range_km(fake);
                }
                if let Ok((mut text, mut text_color)) = noise_query.single_mut() {
                    text.0 = "Noise: MAX".to_string();
                    text_color.0 = Color::srgb(1.0, 0.0, 0.0);
                }
            }
            _ => {
                // Severe: all instruments scramble
                let glitch_strings = ["???", "ERR", "---", "NaN", "∞", "0.0̸̡"];
                if let Ok((mut text, mut text_color)) = depth_query.single_mut() {
                    text.0 = glitch_strings[rng.gen_range(0..glitch_strings.len())].to_string();
                    text_color.0 = Color::srgba(1.0, 0.3, 0.3, 0.8);
                }
                if let Ok((mut text, mut text_color)) = hull_query.single_mut() {
                    text.0 = format!("Hull: {}", glitch_strings[rng.gen_range(0..glitch_strings.len())]);
                    text_color.0 = Color::srgba(1.0, 0.3, 0.3, 0.8);
                }
                if let Ok((mut text, mut text_color)) = oxygen_query.single_mut() {
                    text.0 = format!("O2: {}", glitch_strings[rng.gen_range(0..glitch_strings.len())]);
                    text_color.0 = Color::srgba(1.0, 0.3, 0.3, 0.8);
                }
                if let Ok((mut text, mut text_color)) = power_query.single_mut() {
                    text.0 = format!("Power: {}", glitch_strings[rng.gen_range(0..glitch_strings.len())]);
                    text_color.0 = Color::srgba(1.0, 0.3, 0.3, 0.8);
                }
                if let Ok((mut text, mut text_color)) = noise_query.single_mut() {
                    text.0 = format!("Noise: {}", glitch_strings[rng.gen_range(0..glitch_strings.len())]);
                    text_color.0 = Color::srgba(1.0, 0.3, 0.3, 0.8);
                }
            }
        }
        return;
    }

    // Check if it's time to trigger a new glitch
    horror.instrument_glitch_timer.tick(time.delta());
    if !horror.instrument_glitch_timer.just_finished() { return; }

    let mut rng = rand::thread_rng();

    // Glitch duration scales with phase
    let duration = match horror.phase {
        1 => rng.gen_range(0.1..0.3),    // single-frame flicker
        2 => rng.gen_range(0.3..0.8),    // noticeable
        3 => rng.gen_range(0.8..1.5),    // alarming
        _ => rng.gen_range(1.0..2.5),    // prolonged
    };

    horror.glitch_active = true;
    horror.glitch_remaining = duration;
}

// ============================================================================
// SYSTEM 10: PHYSICS DISTORTION
// ============================================================================

fn apply_physics_distortion(
    time: Res<Time>,
    horror: Res<AbyssalHorror>,
    mut ship_query: Query<&mut Velocity, With<Ship>>,
) {
    if horror.phase < 2 { return; }

    let Ok(mut velocity) = ship_query.single_mut() else { return; };
    let dt = time.delta_secs();

    // Apply phantom drift
    velocity.0 += horror.drift_direction * horror.drift_strength * dt;
}

// ============================================================================
// SYSTEM 11: HORROR CAMERA EFFECTS
// ============================================================================

fn apply_horror_camera_effects(
    time: Res<Time>,
    horror: Res<AbyssalHorror>,
    mut camera_state: ResMut<CameraState>,
) {
    if horror.phase < 1 { return; }

    let t = time.elapsed_secs();

    match horror.phase {
        1 => {
            // Occasional micro-shake
            if (t * 0.5).sin() > 0.95 {
                camera_state.shake_intensity = camera_state.shake_intensity.max(0.5);
            }
        }
        2 => {
            // Slow zoom drift
            let zoom_offset = (t * 0.3).sin() * 0.05;
            camera_state.zoom = (camera_state.zoom + zoom_offset * time.delta_secs()).clamp(
                camera_state.min_zoom,
                camera_state.max_zoom,
            );
            // Occasional shake
            if (t * 0.3).sin() > 0.9 {
                camera_state.shake_intensity = camera_state.shake_intensity.max(1.0);
            }
        }
        3 => {
            // Heartbeat pulse
            horror.camera_pulse_timer.duration(); // just reference to keep consistent
            let pulse = ((t * 1.5).sin() * 0.5 + 0.5).powf(4.0); // sharp pulses
            camera_state.zoom = (camera_state.zoom - pulse * 0.03 * time.delta_secs()).clamp(
                camera_state.min_zoom,
                camera_state.max_zoom,
            );
            // More frequent shake
            if (t * 0.4).sin() > 0.8 {
                camera_state.shake_intensity = camera_state.shake_intensity.max(1.5);
            }
        }
        _ => {
            // Phase 4: sustained instability
            camera_state.shake_intensity = camera_state.shake_intensity.max(1.0 + horror.intensity);

            let zoom_chaos = (t * 0.7).sin() * 0.03 + (t * 1.3).cos() * 0.02;
            camera_state.zoom = (camera_state.zoom + zoom_chaos * time.delta_secs()).clamp(
                camera_state.min_zoom,
                camera_state.max_zoom,
            );
        }
    }
}

// ============================================================================
// SYSTEM 12: HORROR BACKGROUND TINT
// ============================================================================

fn apply_horror_background_tint(
    time: Res<Time>,
    horror: Res<AbyssalHorror>,
    mut clear_color: ResMut<ClearColor>,
) {
    if horror.phase < 2 { return; }

    let t = time.elapsed_secs();
    let color = &mut clear_color.0;

    match horror.phase {
        2 => {
            // Slight sickly green/purple tint
            let r = (color.to_srgba().red - 0.005).max(0.0);
            let g = color.to_srgba().green;
            let b = (color.to_srgba().blue + 0.008).min(0.15);
            *color = Color::srgb(r, g, b);
        }
        3 => {
            // Pulsing dark red undertone
            let pulse = ((t * 0.5).sin() * 0.5 + 0.5) * 0.02;
            let r = (color.to_srgba().red + pulse).min(0.08);
            let g = (color.to_srgba().green - 0.002).max(0.0);
            let b = color.to_srgba().blue;
            *color = Color::srgb(r, g, b);
        }
        _ => {
            // Phase 4: occasional crimson flash
            let flash = ((t * 0.2).sin() * 0.5 + 0.5).powf(8.0);
            let r = (color.to_srgba().red + flash * 0.05).min(0.1);
            let g = (color.to_srgba().green * 0.98).max(0.0);
            let b = (color.to_srgba().blue * 0.98).max(0.0);
            *color = Color::srgb(r, g, b);
        }
    }
}

// ============================================================================
// SYSTEM 13: HORROR NOTIFICATIONS
// ============================================================================

fn horror_notifications(
    time: Res<Time>,
    mut horror: ResMut<AbyssalHorror>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if horror.phase < 1 { return; }

    horror.notification_timer.tick(time.delta());
    if !horror.notification_timer.just_finished() { return; }

    let mut rng = rand::thread_rng();

    let messages: &[&str] = match horror.phase {
        1 => &[
            "The radar seems... off.",
            "Was that shadow always there?",
            "A faint echo, from nowhere.",
            "The pressure feels wrong.",
        ],
        2 => &[
            "The creatures have stopped moving.",
            "Something is watching.",
            "Your instruments flicker.",
            "The silence is deafening.",
            "They've noticed you.",
        ],
        3 => &[
            "They're all looking at you.",
            "The void is listening.",
            "Even the predators are afraid.",
            "Nothing attacks. Everything watches.",
            "You feel observed from below.",
        ],
        _ => &[
            "Time feels wrong.",
            "There is something below.",
            "You were not meant to be here.",
            "It knows you're here.",
            "The abyss stares back.",
            "Reality bends around you.",
        ],
    };

    let message = messages[rng.gen_range(0..messages.len())];
    let notif_type = if horror.phase >= 3 {
        NotificationType::Danger
    } else {
        NotificationType::Warning
    };

    notifications.write(ShowNotification {
        message: message.to_string(),
        notification_type: notif_type,
        duration: 4.0,
    });
}
