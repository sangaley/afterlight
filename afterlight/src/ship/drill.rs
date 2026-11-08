use bevy::prelude::*;
use rand::Rng;

use crate::components::*;
use crate::events::*;
use crate::resources::*;
use crate::ai_ship::components::AiShipWreck;

// ============================================================================
// BREAKER DRILL — contact wreck salvage, the industrial alternative to
// EVA crew (crew::eva_salvage). Park the drill block against a hulk and
// it chews the nearest wreck block in reach: nobody at risk, faster per
// block, loot lands straight in the hold — but it needs power, an
// operator at its station (unmanned stations don't run), and it's LOUD
// (see systems::update_ship_state): strip-mining a wreck advertises
// your position to everything with ears.
// ============================================================================

const CHEW_SECONDS: f32 = 1.5;
const SPARK_INTERVAL: f32 = 0.22;
/// Noise added per actively chewing drill — louder than cruising engines.
pub const DRILL_NOISE: f32 = 35.0;

/// Runtime state of one drill module (lazily attached to anything with
/// a SalvageSystem — the Breaker Drill, and the Mineral Extractor until
/// asteroid mining exists).
#[derive(Component, Default)]
pub struct DrillRig {
    pub target: Option<Entity>,
    pub progress: f32,
    pub spark_timer: f32,
}

pub fn wreck_drill_system(
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    time: Res<Time>,
    mut commands: Commands,
    power_state: Res<PowerState>,
    ship_query: Query<Entity, With<Ship>>,
    mut drill_query: Query<(
        Entity,
        &Module,
        &SalvageSystem,
        &GlobalTransform,
        Option<&ModuleEfficiency>,
        Option<&mut DrillRig>,
        &ChildOf,
    )>,
    block_query: Query<
        (Entity, &GlobalTransform, &Sprite, &ChildOf),
        (Or<(With<Module>, With<HullSegment>)>, Without<SalvageSystem>, Without<CrewMember>),
    >,
    mut wreck_query: Query<(&mut Wreck, &mut PointOfInterest, Option<&AiShipWreck>)>,
    children_query: Query<&Children>,
    mut inventory: ResMut<Inventory>,
    mut statistics: ResMut<Statistics>,
    mut notifications: MessageWriter<ShowNotification>,
    mut full_warn_cooldown: Local<f32>,
) {
    let Ok(player_ship) = ship_query.single() else { return };
    let dt = time.delta_secs();
    *full_warn_cooldown = (*full_warn_cooldown - dt).max(0.0);
    let mut rng = rand::thread_rng();

    let generic_loot = [
        ItemType::ScrapMetal,
        ItemType::Crystal,
        ItemType::FuelCell,
        ItemType::RareAlloy,
        ItemType::AmmoCrate,
    ];

    for (entity, module, salvage, drill_gt, efficiency, rig, parent) in drill_query.iter_mut() {
        if parent.parent() != player_ship {
            continue;
        }

        // Lazily attach runtime state the first time we see a drill.
        let Some(rig) = rig else {
            commands.entity(entity).try_insert(DrillRig::default());
            continue;
        };
        let rig = rig.into_inner();

        // Powered, active, and staffed — an unmanned drill doesn't spin.
        let speed = efficiency.map(|e| e.value).unwrap_or(1.0) * salvage.efficiency;
        if !module.is_active || power_state.power_balance < 0.0 || speed <= 0.0 {
            rig.target = None;
            rig.progress = 0.0;
            continue;
        }

        let drill_pos = drill_gt.translation().truncate();

        // Hulls are solid now (see ship::collision), which holds a skin gap
        // between ship and wreck — pad the registry's contact range so the
        // drill still reaches across that gap when parked against the hulk.
        let reach = salvage.range + 300.0;

        // Validate the current target, else acquire the nearest wreck
        // block in reach (wreck blocks only — never live ships).
        let mut target = rig.target.filter(|&t| {
            block_query.get(t).is_ok_and(|(_, gt, _, block_parent)| {
                wreck_query.get(block_parent.parent()).is_ok()
                    && gt.translation().truncate().distance(drill_pos) <= reach
            })
        });
        if target.is_none() {
            rig.progress = 0.0;
            let mut best_dist = reach;
            for (block_entity, gt, _, block_parent) in block_query.iter() {
                if wreck_query.get(block_parent.parent()).is_err() {
                    continue;
                }
                let dist = gt.translation().truncate().distance(drill_pos);
                if dist <= best_dist {
                    best_dist = dist;
                    target = Some(block_entity);
                }
            }
            rig.target = target;
        }
        let Some(block_entity) = target else { continue };
        let Ok((_, block_gt, block_sprite, block_parent)) = block_query.get(block_entity) else {
            continue;
        };
        let block_pos = block_gt.translation().truncate();
        let wreck_root = block_parent.parent();

        // CHEW — sparks fly at the contact point while the grinder works.
        rig.progress += dt * speed / CHEW_SECONDS;
        rig.spark_timer -= dt;
        if rig.spark_timer <= 0.0 {
            rig.spark_timer = SPARK_INTERVAL;
            let contact = block_pos + (drill_pos - block_pos) * 0.4
                + Vec2::new(rng.gen_range(-8.0..8.0), rng.gen_range(-8.0..8.0));
            crate::combat::spawn_hit_effect(
                &mut commands,
                contact,
                Color::srgb(1.0, 0.75, 0.3),
                rng.gen_range(12.0..22.0),
            );
        }
        if rig.progress < 1.0 {
            continue;
        }
        rig.progress = 0.0;
        rig.target = None;

        // Block ground down — bank the haul straight into the hold.
        let Ok((mut wreck, mut poi, ai_wreck)) = wreck_query.get_mut(wreck_root) else {
            continue;
        };
        let item = if wreck.loot_remaining > 0 {
            match ai_wreck {
                Some(aw) => crate::ai_ship::wreck::roll_wreck_loot(aw.ship_type, aw.intact_frac, &mut rng),
                None => generic_loot[rng.gen_range(0..generic_loot.len())],
            }
        } else {
            // Cargo's gone — the hull metal itself is the haul.
            ItemType::ScrapMetal
        };

        if !inventory.add_item(item, 1) {
            // Hold full: leave the block standing, idle the drill.
            if *full_warn_cooldown <= 0.0 {
                *full_warn_cooldown = 5.0;
                notifications.write(ShowNotification {
                    message: "Cargo hold full - drill idle.".into(),
                    notification_type: NotificationType::Warning,
                    duration: 3.0,
                });
            }
            continue;
        }
        if wreck.loot_remaining > 0 {
            wreck.loot_remaining -= 1;
            if wreck.loot_remaining == 0 {
                poi.discovered = true;
                wreck.is_explored = true;
                statistics.wrecks_salvaged += 1;
                notifications.write(ShowNotification {
                    message: "Cargo stripped - drill grinding hull to scrap.".into(),
                    notification_type: NotificationType::Info,
                    duration: 3.0,
                });
            }
        }
        notifications.write(ShowNotification {
            message: format!("Drill recovered {}", item.name()),
            notification_type: NotificationType::Success,
            duration: 1.5,
        });

        // The block physically comes off the hulk.
        crate::vfx::debris::spawn_chunks(&mut commands, &fx, &mut rng, block_pos, block_sprite.color, Vec2::ZERO);
        commands.entity(block_entity).try_despawn();

        // Last block gone? The wreck ceases to exist.
        let blocks_left = children_query
            .get(wreck_root)
            .map(|children| {
                children
                    .iter()
                    .filter(|c| *c != block_entity && block_query.get(*c).is_ok())
                    .count()
            })
            .unwrap_or(0);
        if blocks_left == 0 {
            commands.entity(wreck_root).try_despawn();
            notifications.write(ShowNotification {
                message: "Wreck fully dismantled.".into(),
                notification_type: NotificationType::Success,
                duration: 3.0,
            });
        }
    }
}
