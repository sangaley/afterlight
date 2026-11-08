use bevy::prelude::*;

mod components;
mod resources;
mod events;
mod states;
mod ship;
mod world;
mod crew;
mod building;
mod ui;
mod meta;
mod narrative;
mod contracts;
mod combat;
mod radar;
mod camera;
mod despawn_races;
mod sprite_map;
pub mod ai_ship;
mod celestial;
mod vfx;
mod spatial;
mod autoplay;
mod demo;
mod audio;
mod debug;
mod gamepad;
mod tutorial;
mod diplomacy;
mod perf;

use states::GameState;
use events::EventsPlugin;
use resources::InputState;
use ship::ShipPlugin;
use world::WorldPlugin;
use crew::CrewPlugin;
use building::BuildingPlugin;
use ui::UiPlugin;
use meta::MetaPlugin;
use combat::CombatPlugin;
use radar::RadarPlugin;
use camera::CameraPlugin;
use ai_ship::AiShipPlugin;
use contracts::ContractsPlugin;
use celestial::CelestialPlugin;
use vfx::VfxPlugin;
use spatial::SpatialPlugin;
use autoplay::AutoplayPlugin;
use demo::DemoPlugin;
use audio::GameAudioPlugin;
use debug::DebugPlugin;
use gamepad::GamepadPlugin;
use tutorial::TutorialPlugin;

fn main() {
    App::new()
        .set_error_handler(despawn_races::survive_despawned)
        .add_systems(Update, despawn_races::reap_stranded)
        // Default Bevy plugins (windowing, rendering, input, etc.)
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Afterlight".into(),
                resolution: (1280, 720).into(),
                // Perf runs (perf.rs) measure what a frame costs, so they
                // must not be held to the display's refresh by vsync.
                present_mode: if perf::perf_mode() {
                    bevy::window::PresentMode::AutoNoVsync
                } else {
                    bevy::window::PresentMode::default()
                },
                ..default()
            }),
            // An offscreen run (AFTERLIGHT_OFFSCREEN, unattended filming and
            // tests) renders into an image, not the window -- so it keeps
            // going when macOS sleeps the display and takes the window away
            // with the monitor, instead of quitting half way through.
            exit_condition: if std::env::var("AFTERLIGHT_OFFSCREEN").ok().as_deref() == Some("1") {
                bevy::window::ExitCondition::DontExit
            } else {
                bevy::window::ExitCondition::OnAllClosed
            },
            ..default()
        }))

        // Game state
        .init_state::<GameState>()

        // Global resources
        .init_resource::<InputState>()
        .init_resource::<crate::resources::InteractPress>()
        // Ahead of every F handler, so they all see the same single press -
        // but AFTER the gamepad bridge, which synthesizes its own key presses
        // into ButtonInput during PreUpdate. Refreshing in First read the
        // keyboard before the controller had spoken, so West stopped docking.
        .add_systems(
            PreUpdate,
            crate::resources::refresh_interact_press
                .after(crate::gamepad::bridge_gamepad_buttons),
        )
        .insert_resource(ClearColor(Color::srgb(0.05, 0.15, 0.35)))

        // Events
        .add_plugins(EventsPlugin)

        // Our game plugins
        .add_plugins((
            ShipPlugin,
            WorldPlugin,
            CrewPlugin,
            BuildingPlugin,
            UiPlugin,
            MetaPlugin,
            CombatPlugin,
            RadarPlugin,
            CameraPlugin,
            // AbyssHorrorPlugin parked in src/parked/ (see parked/README.md) —
            // built around creatures watching/fleeing you, pointless while
            // creature spawning is off. Move it back and re-add here to revive.
            AiShipPlugin,
            ContractsPlugin,
            CelestialPlugin,
            VfxPlugin,
        ))
        .add_plugins(SpatialPlugin)
        .add_plugins(DemoPlugin)
        .add_plugins(demo::CapturePlugin)
        .add_plugins(AutoplayPlugin)
        .add_plugins(DebugPlugin)
        .add_plugins(GameAudioPlugin)
        .add_plugins(GamepadPlugin)
        .add_plugins(TutorialPlugin)
        .add_plugins(diplomacy::DiplomacyPlugin)
        .add_plugins(narrative::NarrativePlugin)
        .add_plugins(ship::towing::TowingPlugin)
        .add_plugins(perf::PerfPlugin)

        .run();
}
