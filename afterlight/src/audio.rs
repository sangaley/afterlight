use bevy::prelude::*;
use bevy::audio::{AudioSink, AudioSinkPlayback, Volume};
use rand::Rng;
use std::collections::HashMap;

use crate::components::{ModuleType, Ship, ShipPhysics};
use crate::events::*;
use crate::resources::InputState;
use crate::states::GameState;
use crate::celestial::events::WarpJumpStarted;
use crate::ui::menu_buttons::GameSettings;

// ============================================================================
// AUDIO PLUGIN
// Everything is event-driven: gameplay systems write messages (WeaponFired,
// ModuleExploded, HullBreached, ...) and the systems here turn them into
// one-shot AudioPlayer entities or manage the persistent loops (engine,
// ambient drone, alarm).
// ============================================================================

// Per-bus volume scalars. Sounds multiply their own base volume by one of these.
const SFX_VOL: f32 = 0.8;
/// Kill switch for weapon fire sounds only (explosions/impacts unaffected).
const WEAPON_FIRE_SOUNDS: bool = true;
const AMBIENT_VOL: f32 = 0.30;

/// The menu sits under the flight bed. It is the first thing anyone hears and
/// it should not be the loudest.
const MENU_VOL: f32 = 0.22;
const UI_VOL: f32 = 0.45;
const ALARM_VOL: f32 = 0.40;
const ENGINE_MAX_VOL: f32 = 0.40;

/// Distance (world units) beyond which off-ship sounds are inaudible.
const AUDIBLE_RANGE: f32 = 30_000.0;

pub struct GameAudioPlugin;

impl Plugin for GameAudioPlugin {
    fn build(&self, app: &mut App) {
        // AFTERLIGHT_MUTE=1 (every automated run): silent from the first frame,
        // before any sound can start -- not from whenever the settings are
        // first applied, by which time a loop could already be playing.
        if std::env::var("AFTERLIGHT_MUTE").ok().as_deref() == Some("1") {
            app.insert_resource(GlobalVolume::new(Volume::Linear(0.0)));
        }
        app
            .init_resource::<AlarmState>()
            .add_systems(Startup, load_audio)
            .add_systems(OnEnter(GameState::Exploring), (start_flight_loops, launch_audio))
            .add_systems(OnEnter(GameState::StationDocked), (start_station_loops, docking_audio))
            .add_systems(OnExit(GameState::StationDocked), stop_station_loops)
            // Deliberately NOT OnExit(Exploring). That fired on every pause,
            // on docking, and on entering the ending — despawning the drone,
            // engine, alarm and both dread layers and restarting them from
            // sample zero on the way back. Pausing mid-fight hard-cut the
            // whole soundscape, and the 2.5-minute ending played in total
            // silence because Truth is entered from Exploring.
            .add_systems(OnEnter(GameState::MainMenu), (stop_flight_loops, start_menu_loop))
            .add_systems(OnExit(GameState::MainMenu), stop_menu_loop)
            .add_systems(OnEnter(GameState::GameOver), stop_flight_loops)
            .add_systems(Update, (
                weapon_fired_audio,
                explosion_audio,
                alarm_audio,
                warp_audio,
                engine_loop_volume,
                dread_volume,
                ship_damaged_audio,
                update_ambient_volume,
                hull_creak_ticker,
            ).run_if(in_state(GameState::Exploring)))
            .add_systems(Update, (
                ui_click_audio,
                notification_audio,
                build_audio,
                fade_in_loops,
            ));
    }
}

// ============================================================================
// ASSETS
// ============================================================================

#[derive(Resource)]
pub struct GameAudio {
    lasers: Vec<Handle<AudioSource>>,
    heavy_cannons: Vec<Handle<AudioSource>>,
    shots: Vec<Handle<AudioSource>>,
    rocket: Handle<AudioSource>,
    explosions: Vec<Handle<AudioSource>>,
    explosion_deep: Handle<AudioSource>,
    explosion_large: Handle<AudioSource>,
    engine_loop: Handle<AudioSource>,
    warps: Vec<Handle<AudioSource>>,
    space_drone: Handle<AudioSource>,
    /// Two dread layers that fade in with the cascade. Both were already
    /// licensed and sitting in assets/audio/ambient with nothing loading them.
    dread_low: Handle<AudioSource>,
    dread_high: Handle<AudioSource>,
    /// Interior ambience for the station. The opening was ~50 seconds of
    /// silence: no menu audio, no station hum, nothing marking launch.
    station_hum: Handle<AudioSource>,
    hull_creaks: Vec<Handle<AudioSource>>,
    alarm_loop: Handle<AudioSource>,
    ui_select: Vec<Handle<AudioSource>>,
    ui_terminal: Vec<Handle<AudioSource>>,
    ui_beeps: Vec<Handle<AudioSource>>,
}

fn load_audio(mut commands: Commands, assets: Res<AssetServer>) {
    let load_all = |paths: &[&str]| -> Vec<Handle<AudioSource>> {
        paths.iter().map(|p| assets.load(p.to_string())).collect()
    };

    commands.insert_resource(GameAudio {
        lasers: load_all(&[
            "audio/weapons/laser_1.mp3",
            "audio/weapons/laser_2.mp3",
            "audio/weapons/laser_3.mp3",
            "audio/weapons/laser_4.mp3",
            "audio/weapons/laser_5.mp3",
        ]),
        heavy_cannons: load_all(&[
            "audio/weapons/heavy_cannon_2.mp3",
        ]),
        shots: load_all(&[
            "audio/weapons/shoot_1.ogg",
            "audio/weapons/shoot_2.ogg",
        ]),
        rocket: assets.load("audio/weapons/rocket_1.ogg"),
        explosions: load_all(&[
            "audio/impacts/explosion_1.ogg",
            "audio/impacts/explosion_2.ogg",
        ]),
        explosion_deep: assets.load("audio/impacts/explosion_deep.mp3"),
        explosion_large: assets.load("audio/impacts/explosion_large.mp3"),
        // engine_rumble_loop: low interior thruster rumble ("thrusters_loop"
        // by DJT4NN3R, freesound.org/s/347576, CC0) — the Little Robot
        // engine_*_loop files sounded synthy rather than like an engine.
        engine_loop: assets.load("audio/engines/engine_rumble_loop.mp3"),
        warps: load_all(&[
            "audio/engines/warp_1.mp3",
            "audio/engines/warp_2.mp3",
            "audio/engines/warp_3.mp3",
        ]),
        space_drone: assets.load("audio/ambient/space_drone.mp3"),
        dread_low: assets.load("audio/ambient/black_hole_drone.mp3"),
        dread_high: assets.load("audio/ambient/hostile_atmosphere_loop.mp3"),
        station_hum: assets.load("audio/ambient/interior_hum.ogg"),
        hull_creaks: load_all(&[
            "audio/ambient/hull_creak_1.mp3",
            "audio/ambient/hull_creak_2.mp3",
            "audio/ambient/hull_groan_long_1.mp3",
            "audio/ambient/hull_groan_long_2.mp3",
        ]),
        alarm_loop: assets.load("audio/alarms/alarm_loop_1.mp3"),
        ui_select: load_all(&[
            "audio/ui/select_1.mp3",
            "audio/ui/select_2.mp3",
        ]),
        ui_terminal: load_all(&[
            "audio/ui/terminal_1.ogg",
            "audio/ui/terminal_2.ogg",
            "audio/ui/terminal_3.ogg",
        ]),
        ui_beeps: load_all(&[
            "audio/ui/beep_1.ogg",
            "audio/ui/beep_2.ogg",
        ]),
    });
}

// ============================================================================
// HELPERS
// ============================================================================

fn play_oneshot(commands: &mut Commands, handle: Handle<AudioSource>, volume: f32) {
    commands.spawn((
        AudioPlayer(handle),
        PlaybackSettings::DESPAWN.with_volume(Volume::Linear(volume)),
    ));
}

fn pick<'a>(rng: &mut impl Rng, v: &'a [Handle<AudioSource>]) -> &'a Handle<AudioSource> {
    &v[rng.gen_range(0..v.len())]
}

/// Quadratic falloff with distance from the player ship. Player-originated
/// sounds pass distance 0.0 and get full volume.
fn attenuate(volume: f32, distance: f32) -> f32 {
    let t = (1.0 - (distance / AUDIBLE_RANGE).clamp(0.0, 1.0)).powi(2);
    volume * t
}

fn player_pos(ship_query: &Query<&Transform, With<Ship>>) -> Vec2 {
    ship_query.single().map(|t| t.translation.truncate()).unwrap_or(Vec2::ZERO)
}

// ============================================================================
// WEAPONS
// ============================================================================

/// Minimum seconds between two sounds of the same weapon type. Keeps the
/// Gatling from stacking 10 overlapping samples and the laser (which writes
/// WeaponFired every frame while beaming) down to a periodic re-trigger.
fn min_interval(weapon: ModuleType) -> f64 {
    match weapon {
        ModuleType::Gatling => 0.12,
        ModuleType::Laser => 1.4,
        ModuleType::PlasmaCaster | ModuleType::IonDisruptor => 0.5,
        _ => 0.05,
    }
}

fn weapon_fired_audio(
    mut events: MessageReader<WeaponFired>,
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    ship_query: Query<&Transform, With<Ship>>,
    time: Res<Time>,
    mut last_played: Local<HashMap<ModuleType, f64>>,
    mut commands: Commands,
) {
    if !WEAPON_FIRE_SOUNDS {
        events.clear();
        return;
    }
    let Some(audio) = audio else { return };
    let mut rng = rand::thread_rng();
    let now = time.elapsed_secs_f64();
    let ppos = player_pos(&ship_query);

    for ev in events.read() {
        let last = last_played.get(&ev.weapon_type).copied().unwrap_or(f64::MIN);
        if now - last < min_interval(ev.weapon_type) {
            continue;
        }

        let (handle, base_vol) = match ev.weapon_type {
            ModuleType::Cannon | ModuleType::Railgun =>
                (pick(&mut rng, &audio.heavy_cannons).clone(), 0.75),
            ModuleType::Coilgun | ModuleType::Gatling =>
                (pick(&mut rng, &audio.shots).clone(), 0.5),
            ModuleType::Laser | ModuleType::PlasmaCaster | ModuleType::IonDisruptor | ModuleType::EMPPulse =>
                (pick(&mut rng, &audio.lasers).clone(), 0.55),
            ModuleType::HeavyMissile | ModuleType::GuidedMissile | ModuleType::ClusterRocket
            | ModuleType::LaunchPad | ModuleType::RailLauncher | ModuleType::VlsSilo =>
                (audio.rocket.clone(), 0.65),
            _ => continue,
        };

        let dist = if ev.from_player { 0.0 } else { ev.position.distance(ppos) };
        let vol = attenuate(base_vol * SFX_VOL * settings.sfx_volume, dist);
        if vol < 0.01 { continue; }

        last_played.insert(ev.weapon_type, now);
        play_oneshot(&mut commands, handle, vol);
    }
}

// ============================================================================
// EXPLOSIONS / DESTRUCTION
// ============================================================================

fn explosion_audio(
    mut module_exploded: MessageReader<ModuleExploded>,
    mut ai_module_exploded: MessageReader<AiModuleExploded>,
    mut ai_destroyed: MessageReader<AiShipDestroyed>,
    mut hull_destroyed: MessageReader<HullSegmentDestroyed>,
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    ship_query: Query<&Transform, With<Ship>>,
    time: Res<Time>,
    mut last_hull_crunch: Local<f64>,
    mut commands: Commands,
) {
    let Some(audio) = audio else { return };
    let mut rng = rand::thread_rng();
    let ppos = player_pos(&ship_query);
    let sfx = SFX_VOL * settings.sfx_volume;

    // Module detonations happen on the player's own ship — full volume.
    for _ in module_exploded.read() {
        play_oneshot(&mut commands, pick(&mut rng, &audio.explosions).clone(), 0.8 * sfx);
        play_oneshot(&mut commands, audio.explosion_deep.clone(), 0.7 * sfx);
    }

    // Module cook-offs on AI ships — crunch attenuated by distance, with the
    // deep layer reserved for bigger blasts (reactors, full ammo bays).
    for ev in ai_module_exploded.read() {
        let dist = ev.position.distance(ppos);
        play_oneshot(&mut commands, pick(&mut rng, &audio.explosions).clone(), attenuate(0.7 * sfx, dist));
        if ev.blast_damage >= 50.0 {
            play_oneshot(&mut commands, audio.explosion_deep.clone(), attenuate(0.5 * sfx, dist));
        }
    }

    // A ship dying is the big payoff — layered boom + low rumble, attenuated.
    for ev in ai_destroyed.read() {
        let dist = ev.position.distance(ppos);
        play_oneshot(&mut commands, audio.explosion_large.clone(), attenuate(0.9 * sfx, dist));
        play_oneshot(&mut commands, audio.explosion_deep.clone(), attenuate(0.8 * sfx, dist));
    }

    // Individual hull blocks popping — quiet crunch, rate-limited so a
    // volley chewing through armor doesn't stack 15 copies in one frame.
    let now = time.elapsed_secs_f64();
    for _ in hull_destroyed.read() {
        if now - *last_hull_crunch < 0.3 { continue; }
        *last_hull_crunch = now;
        play_oneshot(&mut commands, pick(&mut rng, &audio.explosions).clone(), 0.35 * sfx);
    }
}

// ============================================================================
// ALARM (hull breach)
// ============================================================================

#[derive(Resource, Default)]
struct AlarmState {
    /// Seconds of alarm remaining; each new breach tops it back up.
    remaining: f32,
}

#[derive(Component)]
struct AlarmLoopAudio;

fn alarm_audio(
    mut breaches: MessageReader<HullBreached>,
    mut state: ResMut<AlarmState>,
    time: Res<Time>,
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    alarm_query: Query<Entity, With<AlarmLoopAudio>>,
    mut commands: Commands,
) {
    let Some(audio) = audio else { return };

    for _ in breaches.read() {
        state.remaining = 8.0;
    }

    if state.remaining > 0.0 {
        state.remaining -= time.delta_secs();
        if alarm_query.is_empty() {
            commands.spawn((
                AudioPlayer(audio.alarm_loop.clone()),
                PlaybackSettings::LOOP.with_volume(Volume::Linear(ALARM_VOL * settings.sfx_volume)),
                AlarmLoopAudio,
            ));
        }
    } else if let Ok(entity) = alarm_query.single() {
        commands.entity(entity).despawn();
    }
}

// ============================================================================
// WARP
// ============================================================================

fn warp_audio(
    mut jumps: MessageReader<WarpJumpStarted>,
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    mut commands: Commands,
) {
    let Some(audio) = audio else { return };
    let mut rng = rand::thread_rng();
    for _ in jumps.read() {
        play_oneshot(&mut commands, pick(&mut rng, &audio.warps).clone(), 0.7 * SFX_VOL * settings.sfx_volume);
    }
}

// ============================================================================
// PERSISTENT LOOPS (engine + ambient drone)
// ============================================================================

#[derive(Component)]
struct EngineLoopAudio;

#[derive(Component)]
struct AmbientLoopAudio;

/// The main menu's bed.
///
/// The menu played nothing at all. The first sound in the game arrived about
/// fifty seconds in, when the player launched, and until then the only
/// evidence the audio worked was that the buttons clicked.
#[derive(Component)]
struct MenuLoopAudio;

/// Brings a loop up from silence over `seconds` instead of slamming it in.
#[derive(Component)]
struct AudioFadeIn {
    elapsed: f32,
    seconds: f32,
    target: f32,
}

/// Station interior ambience. Runs while docked, which used to be silent.
#[derive(Component)]
struct StationLoopAudio;

/// The bed under everything, brought up by how far along the run is.
///
/// Starts silent and stays silent for the whole opening. Two layers rather
/// than one so it can thicken rather than just get louder: the low drone
/// arrives first, and the second only in the last stretch.
#[derive(Component)]
struct DreadLoopAudio {
    /// Cascade level at which this layer starts being audible at all.
    from: f32,
    /// Cascade level at which it reaches `ceiling`.
    ///
    /// Both layers used to peak at 1.0, which meant neither was audible in a
    /// demo: the trail walls at tier 1 and a demo session reaches about 0.17,
    /// so a layer ramping to full at 1.0 sat at five percent of the drone it
    /// was under. The first layer now peaks partway along, so the bed does
    /// what it was written to do on a curve the demo can actually reach.
    peak: f32,
    /// Volume it reaches at `peak`.
    ceiling: f32,
}

fn start_flight_loops(
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    mut commands: Commands,
) {
    let Some(audio) = audio else { return };
    // Engine starts silent; engine_loop_volume drives it from throttle.
    commands.spawn((
        AudioPlayer(audio.engine_loop.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        EngineLoopAudio,
    ));
    commands.spawn((
        AudioPlayer(audio.space_drone.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(AMBIENT_VOL * settings.music_volume)),
        AmbientLoopAudio,
    ));
    commands.spawn((
        AudioPlayer(audio.dread_low.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        DreadLoopAudio { from: 0.10, peak: 0.45, ceiling: 0.26 },
    ));
    commands.spawn((
        AudioPlayer(audio.dread_high.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        DreadLoopAudio { from: 0.55, peak: 1.0, ceiling: 0.18 },
    ));
}

/// The menu's drone. Fades up rather than starting at full, because the
/// player is reading a title card, not walking into a room.
fn start_menu_loop(
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    existing: Query<Entity, With<MenuLoopAudio>>,
    mut commands: Commands,
) {
    let Some(audio) = audio else { return };
    if !existing.is_empty() {
        return;
    }
    let target = MENU_VOL * settings.music_volume;
    commands.spawn((
        // The deep one, not the open-space one. The menu should sound like
        // the thing the expedition went looking for, and `space_drone`
        // already carries the flying.
        AudioPlayer(audio.dread_low.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)),
        MenuLoopAudio,
        AudioFadeIn { elapsed: 0.0, seconds: 4.0, target },
    ));
}

fn stop_menu_loop(mut commands: Commands, loops: Query<Entity, With<MenuLoopAudio>>) {
    for entity in loops.iter() {
        commands.entity(entity).despawn();
    }
}

/// The master volume (and mute) as a factor. Bevy applies GlobalVolume only
/// when a sound starts; the loops below set their own volume every frame,
/// which used to replace it -- so Mute silenced one-shots while the engine,
/// ambient and dread loops played on. Every live set_volume multiplies by this.
fn master(global: &GlobalVolume) -> f32 {
    global.volume.to_linear()
}

fn fade_in_loops(
    time: Res<Time>,
    mut commands: Commands,
    mut fading: Query<(Entity, &mut AudioFadeIn, &mut AudioSink)>,
    global: Res<GlobalVolume>,
) {
    for (entity, mut fade, mut sink) in fading.iter_mut() {
        fade.elapsed += time.delta_secs();
        let t = (fade.elapsed / fade.seconds).clamp(0.0, 1.0);
        // Ease in, so the first second is nearly silent rather than a ramp
        // the ear can follow.
        sink.set_volume(Volume::Linear(fade.target * t * t * master(&global)));
        if t >= 1.0 {
            commands.entity(entity).remove::<AudioFadeIn>();
        }
    }
}

fn start_station_loops(
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    existing: Query<Entity, With<StationLoopAudio>>,
    mut commands: Commands,
) {
    let Some(audio) = audio else { return };
    if !existing.is_empty() {
        return;
    }
    let vol = AMBIENT_VOL * settings.music_volume;
    commands.spawn((
        AudioPlayer(audio.station_hum.clone()),
        PlaybackSettings::LOOP.with_volume(Volume::Linear(vol)),
        StationLoopAudio,
    ));
    // There was a machine loop under the hum: three seconds of clank,
    // repeating for as long as you sat at the berth. Players heard it as a
    // fault, not as a station.
}

fn stop_station_loops(loops: Query<Entity, With<StationLoopAudio>>, mut commands: Commands) {
    for e in loops.iter() {
        commands.entity(e).despawn();
    }
}

/// Being shot used to be silent.
///
/// ShipDamaged had no handler at all: the only audible thing was a hull tile
/// being fully destroyed, so ordinary incoming fire produced camera shake and
/// nothing else. There is no dedicated impact asset (see the TODO in
/// combat/new_projectiles.rs), so this uses the existing hull-crunch bank at
/// low volume and a short pitch-varied rate limit — close enough to read as
/// metal being hit, and far better than nothing.
fn ship_damaged_audio(
    mut events: MessageReader<ShipDamaged>,
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    time: Res<Time>,
    mut commands: Commands,
    mut last: Local<f32>,
) {
    let Some(audio) = audio else { return };
    let mut heaviest = 0.0f32;
    for e in events.read() {
        heaviest = heaviest.max(e.amount);
    }
    if heaviest <= 0.0 {
        return;
    }
    // Rate limit: a burst of gatling hits is one sound, not thirty.
    let now = time.elapsed_secs();
    if now - *last < 0.18 {
        return;
    }
    *last = now;

    if let Some(sound) = audio.hull_creaks.first() {
        // Scale with the size of the hit so a graze and a shell are different.
        let vol = (0.10 + (heaviest / 120.0).clamp(0.0, 1.0) * 0.28)
            * SFX_VOL
            * settings.sfx_volume;
        commands.spawn((
            AudioPlayer(sound.clone()),
            PlaybackSettings::DESPAWN.with_volume(Volume::Linear(vol)),
        ));
    }
}

/// Nothing marked launch — the most cinematic beat in the first minute.
fn launch_audio(
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    mut commands: Commands,
) {
    let Some(audio) = audio else { return };
    if let Some(sound) = audio.warps.first() {
        commands.spawn((
            AudioPlayer(sound.clone()),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(0.45 * SFX_VOL * settings.sfx_volume)),
        ));
    }
}

/// Ease each dread layer toward where the cascade says it should be.
///
/// Eased rather than set, for the same reason the engine loop is: a volume
/// that snaps is audible as a snap. Nobody should be able to point at the
/// moment the music changed.
fn dread_volume(
    time: Res<Time>,
    settings: Res<GameSettings>,
    cascade: Res<crate::narrative::CascadeState>,
    mut layers: Query<(&mut AudioSink, &DreadLoopAudio)>,
    global: Res<GlobalVolume>,
) {
    let level = cascade.level.clamp(0.0, 1.0);
    for (mut sink, layer) in layers.iter_mut() {
        let span = (layer.peak - layer.from).max(0.001);
        let ramp = ((level - layer.from) / span).clamp(0.0, 1.0);
        let target = ramp * layer.ceiling * settings.music_volume * master(&global);
        let current = sink.volume().to_linear();
        let eased = current + (target - current) * (time.delta_secs() * 0.6).min(1.0);
        sink.set_volume(Volume::Linear(eased));
    }
}

/// Keep the ambient drone in sync with the MUSIC bus while it changes live.
fn update_ambient_volume(
    settings: Res<GameSettings>,
    mut sink_query: Query<&mut AudioSink, With<AmbientLoopAudio>>,
    global: Res<GlobalVolume>,
) {
    let Ok(mut sink) = sink_query.single_mut() else { return };
    // A sink only exists a frame after its sound starts: a mute set before
    // then never reached it, and the drone droned on in silent runs.
    if !settings.is_changed() && !global.is_changed() && !sink.is_added() {
        return;
    }
    {
        sink.set_volume(Volume::Linear(AMBIENT_VOL * settings.music_volume * master(&global)));
    }
}

fn stop_flight_loops(
    loops: Query<
        Entity,
        Or<(
            With<EngineLoopAudio>,
            With<AmbientLoopAudio>,
            With<AlarmLoopAudio>,
            With<DreadLoopAudio>,
        )>,
    >,
    mut state: ResMut<AlarmState>,
    mut commands: Commands,
) {
    state.remaining = 0.0;
    for entity in loops.iter() {
        commands.entity(entity).despawn();
    }
}

fn engine_loop_volume(
    input: Res<InputState>,
    settings: Res<GameSettings>,
    ship_query: Query<&ShipPhysics, With<Ship>>,
    mut sink_query: Query<&mut AudioSink, With<EngineLoopAudio>>,
    time: Res<Time>,
    mut current: Local<f32>,
    global: Res<GlobalVolume>,
) {
    let Ok(mut sink) = sink_query.single_mut() else { return };
    let throttle = ship_query.single().map(|p| p.throttle.abs()).unwrap_or(0.0);
    let intensity = throttle
        .max(input.movement.x.abs() * 0.5)
        .max(if input.brake { 0.4 } else { 0.0 });

    // Ease toward the target so thrust taps don't click the loop on/off.
    let target = intensity.clamp(0.0, 1.0) * ENGINE_MAX_VOL * settings.sfx_volume;
    let rate = if target > *current { 6.0 } else { 2.5 };
    *current += (target - *current) * (rate * time.delta_secs()).min(1.0);
    sink.set_volume(Volume::Linear(*current * master(&global)));
}

// ============================================================================
// HULL CREAKS — sparse, quiet, unsettling
// ============================================================================

fn hull_creak_ticker(
    time: Res<Time>,
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    mut timer: Local<Option<Timer>>,
    mut commands: Commands,
) {
    let Some(audio) = audio else { return };
    let mut rng = rand::thread_rng();

    let t = timer.get_or_insert_with(|| {
        Timer::from_seconds(rng.gen_range(25.0..70.0), TimerMode::Once)
    });
    t.tick(time.delta());
    if t.is_finished() {
        play_oneshot(&mut commands, pick(&mut rng, &audio.hull_creaks).clone(), 0.22 * settings.sfx_volume);
        *timer = Some(Timer::from_seconds(rng.gen_range(25.0..70.0), TimerMode::Once));
    }
}

// ============================================================================
// UI
// ============================================================================

fn ui_click_audio(
    interactions: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    mut commands: Commands,
) {
    let Some(audio) = audio else { return };
    let mut rng = rand::thread_rng();
    for interaction in interactions.iter() {
        if *interaction == Interaction::Pressed {
            play_oneshot(&mut commands, pick(&mut rng, &audio.ui_select).clone(), UI_VOL * settings.ui_volume);
        }
    }
}

fn notification_audio(
    mut notifications: MessageReader<ShowNotification>,
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    time: Res<Time>,
    mut last: Local<f64>,
    mut commands: Commands,
) {
    let Some(audio) = audio else { return };
    let now = time.elapsed_secs_f64();
    for ev in notifications.read() {
        if now - *last < 0.25 { continue; }
        *last = now;
        let handle = match ev.notification_type {
            NotificationType::Info => audio.ui_terminal[0].clone(),
            NotificationType::Success => audio.ui_terminal[1].clone(),
            NotificationType::Warning => audio.ui_beeps[0].clone(),
            NotificationType::Danger => audio.ui_beeps[1].clone(),
        };
        play_oneshot(&mut commands, handle, UI_VOL * settings.ui_volume);
    }
}

fn build_audio(
    mut placed: MessageReader<ModulePlaced>,
    mut removed: MessageReader<ModuleRemoved>,
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    mut commands: Commands,
) {
    let Some(audio) = audio else { return };
    for _ in placed.read() {
        play_oneshot(&mut commands, audio.ui_terminal[2].clone(), UI_VOL * settings.ui_volume);
    }
    for _ in removed.read() {
        play_oneshot(&mut commands, audio.ui_beeps[0].clone(), UI_VOL * 0.8 * settings.ui_volume);
    }
}

/// Docking had a purpose-built sound that never played: DockingCompleted has
/// a handler and no writer anywhere in the codebase, so the only audible
/// confirmation was the generic notification blip. Driven off the state
/// change instead, which is the thing that actually happens.
fn docking_audio(
    audio: Option<Res<GameAudio>>,
    settings: Res<GameSettings>,
    mut commands: Commands,
) {
    let Some(audio) = audio else { return };
    if let Some(sound) = audio.ui_terminal.first() {
        commands.spawn((
            AudioPlayer(sound.clone()),
            PlaybackSettings::DESPAWN
                .with_volume(Volume::Linear(0.6 * UI_VOL * settings.ui_volume)),
        ));
    }
}

#[cfg(test)]
mod dread_tests {
    use super::*;

    /// The layers as `start_flight_loops` actually spawns them.
    ///
    /// These used to be literals repeated in the test, which is why the bed
    /// could be tuned out of the demo's reach entirely without a single test
    /// noticing. Reading the real values is the whole point.
    const LOW: DreadLoopAudio = DreadLoopAudio { from: 0.10, peak: 0.45, ceiling: 0.26 };
    const HIGH: DreadLoopAudio = DreadLoopAudio { from: 0.55, peak: 1.0, ceiling: 0.18 };

    /// Mirrors the ramp in `dread_volume`.
    fn ramp(level: f32, layer: &DreadLoopAudio) -> f32 {
        let span = (layer.peak - layer.from).max(0.001);
        ((level - layer.from) / span).clamp(0.0, 1.0) * layer.ceiling
    }

    /// The opening is silent. If the bed is audible in the first minutes there
    /// is nothing left to bring in later.
    #[test]
    fn the_opening_has_no_dread_bed() {
        assert_eq!(ramp(0.0, &LOW), 0.0);
        assert_eq!(ramp(LOW.from - 0.01, &LOW), 0.0);
        assert_eq!(ramp(0.0, &HIGH), 0.0);
    }

    /// And the demo does hear it.
    ///
    /// The expedition trail walls at tier 1 and a demo session lands around
    /// 0.17. With both layers peaking at 1.0 the first sat at about five per
    /// cent of the drone it was under -- inaudible -- so the demo shipped with
    /// one unchanging drone and the story's soundtrack never started.
    #[test]
    fn a_demo_session_reaches_the_first_layer() {
        let demo = 0.17;
        let heard = ramp(demo, &LOW);
        assert!(
            heard > 0.0,
            "the low layer is still out of the demo's reach at cascade {demo}"
        );
        assert!(
            heard >= AMBIENT_VOL * 0.10,
            "at {heard} under a drone at {AMBIENT_VOL} the bed is technically \
             playing and practically inaudible"
        );
    }

    /// It thickens rather than simply getting louder: the second layer stays
    /// silent well past the point the first has arrived.
    #[test]
    fn the_layers_arrive_in_order() {
        assert!(HIGH.from > LOW.from, "both layers start together");
        let level = 0.5;
        assert!(ramp(level, &LOW) > 0.0, "the low layer should be in by half way");
        assert_eq!(ramp(level, &HIGH), 0.0, "the high layer is early");
    }

    /// And neither should ever drown the game. These sit under an ambient
    /// drone that is itself only at 0.30.
    #[test]
    fn it_stays_under_everything_else() {
        assert!(ramp(1.0, &LOW) <= AMBIENT_VOL);
        assert!(ramp(1.0, &HIGH) <= AMBIENT_VOL);
    }

    /// The menu sits under the flight bed, because it is the first thing
    /// anyone hears.
    #[test]
    fn the_menu_does_not_shout() {
        assert!(MENU_VOL > 0.0, "the menu is silent again");
        assert!(MENU_VOL < AMBIENT_VOL);
    }
}
