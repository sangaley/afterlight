use bevy::prelude::*;

use crate::combat::gun_assembly::GunAssembly;
use crate::combat::shell::{GunShell, ShellBase, ShellBody, ShellDesign, ShellHead, MAX_SHELL_MODULES};
use crate::ui::theme::ThemeColors;
use super::framework::*;
use super::tooltip::Tooltip;
use super::tuning::TuningWindow;

// ============================================================================
// SHELL DESIGNER
// The SHELL section of a built gun's tuning window (right-click its Firing
// Piece while docked). The strip reads back to front, the way the shell sits
// in the loader: base, casings, bodies, head. Click a casing or body to take
// it out; click the head or base to change it; the buttons below add
// modules. combat::shell has the rules, combat::gun_math the numbers.
// ============================================================================

// Sheet chips: no fill until picked; the picked one gold.
const ACCENT: Color = crate::ui::theme::Sheet::PICKED;
const CHIP_BG: Color = Color::NONE;
const SELECTED_BG: Color = crate::ui::theme::Sheet::ROW_PICKED;

/// Where a strip chip sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripSlot {
    Base,
    Casing,
    Body(usize),
    Head,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellAction {
    /// Take a casing or body out; cycle the head or base.
    Strip(StripSlot),
    AddCasing,
    AddBody(ShellBody),
    Head(ShellHead),
    Base(ShellBase),
}

#[derive(Component)]
pub struct ShellButton(pub ShellAction);

/// The row the strip's chips live in; rebuilt when the design changes.
#[derive(Component)]
pub struct ShellStripRow;

#[derive(Component)]
pub struct ShellReadout;

/// Saves the gun, parts and shell, as a design in designs/guns.
#[derive(Component)]
pub struct SaveGunDesignButton;

/// Puts the gun on point defence, or takes it off (combat::missile_defence).
#[derive(Component)]
pub struct PointDefenceButton;

fn point_defence_label(on: bool) -> &'static str {
    if on { "POINT DEFENCE: ON - missiles only" } else { "POINT DEFENCE: OFF" }
}

/// What a click does to a design. Pure, so the rules are testable.
pub fn apply(design: &mut ShellDesign, action: ShellAction) {
    match action {
        ShellAction::Strip(StripSlot::Casing) => design.casings = design.casings.saturating_sub(1),
        ShellAction::Strip(StripSlot::Body(i)) if i < design.bodies.len() => {
            design.bodies.remove(i);
        }
        ShellAction::Strip(StripSlot::Body(_)) => {}
        ShellAction::Strip(StripSlot::Head) => {
            let i = ShellHead::ALL.iter().position(|h| *h == design.head).unwrap_or(0);
            design.head = ShellHead::ALL[(i + 1) % ShellHead::ALL.len()];
        }
        ShellAction::Strip(StripSlot::Base) | ShellAction::Base(_) if !design.has_room() && design.base == ShellBase::None => {}
        ShellAction::Strip(StripSlot::Base) => {
            let i = ShellBase::ALL.iter().position(|b| *b == design.base).unwrap_or(0);
            design.base = ShellBase::ALL[(i + 1) % ShellBase::ALL.len()];
        }
        ShellAction::AddCasing if design.has_room() => design.casings += 1,
        ShellAction::AddBody(b) if design.has_room() => {
            // New bodies go just behind the head.
            design.bodies.push(b);
        }
        ShellAction::AddCasing | ShellAction::AddBody(_) => {}
        ShellAction::Head(h) => design.head = h,
        ShellAction::Base(b) => design.base = b,
    }
}

fn chip(commands: &mut Commands, label: &str, action: ShellAction, selected: bool, tip: &str) -> Entity {
    let btn = commands
        .spawn((
            Node {
                padding: UiRect::new(Val::Px(6.0), Val::Px(6.0), Val::Px(3.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(if selected { SELECTED_BG } else { CHIP_BG }),
            BorderColor::all(if selected { ACCENT } else { crate::ui::theme::Sheet::RULE }),
            Interaction::None,
            Button,
            ShellButton(action),
            Tooltip { text: tip.into(), detail: None },
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label),
            TextFont { font_size: FontSize::Px(10.0), ..default() },
            TextColor(if selected { ACCENT } else { WindowStyle::TEXT_COLOR }),
        ))
        .id();
    commands.entity(btn).add_child(text);
    btn
}

fn row(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(3.0),
            row_gap: Val::Px(3.0),
            margin: UiRect::vertical(Val::Px(3.0)),
            ..default()
        })
        .id()
}

fn label(commands: &mut Commands, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text),
            TextFont { font_size: FontSize::Px(10.0), ..default() },
            TextColor(WindowStyle::TEXT_DIM),
        ))
        .id()
}

/// The SHELL section, added to a built gun's tuning window.
pub fn spawn_shell_section(commands: &mut Commands, content: Entity, design: &ShellDesign) {
    spawn_window_section(commands, content, "SHELL  (back to front - click to remove)");

    let strip = row(commands);
    commands.entity(strip).insert(ShellStripRow);
    fill_strip(commands, strip, design);
    commands.entity(content).add_child(strip);

    let add_label = label(commands, "ADD");
    commands.entity(content).add_child(add_label);
    let adds = row(commands);
    let casing = chip(commands, "+ Casing", ShellAction::AddCasing, false, "Propellant, half a gauge long: faster, but needs more barrel and kicks harder.");
    commands.entity(adds).add_child(casing);
    for body in ShellBody::ALL {
        let tip = match body {
            ShellBody::Solid => "Solid steel: weight behind the head. More damage and penetration, a slower shell.",
            ShellBody::He => "Explosive: a bigger blast where it lands.",
            ShellBody::Frag => "Fragmentation: harms crew, spares the plate.",
            ShellBody::Emp => "EMP: knocks out systems around the hit for longer, further out.",
            ShellBody::Incendiary => "Incendiary: starts a fire.",
            ShellBody::ProximityFuse => "Proximity fuse: bursts beside an enemy missile instead of having to hit it. Flak: load frag or HE with it.",
        };
        let c = chip(commands, &format!("+ {}", body.name()), ShellAction::AddBody(body), false, tip);
        commands.entity(adds).add_child(c);
    }
    commands.entity(content).add_child(adds);

    let head_label = label(commands, "HEAD");
    commands.entity(content).add_child(head_label);
    let heads = row(commands);
    for head in ShellHead::ALL {
        let c = chip(commands, head.name(), ShellAction::Head(head), head == design.head, head.description());
        commands.entity(heads).add_child(c);
    }
    commands.entity(content).add_child(heads);

    let base_label = label(commands, "BASE");
    commands.entity(content).add_child(base_label);
    let bases = row(commands);
    for base in ShellBase::ALL {
        let tip = match base {
            ShellBase::None => "Nothing behind the casings.",
            ShellBase::Tracer => "A tracer: the gunners see where it goes. A quarter less spread.",
            ShellBase::Bleeder => "A base bleeder: less drag behind it. A fifth more range.",
        };
        let c = chip(commands, base.name(), ShellAction::Base(base), base == design.base, tip);
        commands.entity(bases).add_child(c);
    }
    commands.entity(content).add_child(bases);

    let readout = commands
        .spawn((
            Text::new(""),
            TextFont { font_size: FontSize::Px(11.0), ..default() },
            TextColor(WindowStyle::TEXT_COLOR),
            ShellReadout,
            Node { margin: UiRect::top(Val::Px(4.0)), ..default() },
        ))
        .id();
    commands.entity(content).add_child(readout);

    spawn_point_defence_switch(commands, content);

    let save = commands
        .spawn((
            Node {
                padding: UiRect::new(Val::Px(10.0), Val::Px(10.0), Val::Px(4.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                margin: UiRect::top(Val::Px(6.0)),
                align_self: AlignSelf::FlexStart,
                ..default()
            },
            BackgroundColor(CHIP_BG),
            BorderColor::all(ACCENT),
            Interaction::None,
            Button,
            SaveGunDesignButton,
            Tooltip {
                text: "Save this gun -- its parts and its shell -- as a design.".into(),
                detail: Some("Build mode: T picks a saved design, Enter stamps it.".into()),
            },
        ))
        .id();
    let save_label = commands
        .spawn((
            Text::new("SAVE AS GUN DESIGN"),
            TextFont { font_size: FontSize::Px(11.0), ..default() },
            TextColor(ACCENT),
        ))
        .id();
    commands.entity(save).add_child(save_label);
    commands.entity(content).add_child(save);
}

/// The POINT DEFENCE switch, for a built gun's window or a laser emitter's.
pub fn spawn_point_defence_switch(commands: &mut Commands, content: Entity) {
    let pd = commands
        .spawn((
            Node {
                padding: UiRect::new(Val::Px(10.0), Val::Px(10.0), Val::Px(4.0), Val::Px(4.0)),
                border: UiRect::all(Val::Px(1.0)),
                margin: UiRect::top(Val::Px(6.0)),
                align_self: AlignSelf::FlexStart,
                ..default()
            },
            BackgroundColor(CHIP_BG),
            BorderColor::all(WindowStyle::BORDER_COLOR),
            Interaction::None,
            Button,
            PointDefenceButton,
            Tooltip {
                text: "On point defence it fires by itself at the incoming missile that will land first -- and never at ships. On a turret, it swings the turret onto it.".into(),
                detail: Some("It only sees missiles the ship's Missile Warners have spotted.".into()),
            },
        ))
        .id();
    let pd_label = commands
        .spawn((
            Text::new(point_defence_label(false)),
            TextFont { font_size: FontSize::Px(11.0), ..default() },
            TextColor(WindowStyle::TEXT_COLOR),
        ))
        .id();
    commands.entity(pd).add_child(pd_label);
    commands.entity(content).add_child(pd);
}

/// The point defence switch: on or off for the gun under the window.
pub fn point_defence_click(
    mut commands: Commands,
    buttons: Query<&Interaction, (With<PointDefenceButton>, Changed<Interaction>)>,
    windows: Query<&TuningWindow>,
    on: Query<(), With<crate::combat::targeting::fire_groups::InterceptMode>>,
) {
    if !buttons.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Ok(tw) = windows.single() else { return };
    if on.contains(tw.module_entity) {
        commands.entity(tw.module_entity).remove::<crate::combat::targeting::fire_groups::InterceptMode>();
    } else {
        commands.entity(tw.module_entity).try_insert(crate::combat::targeting::fire_groups::InterceptMode);
    }
}

/// Keep the switch saying what the gun is set to.
pub fn point_defence_refresh(
    windows: Query<&TuningWindow>,
    on: Query<(), With<crate::combat::targeting::fire_groups::InterceptMode>>,
    mut buttons: Query<(&mut BorderColor, &Children), With<PointDefenceButton>>,
    mut texts: Query<(&mut Text, &mut TextColor)>,
) {
    let Ok(tw) = windows.single() else { return };
    let set = on.contains(tw.module_entity);
    for (mut border, children) in buttons.iter_mut() {
        *border = BorderColor::all(if set { ACCENT } else { WindowStyle::BORDER_COLOR });
        for c in children.iter() {
            if let Ok((mut text, mut colour)) = texts.get_mut(c) {
                if text.0 != point_defence_label(set) {
                    text.0 = point_defence_label(set).into();
                }
                colour.0 = if set { ACCENT } else { WindowStyle::TEXT_COLOR };
            }
        }
    }
}

/// The save button: write the gun under the window to designs/guns.
pub fn save_gun_design_click(
    buttons: Query<&Interaction, (With<SaveGunDesignButton>, Changed<Interaction>)>,
    windows: Query<&TuningWindow>,
    guns: Query<&GunAssembly>,
    shells: Query<&GunShell>,
    modules: Query<&crate::components::Module>,
    mut designs: ResMut<crate::combat::gun_design::GunDesigns>,
    mut notifications: MessageWriter<crate::events::ShowNotification>,
) {
    if !buttons.iter().any(|i| *i == Interaction::Pressed) {
        return;
    }
    let Ok(tw) = windows.single() else { return };
    let Ok(gun) = guns.get(tw.module_entity) else { return };
    let blocks: Vec<crate::components::Module> = gun.members.iter().filter_map(|e| modules.get(*e).ok().cloned()).collect();
    let shell = shells.get(tw.module_entity).ok().map(|s| s.0.clone());
    let head = shell.as_ref().map(|s| s.head).unwrap_or_default();
    let name = format!("{:.0} mm {}", gun.stats.gauge_mm, head.name());
    let Some(design) = crate::combat::gun_design::GunDesign::capture(name, &blocks, shell) else { return };
    let (message, kind) = match crate::combat::gun_design::write_gun_design(&design) {
        Ok(path) => {
            designs.0 = crate::combat::gun_design::read_gun_designs();
            info!("Gun design saved to {:?}", path);
            (format!("Saved gun design '{}' ({})", design.name, path.display()), crate::events::NotificationType::Success)
        }
        Err(e) => (format!("Couldn't save the gun design: {e}"), crate::events::NotificationType::Danger),
    };
    notifications.write(crate::events::ShowNotification { message, notification_type: kind, duration: 3.5 });
}

fn fill_strip(commands: &mut Commands, strip: Entity, design: &ShellDesign) {
    let mut chips = Vec::new();
    if design.base != ShellBase::None {
        chips.push(chip(commands, design.base.name(), ShellAction::Strip(StripSlot::Base), false, "Click to change the base."));
    }
    for _ in 0..design.casings {
        chips.push(chip(commands, "casing", ShellAction::Strip(StripSlot::Casing), false, "Click to take a casing out."));
    }
    for (i, b) in design.bodies.iter().enumerate() {
        chips.push(chip(commands, b.name(), ShellAction::Strip(StripSlot::Body(i)), false, "Click to take this body out."));
    }
    chips.push(chip(commands, design.head.name(), ShellAction::Strip(StripSlot::Head), true, "The head. Click to change it."));
    commands.entity(strip).add_children(&chips);
}

/// Clicks on the shell section: change the gun's design.
pub fn shell_designer_click(
    mut commands: Commands,
    buttons: Query<(&ShellButton, &Interaction), Changed<Interaction>>,
    windows: Query<&TuningWindow>,
    shells: Query<&GunShell>,
) {
    let Some(action) = buttons.iter().find(|(_, i)| **i == Interaction::Pressed).map(|(b, _)| b.0) else { return };
    let Ok(tw) = windows.single() else { return };
    let mut design = shells.get(tw.module_entity).map(|s| s.0.clone()).unwrap_or_default();
    apply(&mut design, action);
    // Inserting marks it changed: combat::gun_assembly re-works the gun out.
    commands.entity(tw.module_entity).try_insert(GunShell(design));
}

/// Keep the section showing the design and what it makes of the gun.
pub fn shell_designer_refresh(
    mut commands: Commands,
    windows: Query<&TuningWindow>,
    shells: Query<&GunShell>,
    guns: Query<&GunAssembly>,
    strip: Query<(Entity, Option<&Children>), With<ShellStripRow>>,
    mut buttons: Query<(&ShellButton, &mut BackgroundColor, &mut BorderColor, &Children)>,
    mut text_colors: Query<&mut TextColor>,
    mut readout: Query<&mut Text, With<ShellReadout>>,
    mut shown: Local<Option<(Entity, ShellDesign)>>,
) {
    let Ok(tw) = windows.single() else { return };
    let design = shells.get(tw.module_entity).map(|s| s.0.clone()).unwrap_or_default();

    if shown.as_ref() != Some(&(tw.module_entity, design.clone())) {
        if let Ok((row, children)) = strip.single() {
            for c in children.into_iter().flatten() {
                commands.entity(*c).despawn();
            }
            fill_strip(&mut commands, row, &design);
        }
        for (btn, mut bg, mut border, children) in buttons.iter_mut() {
            let selected = match btn.0 {
                ShellAction::Head(h) => h == design.head,
                ShellAction::Base(b) => b == design.base,
                _ => continue,
            };
            *bg = BackgroundColor(if selected { SELECTED_BG } else { CHIP_BG });
            *border = BorderColor::all(if selected { ACCENT } else { crate::ui::theme::Sheet::RULE });
            for c in children.iter() {
                if let Ok(mut tc) = text_colors.get_mut(c) {
                    tc.0 = if selected { ACCENT } else { WindowStyle::TEXT_COLOR };
                }
            }
        }
        *shown = Some((tw.module_entity, design.clone()));
    }

    let Ok(mut text) = readout.single_mut() else { return };
    let Ok(gun) = guns.get(tw.module_entity) else {
        text.0 = "Not a working gun yet: give the firing piece loaders and a barrel.".into();
        return;
    };
    let s = &gun.stats;
    let longest = gun.parts.loaders.iter().map(|l| l.length_m).max().unwrap_or(0);
    let p = design.payload();
    let mut payload = Vec::new();
    for (n, name) in [(p.he, "HE"), (p.frag, "frag"), (p.emp, "EMP"), (p.incendiary, "fire")] {
        if n > 0.0 {
            payload.push(format!("{n:.0} {name}"));
        }
    }
    let mut lines = vec![
        format!(
            "{:.0} mm  {}  {}/{} modules",
            s.gauge_mm,
            design.strip(),
            design.modules(),
            MAX_SHELL_MODULES
        ),
        format!(
            "LENGTH {:.2} m  (longest loader {} m)   BURN {:.1} m of barrel ({} m)",
            s.shell_m, longest, s.burn_m, gun.parts.barrel_m
        ),
        format!(
            "VEL {:.0} m/s   DMG {:.0}   PEN {:.1}   {:.1} rpm",
            s.velocity_mps, s.damage, crate::combat::gun_assembly::penetration_for(s), s.rpm
        ),
        format!(
            "PAYLOAD {}   ON HIT: {}",
            if payload.is_empty() { "none".to_string() } else { payload.join(", ") },
            on_hit(&design)
        ),
    ];
    if let Some(problem) = gun.problems.first() {
        lines.push(problem.clone());
    }
    text.0 = lines.join("\n");
}

/// One line on what the shell does when it lands.
pub fn on_hit(design: &ShellDesign) -> &'static str {
    if design.has_proximity_fuse() {
        return "flak: bursts beside missiles";
    }
    match (design.head, design.dominant_payload()) {
        (ShellHead::Ap, Some(ShellBody::He)) => "punches in, then bursts",
        (_, Some(ShellBody::He)) => "bursts on the surface",
        (_, Some(ShellBody::Frag)) => "shreds crew",
        (_, Some(ShellBody::Emp)) => "knocks out systems",
        (_, Some(ShellBody::Incendiary)) => "starts a fire",
        (ShellHead::Heat, _) => "burns through armour at any angle",
        _ => "punches through",
    }
}

#[cfg(test)]
mod shell_designer_tests {
    use super::*;

    #[test]
    fn clicks_edit_the_strip() {
        let mut d = ShellDesign::default();
        apply(&mut d, ShellAction::AddCasing);
        assert_eq!(d.casings, 3);
        apply(&mut d, ShellAction::AddBody(ShellBody::He));
        assert_eq!(d.bodies, vec![ShellBody::Solid, ShellBody::He]);
        apply(&mut d, ShellAction::Strip(StripSlot::Body(0)));
        assert_eq!(d.bodies, vec![ShellBody::He]);
        apply(&mut d, ShellAction::Strip(StripSlot::Casing));
        assert_eq!(d.casings, 2);
        apply(&mut d, ShellAction::Strip(StripSlot::Head));
        assert_eq!(d.head, ShellHead::Heat);
        apply(&mut d, ShellAction::Base(ShellBase::Tracer));
        assert_eq!(d.base, ShellBase::Tracer);
    }

    #[test]
    fn a_click_redesigns_the_gun_in_the_open_window() {
        let mut app = App::new();
        app.add_systems(Update, shell_designer_click);
        let fp = app.world_mut().spawn_empty().id();
        app.world_mut().spawn(TuningWindow { module_entity: fp, module_type: crate::components::ModuleType::FiringPiece });
        app.world_mut().spawn((ShellButton(ShellAction::Head(ShellHead::He)), Interaction::Pressed));
        app.update();
        let shell = app.world().get::<GunShell>(fp).expect("the gun has a designed shell now");
        assert_eq!(shell.0.head, ShellHead::He);
        assert_eq!(shell.0.casings, 2, "the rest of the default design kept");
    }

    #[test]
    fn the_save_button_writes_the_gun_and_its_shell_as_a_design() {
        use crate::combat::gun_design::{read_gun_designs, GunDesigns};
        use crate::components::{Module, ModuleType, Rotation};
        let dir = std::env::temp_dir().join(format!("depths_gun_designs_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("AFTERLIGHT_GUN_DESIGN_DIR", &dir);

        let mut app = App::new();
        app.add_message::<crate::events::ShowNotification>();
        app.init_resource::<GunDesigns>();
        app.add_systems(Update, save_gun_design_click);
        let m = |t, x, y| Module {
            module_type: t, health: 1.0, max_health: 1.0, power_consumption: 0.0, power_generation: 0.0,
            is_active: true, grid_position: IVec2::new(x, y), size: IVec2::ONE, rotation: Rotation::South,
        };
        let barrel = app.world_mut().spawn(m(ModuleType::GunBarrel, 3, 2)).id();
        let fp = app.world_mut().spawn(m(ModuleType::FiringPiece, 3, 3)).id();
        let shell = ShellDesign { head: ShellHead::He, ..default() };
        app.world_mut().entity_mut(fp).insert((
            GunAssembly { members: vec![fp, barrel], ..default() },
            GunShell(shell.clone()),
        ));
        app.world_mut().spawn(TuningWindow { module_entity: fp, module_type: ModuleType::FiringPiece });
        app.world_mut().spawn((SaveGunDesignButton, Interaction::Pressed));
        app.update();

        let saved = read_gun_designs();
        std::env::remove_var("AFTERLIGHT_GUN_DESIGN_DIR");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].shell, Some(shell));
        assert!(saved[0].parts.iter().any(|p| p.module_type == ModuleType::GunBarrel && p.offset == IVec2::new(1, 0)), "{:?}", saved[0].parts);
        assert_eq!(app.world().resource::<GunDesigns>().0.len(), 1, "and the list knows about it");
    }

    #[test]
    fn a_full_strip_takes_no_more() {
        let mut d = ShellDesign::default();
        for _ in 0..40 {
            apply(&mut d, ShellAction::AddCasing);
        }
        assert_eq!(d.modules(), MAX_SHELL_MODULES);
        apply(&mut d, ShellAction::AddBody(ShellBody::Solid));
        assert_eq!(d.modules(), MAX_SHELL_MODULES);
        apply(&mut d, ShellAction::Base(ShellBase::Tracer));
        assert_eq!(d.base, ShellBase::None, "no room for a base either");
    }
}
