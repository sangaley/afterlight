use bevy::prelude::*;

use crate::ai_ship::components::{AiShipType, AiShipWreck};
use crate::ai_ship::spawner;
use crate::building::ModuleRegistry;
use crate::components::{Ship, Weapon};
use crate::contracts::MissionBoardOpen;
use crate::crew::eva_salvage::EvaSalvaging;
use crate::events::{NotificationType, ShowNotification};
use crate::states::{BuildState, GameState};
use crate::ui::theme::{ThemeColors, ThemeFonts, ThemeSpacing};

// ============================================================================
// GUIDED TUTORIAL
// A "Flight Training" card teaches a new pilot the whole loop — and the ideas
// behind it, not just the keys. It alternates two kinds of step:
//   * ACTION steps advance when the player performs the action (thrust, ping,
//     kill, salvage, dock, build, ...).
//   * INFO steps (Advance::Continue) explain a concept — what the HUD means,
//     why the ship is modular, how the salvage economy works — and advance on
//     [Space]. Nothing is on a timer, so a slow reader is never rushed.
//
// The combat lesson is a real encounter. When the player reaches the scan step
// a single weak Recursive Kingdom raider is spawned OFF-SCREEN but inside radar range,
// so the player has to ping to find its bearing and fly out to it. Its weapons
// are clamped to a short range (`tame_tutorial_enemy`) so it holds near its
// spawn instead of charging across the system. It dies into a real wreck
// (ai_ship::wreck) the player then strips with a crew. Home space (Haven) is
// otherwise safe, so this is the player's only contact during training.
//
// It only arms on a brand-new expedition (never on a loaded save) and vanishes
// the moment training is done, so it stays out of the way of returning players.
// ============================================================================

/// How the current step is completed.
///
/// `pub` so the autoplay director (`autoplay.rs`, dev tooling) can ask what
/// the current step is waiting on and press the matching key, instead of
/// guessing its way through training.
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Advance {
    Launch,    // reached open space (launched from the station)
    Continue,  // an info step — read it, press [Space]
    Thrust,    // held W / Up
    Scan,      // pressed Z (also the step the training raider spawns on)
    Kill,      // the training raider became a wreck
    Salvage,   // a salvage detail is out
    Dock,      // docked at a station again
    Build,     // opened the shipyard (build mode)
    Contracts, // opened the bounty board
    Crew,      // opened the crew menu
}

/// How far off the player's bow the training raider spawns. Off-screen at the
/// default zoom (~±1150 x ±650 world units visible) so it can't just be seen,
/// but well inside the starter RadarArray's 2500-unit ping range so a scan
/// reveals it. Diagonal so both axes clear the screen edge.
const ENEMY_SPAWN_OFFSET: Vec2 = Vec2::new(1400.0, 1400.0);

/// Weapon range the training raider is clamped to once spawned. Its engage
/// range is weapon-range * 1.05 (see ai_ship::ai_brain), so a short value keeps
/// it patrolling its spawn until the player closes in — instead of a Rust
/// Swarm's stock 4800-range rockets dragging it across the system on contact.
const TUTORIAL_ENEMY_RANGE: f32 = 700.0;

/// One guided step: the instruction/lesson on the card + how it's completed.
struct TutorialStep {
    body: &'static str,
    advance: Advance,
}

/// The training script, in order. Action steps teach by doing; info steps
/// (Advance::Continue) teach the concept behind what the player just did.
/// Keys are written in [BRACKETS] since the card text is a single color.
const STEPS: &[TutorialStep] = &[
    TutorialStep {
        advance: Advance::Launch,
        body: "Welcome aboard, captain. You're docked at your home station - safe here. Use the Build button below to rebuild your ship anytime, and Launch when you're ready for the void. Every button shows its shortcut key if you'd rather use those.",
    },
    TutorialStep {
        advance: Advance::Continue,
        body: "That bar across the top is your ship's life. HULL is your armor, PWR is power - reactors make it, every module spends it - and FUEL burns whenever you thrust. Let PWR fall short and systems start to fail.",
    },
    TutorialStep {
        advance: Advance::Thrust,
        body: "Hold [W] to thrust forward. Your nose always tracks the mouse, so you steer by aiming where you want to go. [S] reverses, [A]/[D] strafe. Watch FUEL tick down as you burn.",
    },
    TutorialStep {
        advance: Advance::Continue,
        body: "Your ship is built from modules locked to a grid - reactor, engines, guns, radar, cargo. Take a hit and individual modules break, and whatever they did stops working. All of it is repairable back home.",
    },
    TutorialStep {
        advance: Advance::Scan,
        body: "The void is dark - you can't see far. Hit the Ping button (or [Z]) to sweep radar; the sweep lights up contacts around you. But a ping is loud, spiking your NOISE and drawing hunters. See what it reveals out there.",
    },
    TutorialStep {
        advance: Advance::Kill,
        body: "That contact is a raider, holding position at range. Thrust out to it, put your nose on it, and hold [SPACE] to fire. Your guns shoot where you aim and burn AMMO - so aim, don't just spray.",
    },
    TutorialStep {
        advance: Advance::Salvage,
        body: "Destroyed - but a kill pays nothing by itself. The reward is the wreck it leaves. Fly in close and use Dock: idle crew suit up, cross over, and strip its cargo into your hold. Dock again recalls them.",
    },
    TutorialStep {
        advance: Advance::Continue,
        body: "That cargo isn't credits yet - you sell it at a station. That's the whole loop out here: hunt, salvage, sell, and pour it back into a stronger ship. Time to head home.",
    },
    TutorialStep {
        advance: Advance::Dock,
        body: "Turn around and fly back to your home station. Ease in close and dock to unload your haul and resupply.",
    },
    TutorialStep {
        advance: Advance::Continue,
        body: "Docked. This is where you spend what you earn - rebuild the ship, take on contracts, and manage your crew. Let's walk each one.",
    },
    TutorialStep {
        advance: Advance::Build,
        body: "Use the Build button to open the shipyard. Place modules on the grid - but keep it balanced: every module draws power, so add reactors to match, and wrap the whole thing in hull so it can take hits.",
    },
    TutorialStep {
        advance: Advance::Contracts,
        body: "Use the Jobs button for the bounty board. Contracts pay credits and shift your standing with the factions - reputation decides who meets you with guns and who meets you with trade.",
    },
    TutorialStep {
        advance: Advance::Crew,
        body: "Use the Crew button to manage crew. Hire hands and post them to stations - an unmanned reactor or gun runs at a fraction of its output. Crew are what actually bring the ship to life.",
    },
    TutorialStep {
        advance: Advance::Continue,
        body: "That's the loop, captain: hunt, salvage, sell, and pour it back into a stronger ship. One more thing. An expedition came out here before us and never reported back - they left records scattered across the wrecks, and the company wants them recovered. Your progress is tracked under the top bar. Use the Map button for the star map, then Radar there for the whole galaxy.",
    },
];

/// Runtime state for the guided tutorial. Armed on a brand-new expedition (see
/// `crate::ui::handle_menu_input`), never on a loaded save.
#[derive(Resource, Default)]
pub struct Tutorial {
    pub active: bool,
    pub step: usize,
    /// The training raider, once spawned — tracked so we can tell exactly when
    /// *it* (not some unrelated contact) has been reduced to a wreck.
    enemy: Option<Entity>,
    /// One-shot guard so the raider is only ever spawned once per expedition.
    enemy_spawned: bool,
    /// Set once the raider's weapons have been clamped to short range.
    enemy_tamed: bool,
}

impl Tutorial {
    /// Start the tutorial from the beginning. Called when a new expedition
    /// launches from the main menu.
    pub fn begin(&mut self) {
        self.active = true;
        self.step = 0;
        self.enemy = None;
        self.enemy_spawned = false;
        self.enemy_tamed = false;
    }

    fn current(&self) -> Option<&'static TutorialStep> {
        STEPS.get(self.step)
    }

    /// What the current step is waiting on, or None when training is over or
    /// was never armed. Read by the autoplay director (`autoplay.rs`).
    pub fn pending(&self) -> Option<Advance> {
        if !self.active {
            return None;
        }
        self.current().map(|s| s.advance)
    }
}

/// Marks the raider spawned for the combat/salvage lesson.
#[derive(Component)]
struct TutorialEnemy;

/// Wrapper node — visibility is toggled here to show/hide the whole card.
#[derive(Component)]
struct TutorialRoot;

/// The "1 / N" progress readout in the card header.
#[derive(Component)]
struct TutorialProgressText;

/// The instruction/lesson line for the current step.
#[derive(Component)]
struct TutorialBodyText;

/// The footer prompt (continue hint + dismiss hint).
#[derive(Component)]
struct TutorialFooterText;

pub struct TutorialPlugin;

impl Plugin for TutorialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tutorial>()
            .add_systems(Startup, spawn_tutorial_card)
            // Passing back through the main menu (or dying) clears any leftover
            // tutorial state so a subsequently *loaded* save never shows the
            // card. A new expedition re-arms it via `Tutorial::begin`.
            .add_systems(OnEnter(GameState::MainMenu), deactivate_tutorial)
            .add_systems(OnEnter(GameState::GameOver), deactivate_tutorial)
            .add_systems(
                Update,
                (spawn_tutorial_enemy, tame_tutorial_enemy)
                    .run_if(in_state(GameState::Exploring)),
            )
            .add_systems(Update, (advance_tutorial, update_tutorial_card).chain());
    }
}

fn deactivate_tutorial(mut tutorial: ResMut<Tutorial>) {
    tutorial.active = false;
}

/// Spawns the (initially hidden) training card, pinned top-center below the HUD
/// bar and clear of the top-right notifications and the bottom build panel.
fn spawn_tutorial_card(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(crate::ui::RIGHT_COLUMN_TOP),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            ZIndex(50),
            Visibility::Hidden,
            TutorialRoot,
            crate::ui::BelowTopBar,
        ))
        .with_children(|wrapper| {
            wrapper
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Px(460.0),
                        max_width: Val::Percent(92.0),
                        padding: UiRect::all(Val::Px(ThemeSpacing::LG)),
                        row_gap: Val::Px(ThemeSpacing::SM),
                        ..default()
                    },
                    BackgroundColor(ThemeColors::BG_CARD),
                ))
                .with_children(|card| {
                    // Subtle accent line — marks this as the training card
                    // (same idiom as the main-menu title accents).
                    card.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Px(2.0),
                            margin: UiRect::bottom(Val::Px(ThemeSpacing::XS)),
                            ..default()
                        },
                        BackgroundColor(ThemeColors::ACCENT_BLUE),
                    ));

                    // Header: title + progress.
                    card.spawn(Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|header| {
                        header.spawn((
                            Text::new("FLIGHT TRAINING"),
                            TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
                            TextColor(ThemeColors::ACCENT_BLUE),
                        ));
                        header.spawn((
                            Text::new(format!("1 / {}", STEPS.len())),
                            TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
                            TextColor(ThemeColors::TEXT_MUTED),
                            TutorialProgressText,
                        ));
                    });

                    // Instruction/lesson line for the current step.
                    card.spawn((
                        Text::new(STEPS[0].body),
                        TextFont { font_size: FontSize::Px(ThemeFonts::BODY), ..default() },
                        TextColor(ThemeColors::TEXT_PRIMARY),
                        TutorialBodyText,
                    ));

                    // Footer prompt — continue hint (info steps) + dismiss hint.
                    // Updated per step; Semicolon dismisses (not ESC, which pauses).
                    card.spawn((
                        Text::new("[;] dismiss training"),
                        TextFont { font_size: FontSize::Px(ThemeFonts::CAPTION), ..default() },
                        TextColor(ThemeColors::TEXT_MUTED),
                        Node { margin: UiRect::top(Val::Px(ThemeSpacing::XS)), ..default() },
                        TutorialFooterText,
                    ));
                });
        });
}

/// Spawns the lone training raider once the player reaches the scan step. Weak
/// faction (Recursive Kingdom), off-screen but inside radar range, so the player has
/// to ping to find it and fly out to engage.
fn spawn_tutorial_enemy(
    mut tutorial: ResMut<Tutorial>,
    mut commands: Commands,
    registry: Res<ModuleRegistry>,
    asset_server: Res<AssetServer>,
    ship_q: Query<&Transform, With<Ship>>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if !tutorial.active
        || tutorial.enemy_spawned
        || tutorial.current().map(|s| s.advance) != Some(Advance::Scan)
    {
        return;
    }
    let Ok(player_tf) = ship_q.single() else {
        return;
    };
    let spawn_pos = player_tf.translation.truncate() + ENEMY_SPAWN_OFFSET;

    let enemy = spawner::spawn_ai_ship(
        AiShipType::RecursiveKingdom,
        spawn_pos,
        &mut commands,
        &registry,
        &asset_server,
    );
    commands.entity(enemy).insert(TutorialEnemy);
    tutorial.enemy = Some(enemy);
    tutorial.enemy_spawned = true;

    notifications.write(ShowNotification {
        message: "Sensors flag an unknown contact beyond visual range.".into(),
        notification_type: NotificationType::Warning,
        duration: 4.0,
    });
}

/// Clamps the training raider's weapon ranges to a short value so it holds
/// position near its spawn until the player flies out to it (its engage range
/// is derived from its longest weapon — see the const doc). Runs until it finds
/// the raider's weapons (spawned a frame or two after the root), then latches.
fn tame_tutorial_enemy(
    mut tutorial: ResMut<Tutorial>,
    children_q: Query<&Children>,
    mut weapon_q: Query<&mut Weapon>,
) {
    if tutorial.enemy_tamed {
        return;
    }
    let Some(enemy) = tutorial.enemy else {
        return;
    };
    let Ok(children) = children_q.get(enemy) else {
        return;
    };

    let mut tamed_any = false;
    for child in children.iter() {
        if let Ok(mut weapon) = weapon_q.get_mut(child) {
            weapon.range = weapon.range.min(TUTORIAL_ENEMY_RANGE);
            tamed_any = true;
        }
    }
    if tamed_any {
        tutorial.enemy_tamed = true;
    }
}

/// Advances the tutorial when the player performs the current step's action (or
/// presses [Space] on an info step).
fn advance_tutorial(
    mut tutorial: ResMut<Tutorial>,
    state: Res<State<GameState>>,
    build_state: Res<State<BuildState>>,
    mission_board: Res<MissionBoardOpen>,
    keyboard: Res<ButtonInput<KeyCode>>,
    wreck_q: Query<(), With<AiShipWreck>>,
    eva_q: Query<(), With<EvaSalvaging>>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    if !tutorial.active {
        return;
    }

    // Semicolon dismisses training at any point (avoids ESC, which pauses).
    // Any raider already spawned is left in place as a normal contact.
    if keyboard.just_pressed(KeyCode::Semicolon) {
        tutorial.active = false;
        notifications.write(ShowNotification {
            message: "Flight training dismissed.".into(),
            notification_type: NotificationType::Info,
            duration: 2.5,
        });
        return;
    }

    let Some(step) = tutorial.current() else {
        return;
    };
    let in_space = *state.get() == GameState::Exploring;
    let completed = match step.advance {
        Advance::Launch => in_space, // launched from the station into open space
        Advance::Continue => keyboard.just_pressed(KeyCode::Space),
        Advance::Thrust => {
            in_space && (keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp))
        }
        Advance::Scan => in_space && keyboard.just_pressed(KeyCode::KeyZ),
        // The raider has been reduced to a wreck (Wreck/AiShipWreck attached in
        // place by ai_ship::wreck::ai_ship_death_system).
        Advance::Kill => tutorial.enemy.is_some_and(|e| wreck_q.get(e).is_ok()),
        // A salvage detail is out (F dispatched idle crew — see eva_salvage).
        Advance::Salvage => !eva_q.is_empty(),
        // A station berth, not a settlement: the steps after this one open
        // the shipyard, the bounty board and the crew roster, and only a
        // station has those. Settling for an outpost colony left training on
        // a lesson about a shipyard the player had no way to open.
        Advance::Dock => *state.get() == GameState::StationDocked,
        // Station-side lessons: opened the shipyard / bounty board / crew menu.
        Advance::Build => *build_state.get() != BuildState::Inactive,
        Advance::Contracts => mission_board.0,
        Advance::Crew => {
            *state.get() == GameState::StationDocked && keyboard.just_pressed(KeyCode::KeyC)
        }
    };

    if !completed {
        return;
    }

    tutorial.step += 1;
    if tutorial.step >= STEPS.len() {
        tutorial.active = false;
        notifications.write(ShowNotification {
            message: "Training complete - good luck out there, captain.".into(),
            notification_type: NotificationType::Success,
            duration: 6.0,
        });
    }
}

/// Keeps the card's text and visibility in sync with the tutorial state. Only
/// shown while docked at the home station or out exploring — never over menus,
/// the pause screen, or the game-over screen.
fn update_tutorial_card(
    tutorial: Res<Tutorial>,
    state: Res<State<GameState>>,
    mut root_q: Query<&mut Visibility, With<TutorialRoot>>,
    mut body_q: Query<&mut Text, (With<TutorialBodyText>, Without<TutorialProgressText>, Without<TutorialFooterText>)>,
    mut prog_q: Query<&mut Text, (With<TutorialProgressText>, Without<TutorialBodyText>, Without<TutorialFooterText>)>,
    mut foot_q: Query<&mut Text, (With<TutorialFooterText>, Without<TutorialBodyText>, Without<TutorialProgressText>)>,
) {
    let showing = tutorial.active
        && matches!(*state.get(), GameState::StationDocked | GameState::Exploring)
        && tutorial.current().is_some();

    if let Ok(mut vis) = root_q.single_mut() {
        *vis = if showing { Visibility::Inherited } else { Visibility::Hidden };
    }
    let Some(step) = tutorial.current().filter(|_| showing) else {
        return;
    };

    if let Ok(mut body) = body_q.single_mut() {
        if body.0.as_str() != step.body {
            body.0 = step.body.to_string();
        }
    }
    if let Ok(mut prog) = prog_q.single_mut() {
        let text = format!("{} / {}", tutorial.step + 1, STEPS.len());
        if prog.0.as_str() != text {
            prog.0 = text;
        }
    }
    if let Ok(mut foot) = foot_q.single_mut() {
        let text = if step.advance == Advance::Continue {
            "[Space] continue      [;] dismiss training"
        } else {
            "[;] dismiss training"
        };
        if foot.0.as_str() != text {
            foot.0 = text.to_string();
        }
    }
}

#[cfg(test)]
mod dock_step_tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    fn dock_step() -> usize {
        STEPS.iter().position(|s| s.advance == Advance::Dock).unwrap()
    }

    fn app_berthed_in(state: GameState) -> App {
        let mut app = App::new();
        app.add_plugins(StatesPlugin);
        app.insert_state(state);
        app.init_state::<BuildState>();
        app.init_resource::<MissionBoardOpen>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_message::<ShowNotification>();
        app.insert_resource(Tutorial { active: true, step: dock_step(), ..default() });
        app.add_systems(Update, advance_tutorial);
        app
    }

    /// "Fly back to your home station" is answered by a station berth.
    #[test]
    fn docking_at_a_station_completes_the_step() {
        let mut app = app_berthed_in(GameState::StationDocked);
        app.update();
        assert_eq!(app.world().resource::<Tutorial>().step, dock_step() + 1);
    }

    /// An outpost colony is not a station: the next lessons open its
    /// shipyard, bounty board and crew roster, and a settlement has none.
    #[test]
    fn a_settlement_berth_does_not() {
        let mut app = app_berthed_in(GameState::Docked);
        app.update();
        assert_eq!(app.world().resource::<Tutorial>().step, dock_step());
    }
}
