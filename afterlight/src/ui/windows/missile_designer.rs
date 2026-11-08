use bevy::prelude::*;

use crate::combat::missile_design::{
    design_of, launcher_spec, problems, stats, MissileBody, MissileDesign, MissileHead, MissileLoadout, MissileMotor, Seeker,
};
use crate::ui::theme::ThemeColors;
use super::framework::*;
use super::tooltip::Tooltip;
use super::tuning::TuningWindow;

// ============================================================================
// MISSILE DESIGNER
// The MISSILE section of a launcher's tuning window (right-click a Launch
// Pad, Rail Launcher or VLS Silo while docked). The strip reads back to
// front, the way the missile sits on the rail: motor, body parts, head.
// Click a body part to take it out; click the motor or head to change it;
// the buttons below add parts. combat::missile_design has the rules -- the
// Missile Bay preview's, number for number.
// ============================================================================

const ACCENT: Color = ThemeColors::ACCENT_ORANGE;
const CHIP_BG: Color = ThemeColors::BG_INPUT;
const SELECTED_BG: Color = ThemeColors::BG_PRESSED;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissileAction {
    /// Take the body part at this place out.
    RemoveBody(usize),
    CycleMotor,
    CycleHead,
    AddBody(MissileBody),
    Motor(MissileMotor),
    Head(MissileHead),
}

#[derive(Component)]
pub struct MissileButton(pub MissileAction);

/// The row the strip's chips live in; rebuilt when the design changes.
#[derive(Component)]
pub struct MissileStripRow;

#[derive(Component)]
pub struct MissileReadout;

/// What a click does to a design on a launcher that takes `max_parts`.
/// Pure, so the rules are testable.
pub fn apply(design: &mut MissileDesign, action: MissileAction, max_parts: usize) {
    match action {
        MissileAction::RemoveBody(i) if i < design.body.len() => {
            design.body.remove(i);
        }
        MissileAction::RemoveBody(_) => {}
        MissileAction::CycleMotor => {
            let i = MissileMotor::ALL.iter().position(|m| *m == design.motor).unwrap_or(0);
            design.motor = MissileMotor::ALL[(i + 1) % MissileMotor::ALL.len()];
        }
        MissileAction::CycleHead => {
            let i = MissileHead::ALL.iter().position(|h| *h == design.head).unwrap_or(0);
            design.head = MissileHead::ALL[(i + 1) % MissileHead::ALL.len()];
        }
        // New parts go just behind the head, where warheads usually sit.
        MissileAction::AddBody(b) if design.parts() < max_parts => design.body.push(b),
        MissileAction::AddBody(_) => {}
        MissileAction::Motor(m) => design.motor = m,
        MissileAction::Head(h) => design.head = h,
    }
}

fn chip(commands: &mut Commands, label: &str, action: MissileAction, selected: bool, tip: &str) -> Entity {
    let btn = commands
        .spawn((
            Node {
                padding: UiRect::new(Val::Px(6.0), Val::Px(6.0), Val::Px(3.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(if selected { SELECTED_BG } else { CHIP_BG }),
            BorderColor::all(if selected { ACCENT } else { WindowStyle::BORDER_COLOR }),
            Interaction::None,
            Button,
            MissileButton(action),
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

/// The MISSILE section, added to a launcher's tuning window.
pub fn spawn_missile_section(commands: &mut Commands, content: Entity, design: &MissileDesign) {
    spawn_window_section(commands, content, "MISSILE  (back to front - click a part to remove)");

    let strip = row(commands);
    commands.entity(strip).insert(MissileStripRow);
    fill_strip(commands, strip, design);
    commands.entity(content).add_child(strip);

    let add_label = label(commands, "ADD");
    commands.entity(content).add_child(add_label);
    let adds = row(commands);
    for body in MissileBody::ALL {
        let c = chip(commands, &format!("+ {}", body.name()), MissileAction::AddBody(body), false, body.describe());
        commands.entity(adds).add_child(c);
    }
    commands.entity(content).add_child(adds);

    let motor_label = label(commands, "MOTOR");
    commands.entity(content).add_child(motor_label);
    let motors = row(commands);
    for motor in MissileMotor::ALL {
        let c = chip(commands, motor.name(), MissileAction::Motor(motor), motor == design.motor, motor.describe());
        commands.entity(motors).add_child(c);
    }
    commands.entity(content).add_child(motors);

    let head_label = label(commands, "HEAD");
    commands.entity(content).add_child(head_label);
    let heads = row(commands);
    for head in MissileHead::ALL {
        let c = chip(commands, head.name(), MissileAction::Head(head), head == design.head, head.describe());
        commands.entity(heads).add_child(c);
    }
    commands.entity(content).add_child(heads);

    let readout = commands
        .spawn((
            Text::new(""),
            TextFont { font_size: FontSize::Px(11.0), ..default() },
            TextColor(WindowStyle::TEXT_COLOR),
            MissileReadout,
            Node { margin: UiRect::top(Val::Px(4.0)), ..default() },
        ))
        .id();
    commands.entity(content).add_child(readout);
}

fn fill_strip(commands: &mut Commands, strip: Entity, design: &MissileDesign) {
    let mut chips = vec![chip(commands, design.motor.name(), MissileAction::CycleMotor, true, "The motor. Click to change it.")];
    for (i, b) in design.body.iter().enumerate() {
        chips.push(chip(commands, b.name(), MissileAction::RemoveBody(i), false, "Click to take this part out."));
    }
    chips.push(chip(commands, design.head.name(), MissileAction::CycleHead, true, "The head. Click to change it."));
    commands.entity(strip).add_children(&chips);
}

/// Clicks on the missile section: change the launcher's design.
pub fn missile_designer_click(
    mut commands: Commands,
    buttons: Query<(&MissileButton, &Interaction), Changed<Interaction>>,
    windows: Query<&TuningWindow>,
    launchers: Query<(&crate::components::Module, Option<&MissileLoadout>)>,
) {
    let Some(action) = buttons.iter().find(|(_, i)| **i == Interaction::Pressed).map(|(b, _)| b.0) else { return };
    let Ok(tw) = windows.single() else { return };
    let Ok((module, loadout)) = launchers.get(tw.module_entity) else { return };
    let Some(spec) = launcher_spec(module.module_type) else { return };
    let mut design = design_of(module.module_type, loadout);
    apply(&mut design, action, spec.max_parts);
    // Inserting marks it changed: fit_missile_loadouts re-works the launcher.
    commands.entity(tw.module_entity).try_insert(MissileLoadout(design));
}

/// Keep the section showing the design and what it adds up to.
#[allow(clippy::type_complexity)]
pub fn missile_designer_refresh(
    mut commands: Commands,
    windows: Query<&TuningWindow>,
    launchers: Query<(&crate::components::Module, Option<&MissileLoadout>)>,
    strip: Query<(Entity, Option<&Children>), With<MissileStripRow>>,
    mut buttons: Query<(&MissileButton, &mut BackgroundColor, &mut BorderColor, &Children)>,
    mut text_colors: Query<&mut TextColor>,
    mut readout: Query<&mut Text, With<MissileReadout>>,
    mut shown: Local<Option<(Entity, MissileDesign)>>,
) {
    let Ok(tw) = windows.single() else { return };
    let Ok((module, loadout)) = launchers.get(tw.module_entity) else { return };
    let Some(spec) = launcher_spec(module.module_type) else { return };
    let design = design_of(module.module_type, loadout);
    if shown.as_ref() == Some(&(tw.module_entity, design.clone())) {
        return;
    }
    if let Ok((row, children)) = strip.single() {
        for c in children.into_iter().flatten() {
            commands.entity(*c).despawn();
        }
        fill_strip(&mut commands, row, &design);
    }
    for (btn, mut bg, mut border, children) in buttons.iter_mut() {
        let selected = match btn.0 {
            MissileAction::Motor(m) => m == design.motor,
            MissileAction::Head(h) => h == design.head,
            _ => continue,
        };
        *bg = BackgroundColor(if selected { SELECTED_BG } else { CHIP_BG });
        *border = BorderColor::all(if selected { ACCENT } else { WindowStyle::BORDER_COLOR });
        for c in children.iter() {
            if let Ok(mut tc) = text_colors.get_mut(c) {
                tc.0 = if selected { ACCENT } else { WindowStyle::TEXT_COLOR };
            }
        }
    }
    *shown = Some((tw.module_entity, design.clone()));

    let Ok(mut text) = readout.single_mut() else { return };
    let s = stats(&design, spec);
    let mut warheads = Vec::new();
    if s.he_damage > 0.0 {
        warheads.push(format!("HE {:.0} over {:.1} blocks", s.he_damage, s.he_radius / 66.0));
    }
    if s.frag_damage > 0.0 {
        warheads.push(format!("frag {:.0} over {:.1}", s.frag_damage, s.frag_radius / 66.0));
    }
    if s.shaped_damage > 0.0 {
        warheads.push(format!("shaped jet {:.0}", s.shaped_damage));
    }
    if s.emp_radius > 0.0 {
        warheads.push(format!("EMP over {:.1}", s.emp_radius / 66.0));
    }
    let seeker = match s.seeker {
        Seeker::Heat { range, .. } => format!("heat {range:.0} u, fooled by decoys {:.0}%", s.fool_chance * 100.0),
        Seeker::Radar { range, .. } => format!("radar {range:.0} u, fooled by chaff {:.0}%", s.fool_chance * 100.0),
        Seeker::CommandLink { range } => format!("command link {range:.0} u, follows your aim"),
        Seeker::None => "none: flies where it was aimed".into(),
    };
    let mut lines = vec![
        format!("{} {:.0} mm  {}  {}/{} parts", spec.name, spec.gauge_mm, design.strip(), s.parts, spec.max_parts),
        format!(
            "TOP {:.0} u/s   REACH {:.0} u   TURN {:.1} blocks   RELOAD {:.1} s",
            s.top_speed, s.reach, s.turn_circle / 66.0, s.reload_s
        ),
        format!(
            "WARHEAD {}   FUSE {}",
            if warheads.is_empty() { format!("none (rams for {:.0})", s.ram_damage * s.top_speed / 1000.0) } else { warheads.join(", ") },
            if s.proximity { "proximity" } else { "contact" }
        ),
        format!("SEEKER {}   TOUGHNESS {:.0}", seeker, s.toughness),
    ];
    if let Some(p) = problems(&design, &s).first() {
        lines.push(p.clone());
    }
    text.0 = lines.join("\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::ModuleType;

    #[test]
    fn a_part_only_goes_on_if_the_launcher_has_room() {
        let mut d = MissileDesign::default_for(ModuleType::LaunchPad);
        assert_eq!(d.parts(), 4);
        apply(&mut d, MissileAction::AddBody(MissileBody::Fuel), 4);
        assert_eq!(d.parts(), 4, "a Launch Pad takes four");
        apply(&mut d, MissileAction::RemoveBody(0), 4);
        apply(&mut d, MissileAction::AddBody(MissileBody::Fuel), 4);
        assert_eq!(d.body.last(), Some(&MissileBody::Fuel));
    }

    #[test]
    fn clicking_the_head_and_motor_cycles_them() {
        let mut d = MissileDesign::default_for(ModuleType::RailLauncher);
        let (head, motor) = (d.head, d.motor);
        apply(&mut d, MissileAction::CycleHead, 6);
        apply(&mut d, MissileAction::CycleMotor, 6);
        assert_ne!(d.head, head);
        assert_ne!(d.motor, motor);
    }
}
