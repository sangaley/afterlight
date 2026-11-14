//! A built gun's window, laid out the way From the Depths lays out its
//! weapon screens: what the gun is made of on the left, the shell in the
//! middle, and what the last change did to it on the right.
//!
//! Opened like any weapon's window (right-click the firing piece while
//! docked). The sliders and the shell designer are the same as before; this
//! module adds the parts list, the before-and-after numbers and the armour
//! readout, and puts the three side by side.

use bevy::prelude::*;

use crate::combat::gun_assembly::GunAssembly;
use crate::combat::gun_math::{GunStats, BASE_GAUGE_MM, GAUGE_PER_INCREASER_MM, GAUGE_PER_REDUCER_MM, RAIL_VELOCITY_PER_MAGNET};
use crate::components::ModuleType;
use crate::ui::theme::{Sheet, ThemeColors, ThemeFonts};
use super::framework::*;
use super::tuning::TuningWindow;

/// The parts column's list, rebuilt when the gun changes.
#[derive(Component)]
pub struct GunPartsList;

/// The performance column's rows, rebuilt when the numbers change.
#[derive(Component)]
pub struct GunPerformanceList;

/// The armour column's rows.
#[derive(Component)]
pub struct GunArmourList;

/// The armour the readout tests against, from the game's own plates
/// (combat::impact's figures, in penetration units).
const ARMOURS: [(&str, f32); 4] = [
    ("Ablative", 5.0),
    ("Steel hull", 15.0),
    ("Hard plate", 40.0),
    ("Thickest hull", 80.0),
];

fn column(commands: &mut Commands, width: f32, ruled: bool) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(width),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(3.0),
                padding: UiRect::axes(Val::Px(12.0), Val::Px(4.0)),
                border: UiRect::left(Val::Px(if ruled { 1.0 } else { 0.0 })),
                ..default()
            },
            BorderColor::all(Sheet::RULE),
        ))
        .id()
}

fn list(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node { flex_direction: FlexDirection::Column, margin: UiRect::vertical(Val::Px(4.0)), ..default() })
        .id()
}

/// Spawn the window for the gun whose firing piece is `firing_piece`.
pub fn spawn_gun_window(commands: &mut Commands, firing_piece: Entity) {
    let content = spawn_floating_window(
        commands,
        &format!("tuning_{:?}", firing_piece),
        "Built gun",
        Vec2::new(980.0, 0.0),
        Vec2::new(40.0, 70.0),
    );
    commands.entity(content).insert(TuningWindow { module_entity: firing_piece, module_type: ModuleType::FiringPiece });

    let row = commands.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Stretch, ..default() }).id();
    commands.entity(content).add_child(row);

    // THE GUN: its parts, then the sliders and the power.
    let gun = column(commands, 300.0, false);
    spawn_window_section(commands, gun, "THE GUN");
    let parts = list(commands);
    commands.entity(parts).insert(GunPartsList);
    commands.entity(gun).add_child(parts);
    super::tuning::spawn_tuning_controls(commands, gun, ModuleType::FiringPiece);

    // SHELL: the designer.
    let shell = column(commands, 360.0, true);
    super::shell_designer::spawn_shell_section(commands, shell, &Default::default());

    // PERFORMANCE: the numbers, and what the last change did to them.
    let perf = column(commands, 300.0, true);
    spawn_window_section(commands, perf, "PERFORMANCE   CHANGE");
    let numbers = list(commands);
    commands.entity(numbers).insert(GunPerformanceList);
    commands.entity(perf).add_child(numbers);
    spawn_window_section(commands, perf, "AGAINST ARMOUR, SQUARE ON");
    let armour = list(commands);
    commands.entity(armour).insert(GunArmourList);
    commands.entity(perf).add_child(armour);

    commands.entity(row).add_children(&[gun, shell, perf]);
}

/// One part of the gun: a name and what it does for it.
pub fn part_lines(gun: &GunAssembly) -> Vec<(String, String)> {
    let p = &gun.parts;
    let s = &gun.stats;
    let mut out = vec![("Firing piece".to_string(), format!("{BASE_GAUGE_MM:.0} mm base"))];
    if p.gauge_increasers > 0 {
        out.push((format!("Gauge increaser x{}", p.gauge_increasers), format!("+{:.0} mm", p.gauge_increasers as f32 * GAUGE_PER_INCREASER_MM)));
    }
    if p.gauge_reducers > 0 {
        out.push((format!("Gauge reducer x{}", p.gauge_reducers), format!("-{:.0} mm", p.gauge_reducers as f32 * GAUGE_PER_REDUCER_MM)));
    }
    if !p.loaders.is_empty() {
        let longest = p.loaders.iter().map(|l| l.length_m).max().unwrap_or(0);
        out.push((format!("Loader x{}", p.loaders.len()), format!("shell up to {longest} m")));
        let clips: f32 = p.loaders.iter().map(|l| l.clips).sum();
        if clips > 0.0 {
            out.push(("Clip racks".to_string(), format!("{} ready", s.ready_rack)));
        }
    }
    if p.barrel_m > 0 {
        out.push((format!("Barrel {} m", p.barrel_m), format!("burns {:.1} m", s.burn_m)));
    }
    if p.rail_magnets > 0 {
        out.push((format!("Rail magnet x{}", p.rail_magnets), format!("+{:.0} m/s", p.rail_magnets as f32 * RAIL_VELOCITY_PER_MAGNET)));
    }
    if p.coolers > 0 {
        out.push((format!("Cooler x{}", p.coolers), format!("{:.1} heat/s off", s.cooling_per_s)));
    }
    if p.absorbers > 0 {
        out.push((format!("Recoil absorber x{}", p.absorbers), "less kick".to_string()));
    }
    if p.muzzle_brake {
        out.push(("Muzzle brake".to_string(), "less kick".to_string()));
    }
    if gun.on_turret {
        out.push(("Turret hoist".to_string(), "feeds it".to_string()));
    } else if gun.feeders > 0 {
        out.push((format!("Feeder x{}", gun.feeders), format!("{:.0} shells/min up", gun.feed_rpm)));
    }
    out
}

/// One number: its name, how it reads, how the last change moved it (as
/// words, and whether that was for the better).
pub fn performance_lines(now: &GunStats, before: Option<&GunStats>) -> Vec<(&'static str, String, Option<(String, bool)>)> {
    // (name, value, the number, how to show a change, bigger is better)
    let reload = |s: &GunStats| if s.rpm > 0.0 { 60.0 / s.rpm } else { 0.0 };
    let reach = |s: &GunStats| s.velocity_mps * s.range_mult;
    let rows: [(&'static str, fn(&GunStats) -> f32, String, u8, bool); 9] = [
        ("Gauge", |s| s.gauge_mm, format!("{:.0} mm", now.gauge_mm), 0, true),
        ("Between shots", reload_fn(), format!("{:.1} s", reload(now)), 1, false),
        ("Damage a shot", |s| s.damage, format!("{:.0}", now.damage), 0, true),
        // In the units armour is measured in (a steel hull is 15), so it
        // reads straight against the armour list below it.
        ("Penetration", |s| crate::combat::gun_assembly::penetration_for(s), format!("{:.1}", crate::combat::gun_assembly::penetration_for(now)), 1, true),
        ("Velocity", |s| s.velocity_mps, format!("{:.0} m/s", now.velocity_mps), 0, true),
        ("Reach", reach_fn(), format!("{:.0}%", reach(now) / 8.5), 0, true),
        ("Spread", |s| s.spread_deg, format!("{:.2} deg", now.spread_deg), 2, false),
        ("Recoil", |s| s.recoil, format!("{:.0}", now.recoil), 0, false),
        ("Heat a shot", |s| s.heat_per_shot, format!("{:.1}", now.heat_per_shot), 1, false),
    ];
    rows.into_iter()
        .map(|(name, get, value, places, bigger_better)| {
            let change = before.and_then(|b| {
                let d = get(now) - get(b);
                let shown = format!("{:+.*}", places as usize, d);
                // A change too small to show is no change.
                if shown.trim_start_matches(['+', '-']).chars().all(|c| c == '0' || c == '.') {
                    return None;
                }
                Some((shown, (d > 0.0) == bigger_better))
            });
            (name, value, change)
        })
        .collect()
}

fn reload_fn() -> fn(&GunStats) -> f32 {
    |s| if s.rpm > 0.0 { 60.0 / s.rpm } else { 0.0 }
}

fn reach_fn() -> fn(&GunStats) -> f32 {
    // Shown against a plain 850 m/s round, so it reads as a share.
    |s| s.velocity_mps * s.range_mult / 8.5
}

/// How much of a square-on hit bites into each armour.
pub fn armour_lines(penetration: f32) -> Vec<(&'static str, f32, f32)> {
    ARMOURS
        .iter()
        .map(|(name, t)| (*name, *t, crate::combat::impact::bite(penetration, *t, 1.0)))
        .collect()
}

fn text(parent: &mut ChildSpawnerCommands, words: String, size: f32, color: Color) {
    parent.spawn((Text::new(words), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color)));
}

fn ruled_row(parent: &mut ChildSpawnerCommands, columns: Vec<RepeatedGridTrack>, fill: impl FnOnce(&mut ChildSpawnerCommands)) {
    parent
        .spawn((
            Node {
                display: Display::Grid,
                grid_template_columns: columns,
                column_gap: Val::Px(8.0),
                padding: UiRect::vertical(Val::Px(3.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(Sheet::RULE),
        ))
        .with_children(fill);
}

/// Keep the parts, the numbers and the armour readout up to date.
#[allow(clippy::type_complexity)]
pub fn gun_window_refresh(
    mut commands: Commands,
    windows: Query<&TuningWindow>,
    guns: Query<&GunAssembly>,
    parts: Query<Entity, With<GunPartsList>>,
    numbers: Query<Entity, With<GunPerformanceList>>,
    armour: Query<Entity, With<GunArmourList>>,
    // The gun shown, the numbers it had before the last change, and now.
    mut shown: Local<Option<(Entity, Option<GunStats>, GunStats, usize)>>,
) {
    let Ok(tw) = windows.single() else { return };
    let (Ok(parts_e), Ok(numbers_e), Ok(armour_e)) = (parts.single(), numbers.single(), armour.single()) else { return };
    let gun = guns.get(tw.module_entity).ok();
    let stats = gun.map(|g| g.stats.clone()).unwrap_or_default();
    let n_parts = gun.map_or(0, |g| g.members.len());

    let (before, fresh) = match shown.as_ref() {
        Some((e, b, last, n)) if *e == tw.module_entity => {
            if *last == stats && *n == n_parts {
                // Nothing changed -- and the lists exist: done.
                return;
            }
            (Some(last.clone()).or(b.clone()), false)
        }
        _ => (None, true),
    };
    let before = if fresh { None } else { before };
    *shown = Some((tw.module_entity, before.clone(), stats.clone(), n_parts));

    for e in [parts_e, numbers_e, armour_e] {
        commands.entity(e).despawn_related::<Children>();
    }

    let Some(gun) = gun else {
        commands.entity(parts_e).with_children(|p| {
            text(p, "Not a working gun yet: give the firing piece loaders and a barrel.".into(), ThemeFonts::CAPTION, Sheet::LABEL);
        });
        return;
    };

    let lines = part_lines(gun);
    commands.entity(parts_e).with_children(|p| {
        for (name, does) in lines {
            ruled_row(p, vec![GridTrack::flex(1.0), GridTrack::auto()], |r| {
                text(r, name, 12.0, ThemeColors::TEXT_PRIMARY);
                text(r, does, ThemeFonts::TINY, Sheet::LABEL);
            });
        }
        for problem in &gun.problems {
            text(p, problem.clone(), ThemeFonts::TINY, ThemeColors::STATUS_WARN);
        }
    });

    let rows = performance_lines(&stats, before.as_ref());
    commands.entity(numbers_e).with_children(|p| {
        for (name, value, change) in rows {
            ruled_row(p, vec![GridTrack::flex(1.0), GridTrack::px(80.0), GridTrack::px(56.0)], |r| {
                text(r, name.to_string(), 12.0, ThemeColors::TEXT_SECONDARY);
                text(r, value, 12.0, ThemeColors::TEXT_PRIMARY);
                match change {
                    Some((d, better)) => text(r, d, ThemeFonts::TINY, if better { ThemeColors::STATUS_OK } else { ThemeColors::STATUS_DANGER }),
                    None => text(r, "-".into(), ThemeFonts::TINY, Sheet::RULE),
                }
            });
        }
    });

    commands.entity(armour_e).with_children(|p| {
        for (name, thick, bite) in armour_lines(crate::combat::gun_assembly::penetration_for(&stats)) {
            let (verdict, color) = if bite >= 1.0 {
                ("goes through".to_string(), ThemeColors::STATUS_OK)
            } else if bite > 0.0 {
                (format!("{:.0}% bites", bite * 100.0), ThemeColors::STATUS_WARN)
            } else {
                ("stopped".to_string(), ThemeColors::STATUS_DANGER)
            };
            ruled_row(p, vec![GridTrack::flex(1.0), GridTrack::px(30.0), GridTrack::px(96.0)], |r| {
                text(r, name.to_string(), 12.0, ThemeColors::TEXT_SECONDARY);
                text(r, format!("{thick:.0}"), ThemeFonts::TINY, Sheet::LABEL);
                text(r, verdict, ThemeFonts::TINY, color);
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_shows_which_way_it_went() {
        let before = GunStats { damage: 4000.0, rpm: 2.0, velocity_mps: 900.0, range_mult: 1.0, ..default() };
        let now = GunStats { damage: 5800.0, rpm: 2.0, velocity_mps: 810.0, range_mult: 1.0, ..default() };
        let rows = performance_lines(&now, Some(&before));
        let get = |n: &str| rows.iter().find(|r| r.0 == n).unwrap().2.clone();
        assert_eq!(get("Damage a shot"), Some(("+1800".to_string(), true)));
        assert_eq!(get("Velocity"), Some(("-90".to_string(), false)));
        assert_eq!(get("Between shots"), None, "unchanged reads as no change");
    }

    #[test]
    fn nothing_to_compare_on_first_open() {
        let rows = performance_lines(&GunStats::default(), None);
        assert!(rows.iter().all(|r| r.2.is_none()));
    }

    #[test]
    fn small_guns_stop_on_plate_that_big_ones_go_through() {
        // 60 mm and 170 mm as they fly (see combat::impact's tests).
        let sixty = armour_lines(16.5);
        let big = armour_lines(47.0);
        let verdict = |rows: &[(&str, f32, f32)], n: &str| rows.iter().find(|r| r.0 == n).unwrap().2;
        assert_eq!(verdict(&sixty, "Steel hull"), 1.0);
        assert!(verdict(&sixty, "Hard plate") < 0.25);
        assert_eq!(verdict(&big, "Hard plate"), 1.0);
        assert!(verdict(&big, "Thickest hull") < 0.5);
    }
}
