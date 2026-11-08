//! The diplomacy window (X): every faction's opinion of you, who they are at
//! war with, why your standing moved, and the treaties you can offer.
//!
//! Treaties:
//! - Pay tribute: buy a hostile or unfriendly faction's patience. Costs more
//!   the more dangerous the faction.
//! - Propose alliance: at standing 60 or better, and only if you aren't
//!   already allied with one of their enemies. Allies give their best prices,
//!   and your alliance costs you standing with everyone they fight.
//! - Break alliance: ends it, at a price in standing.

use bevy::prelude::*;

use super::{FactionStanding, StandingTier, FACTIONS};
use crate::ai_ship::components::{faction_display_name, faction_power, factions_hostile, AiShipType};
use crate::resources::Currency;
use crate::states::GameState;
use crate::ui::theme::{ThemeColors, ThemeFonts, ThemeSpacing};
use crate::ui::windows::framework::{spawn_floating_window, FloatingWindow, ScrollsOnWheel};

const WINDOW_ID: &str = "diplomacy";

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Treaty {
    Tribute,
    Alliance,
    BreakAlliance,
}

#[derive(Component, Clone, Copy)]
pub struct TreatyButton {
    pub faction: AiShipType,
    pub treaty: Treaty,
}

/// What tribute costs with a faction: a hostile apex boss is expensive.
pub fn tribute_cost(faction: AiShipType) -> u32 {
    (150.0 + faction_power(faction) * 120.0).round() as u32
}

/// How much one tribute buys.
pub const TRIBUTE_GAIN: f32 = 15.0;

fn tier_color(tier: StandingTier) -> Color {
    match tier {
        StandingTier::Hostile => ThemeColors::STATUS_DANGER,
        StandingTier::Unfriendly => ThemeColors::ACCENT_ORANGE,
        StandingTier::Neutral => ThemeColors::TEXT_SECONDARY,
        StandingTier::Friendly => ThemeColors::STATUS_OK,
        StandingTier::Allied => ThemeColors::ACCENT_CYAN,
    }
}

/// Factions at war with this one (by the galaxy's politics, not yours).
pub fn enemies_of(faction: AiShipType) -> Vec<AiShipType> {
    FACTIONS.iter().copied().filter(|&o| o != faction && factions_hostile(faction, o)).collect()
}

/// "At war with ..." in as few words as it takes: by name when they fight a
/// few, by exception when they fight most.
fn war_line(faction: AiShipType) -> String {
    let enemies = enemies_of(faction);
    let others: Vec<AiShipType> = FACTIONS.iter().copied().filter(|&o| o != faction).collect();
    let names = |list: &[AiShipType]| list.iter().map(|&e| faction_display_name(e)).collect::<Vec<_>>().join(", ");
    if enemies.is_empty() {
        "At peace with everyone".to_string()
    } else if enemies.len() == others.len() {
        "At war with everyone".to_string()
    } else if enemies.len() * 2 > others.len() {
        let spared: Vec<AiShipType> = others.into_iter().filter(|o| !enemies.contains(o)).collect();
        format!("At war with everyone but {}", names(&spared))
    } else {
        format!("At war with {}", names(&enemies))
    }
}

/// Is a treaty on the table right now, and if not, why not.
pub fn treaty_available(standing: &FactionStanding, faction: AiShipType, treaty: Treaty) -> Result<(), String> {
    let tier = standing.tier(faction);
    match treaty {
        Treaty::Tribute => {
            if tier <= StandingTier::Unfriendly {
                Ok(())
            } else {
                Err("They don't need paying off".into())
            }
        }
        Treaty::Alliance => {
            if standing.is_allied(faction) {
                return Err("Already allied".into());
            }
            if standing.value(faction) < StandingTier::ALLIANCE_FROM {
                return Err(format!("Needs standing {:.0}", StandingTier::ALLIANCE_FROM));
            }
            if let Some(rival) = enemies_of(faction).into_iter().find(|&e| standing.is_allied(e)) {
                return Err(format!("You are allied with {}", faction_display_name(rival)));
            }
            Ok(())
        }
        Treaty::BreakAlliance => {
            if standing.is_allied(faction) {
                Ok(())
            } else {
                Err("No alliance".into())
            }
        }
    }
}

/// Carry out a treaty. Returns a line for the log on success.
pub fn sign(
    standing: &mut FactionStanding,
    currency: &mut Currency,
    faction: AiShipType,
    treaty: Treaty,
) -> Result<(), String> {
    treaty_available(standing, faction, treaty)?;
    let name = faction_display_name(faction);
    match treaty {
        Treaty::Tribute => {
            let cost = tribute_cost(faction);
            if currency.credits < cost {
                return Err(format!("Tribute costs {cost} cr"));
            }
            currency.credits -= cost;
            standing.change(faction, TRIBUTE_GAIN, format!("You paid {name} {cost} cr in tribute"));
        }
        Treaty::Alliance => {
            standing.set_allied(faction, true, format!("Alliance signed with {name}"));
            for enemy in enemies_of(faction) {
                standing.change(enemy, -10.0, format!("You allied with their enemy, {name}"));
            }
        }
        Treaty::BreakAlliance => {
            standing.set_allied(faction, false, format!("You broke the alliance with {name}"));
            standing.change(faction, -30.0, "You broke the alliance");
        }
    }
    Ok(())
}

fn toggle_window(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    windows: Query<(Entity, &FloatingWindow)>,
    standing: Res<FactionStanding>,
    currency: Res<Currency>,
) {
    if !keyboard.just_pressed(KeyCode::KeyX) {
        return;
    }
    if let Some((e, _)) = windows.iter().find(|(_, w)| w.id == WINDOW_ID) {
        commands.entity(e).try_despawn();
        return;
    }
    let content = spawn_floating_window(
        &mut commands,
        WINDOW_ID,
        "Diplomacy",
        Vec2::new(600.0, 600.0),
        Vec2::new(460.0, 56.0),
    );
    // Twelve factions run taller than the screen, so the rows scroll inside
    // the window instead of hanging off its bottom edge.
    let rows = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                max_height: Val::Vh(72.0),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            ScrollPosition::default(),
            ScrollsOnWheel,
            DiplomacyRows { shown: shown(&standing, &currency) },
            ChildOf(content),
        ))
        .id();
    fill(&mut commands, rows, &standing, &currency);
}

/// The scrolling list of factions, and the state it was last drawn from.
#[derive(Component)]
struct DiplomacyRows {
    shown: String,
}

/// Everything the window shows, boiled down: the rows are rebuilt only when
/// this changes.
fn shown(standing: &FactionStanding, currency: &Currency) -> String {
    let mut s = String::new();
    for &faction in FACTIONS.iter() {
        s += &format!("{}{:.0}", standing.tier(faction).label(), standing.value(faction));
        for treaty in [Treaty::Tribute, Treaty::Alliance, Treaty::BreakAlliance] {
            s.push(if treaty_available(standing, faction, treaty).is_ok() { '+' } else { '-' });
        }
        s.push(if currency.credits >= tribute_cost(faction) { '$' } else { '_' });
    }
    for e in standing.log.iter().rev().take(5) {
        s += &format!("|{:?}{}{}", e.faction, e.delta, e.reason);
    }
    s
}

/// Rebuild the open window when anything it shows changes.
///
/// Runs in PostUpdate, after every close of the frame (X, the title bar's
/// close button) has gone through. In Update it could find the window still
/// standing in the same frame it was closed -- standing changes most frames
/// in a fight -- and queue new rows into an entity that was gone by the time
/// they landed: a crash on closing the window.
///
/// It runs before the UI is laid out and stacked. After, the new rows had no
/// place in the draw order yet and were drawn first, under the window's own
/// background -- and as FactionStanding was marked changed every frame, they
/// were new every frame: the whole list sat dim behind the window, with the
/// toolbar showing through it.
fn refresh_window(
    mut commands: Commands,
    mut rows: Query<(Entity, &mut DiplomacyRows)>,
    standing: Res<FactionStanding>,
    currency: Res<Currency>,
) {
    if !standing.is_changed() && !currency.is_changed() {
        return;
    }
    let Ok((rows, mut drawn)) = rows.single_mut() else { return };
    let now = shown(&standing, &currency);
    if drawn.shown == now {
        return;
    }
    drawn.shown = now;
    commands.entity(rows).despawn_related::<Children>();
    fill(&mut commands, rows, &standing, &currency);
}

fn press_treaty(
    buttons: Query<(&Interaction, &TreatyButton), Changed<Interaction>>,
    mut standing: ResMut<FactionStanding>,
    mut currency: ResMut<Currency>,
) {
    for (interaction, button) in buttons.iter() {
        if *interaction != Interaction::Pressed {
            continue;
        }
        if let Err(why) = sign(&mut standing, &mut currency, button.faction, button.treaty) {
            info!("Treaty refused: {why}");
        }
    }
}

fn text(parent: &mut ChildSpawnerCommands, s: impl Into<String>, size: f32, color: Color) {
    parent.spawn((Text::new(s.into()), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color)));
}

fn fill(commands: &mut Commands, content: Entity, standing: &FactionStanding, currency: &Currency) {
    commands.entity(content).with_children(|root| {
        text(
            root,
            "Who shoots at you, who trades with you, and why. Only Hostile factions attack on sight; a ship you fire on always fires back.",
            ThemeFonts::CAPTION,
            ThemeColors::TEXT_MUTED,
        );
        for &faction in FACTIONS.iter() {
            let tier = standing.tier(faction);
            let value = standing.value(faction);
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(1.0),
                    padding: UiRect::axes(Val::Px(ThemeSpacing::SM), Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(ThemeColors::BG_CARD),
            ))
            .with_children(|card| {
                card.spawn(Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(ThemeSpacing::MD),
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((Node { width: Val::Px(8.0), height: Val::Px(8.0), ..default() }, BackgroundColor(tier_color(tier))));
                    row.spawn(Node { flex_grow: 1.0, ..default() }).with_children(|n| {
                        text(n, faction_display_name(faction), ThemeFonts::BODY_SMALL, ThemeColors::TEXT_PRIMARY);
                    });
                    text(row, format!("{}  {:+.0}", tier.label(), value), ThemeFonts::BODY_SMALL, tier_color(tier));
                    for treaty in [Treaty::Tribute, Treaty::Alliance, Treaty::BreakAlliance] {
                        if treaty_available(standing, faction, treaty).is_err() {
                            continue;
                        }
                        let label = match treaty {
                            Treaty::Tribute => format!("Tribute {} cr", tribute_cost(faction)),
                            Treaty::Alliance => "Propose alliance".to_string(),
                            Treaty::BreakAlliance => "Break alliance".to_string(),
                        };
                        let affordable = treaty != Treaty::Tribute || currency.credits >= tribute_cost(faction);
                        row.spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(ThemeSpacing::MD), Val::Px(ThemeSpacing::XS)),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BackgroundColor(ThemeColors::BG_ELEVATED),
                            BorderColor::all(if affordable { ThemeColors::BORDER_BRIGHT } else { ThemeColors::BORDER_SUBTLE }),
                            Button,
                            Interaction::default(),
                            TreatyButton { faction, treaty },
                        ))
                        .with_children(|b| {
                            text(b, label, ThemeFonts::CAPTION, if affordable { ThemeColors::TEXT_PRIMARY } else { ThemeColors::TEXT_MUTED });
                        });
                    }
                });
                // The bar: -100 on the left, 100 on the right.
                card.spawn((
                    Node { width: Val::Percent(100.0), height: Val::Px(4.0), ..default() },
                    BackgroundColor(ThemeColors::BG_INPUT),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        Node { width: Val::Percent((value + 100.0) / 2.0), height: Val::Percent(100.0), ..default() },
                        BackgroundColor(tier_color(tier)),
                    ));
                });
                let war = war_line(faction);
                let prices = match tier.station_prices() {
                    None => "Their station won't let you dock".to_string(),
                    Some((buy, sell)) => format!("Station: buy x{buy:.2}, sell x{sell:.2}"),
                };
                text(card, format!("{war}.  {prices}"), ThemeFonts::CAPTION, ThemeColors::TEXT_MUTED);
            });
        }
        text(root, "Recent", ThemeFonts::BODY_SMALL, ThemeColors::TEXT_TITLE);
        if standing.log.is_empty() {
            text(root, "Nothing yet. Kills, hits, trade and treaties show up here.", ThemeFonts::CAPTION, ThemeColors::TEXT_MUTED);
        }
        for e in standing.log.iter().rev().take(5) {
            let color = if e.delta < 0.0 {
                ThemeColors::STATUS_DANGER
            } else if e.delta > 0.0 {
                ThemeColors::STATUS_OK
            } else {
                ThemeColors::TEXT_SECONDARY
            };
            let amount = if e.delta == 0.0 { String::new() } else { format!("{:+.0}  ", e.delta) };
            text(root, format!("{amount}{}: {}", faction_display_name(e.faction), e.reason), ThemeFonts::CAPTION, color);
        }
    });
}

pub struct DiplomacyUiPlugin;

impl Plugin for DiplomacyUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (toggle_window, press_treaty).run_if(in_state(GameState::Exploring)))
            .add_systems(
                PostUpdate,
                refresh_window.before(bevy::ui::UiSystems::Prepare).run_if(in_state(GameState::Exploring)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Closing the window in a frame where standing also changed used to
    /// crash: the redraw queued rows into the window being closed.
    #[test]
    fn closing_the_window_while_standing_changes() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<FactionStanding>()
            .insert_resource(Currency { credits: 100 })
            .add_systems(Update, toggle_window)
            .add_systems(PostUpdate, refresh_window);
        let press = |app: &mut App| {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.release(KeyCode::KeyX);
            keys.clear();
            keys.press(KeyCode::KeyX);
        };
        press(&mut app);
        app.update();
        let open = |app: &mut App| app.world_mut().query::<&FloatingWindow>().iter(app.world()).count();
        assert_eq!(open(&mut app), 1);
        for _ in 0..3 {
            press(&mut app);
            app.world_mut().resource_mut::<FactionStanding>().change(AiShipType::CorpseStars, -1.0, "test");
            app.update();
        }
        assert_eq!(open(&mut app), 0, "opened, closed, opened, closed");
    }

    /// A system that only borrows standing mutably must not rebuild the
    /// rows: rebuilt every frame, they never got a place in the draw order
    /// and sat dim behind the window. A real change still redraws them.
    #[test]
    fn rows_are_rebuilt_only_when_what_they_show_changes() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<FactionStanding>()
            .insert_resource(Currency { credits: 100 })
            .add_systems(Update, toggle_window)
            .add_systems(PostUpdate, refresh_window);
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyX);
        app.update();
        // Nothing clears just_pressed without the input plugin.
        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().clear();
        let first_row = |app: &mut App| {
            app.world_mut().query_filtered::<&Children, With<DiplomacyRows>>().single(app.world()).unwrap()[0]
        };
        let before = first_row(&mut app);
        for _ in 0..3 {
            app.world_mut().resource_mut::<FactionStanding>().set_changed();
            app.update();
        }
        assert_eq!(first_row(&mut app), before, "nothing shown changed");
        app.world_mut().resource_mut::<FactionStanding>().change(AiShipType::CorpseStars, -5.0, "test");
        app.update();
        assert_ne!(first_row(&mut app), before, "standing moved");
    }

    #[test]
    fn tribute_calms_a_hostile_faction_and_costs_credits() {
        let mut s = FactionStanding::default();
        let mut c = Currency { credits: 10_000 };
        let before = s.value(AiShipType::CorpseStars);
        sign(&mut s, &mut c, AiShipType::CorpseStars, Treaty::Tribute).unwrap();
        assert_eq!(s.value(AiShipType::CorpseStars), before + TRIBUTE_GAIN);
        assert_eq!(c.credits, 10_000 - tribute_cost(AiShipType::CorpseStars));
    }

    #[test]
    fn no_alliance_with_the_enemy_of_an_ally() {
        let mut s = FactionStanding::default();
        let mut c = Currency { credits: 0 };
        s.change(AiShipType::GildedThrone, 80.0, "test");
        s.change(AiShipType::SynthesisCollective, 150.0, "test");
        sign(&mut s, &mut c, AiShipType::GildedThrone, Treaty::Alliance).unwrap();
        // The Throne hunts the Collective.
        assert!(sign(&mut s, &mut c, AiShipType::SynthesisCollective, Treaty::Alliance).is_err());
    }

    #[test]
    fn friendly_factions_need_no_tribute() {
        let s = FactionStanding::default();
        assert!(treaty_available(&s, AiShipType::TheSilence, Treaty::Tribute).is_err());
    }
}
