use bevy::prelude::*;
use rand::Rng;

use crate::components::*;
use crate::events::*;
use crate::resources::ItemType;
use super::components::*;

/// LOOT IDENTITY — what a wreck yields depends on who you killed and how.
/// Each faction has signature cargo (a The Silence carries intel, a Recursive Kingdom
/// carries junk); scrap metal is the filler that survives any kill. The
/// forensic record then biases composition: a shattered hulk's delicate
/// cargo is slag (non-scrap weights crushed), a pristine kill preserves and
/// even concentrates it. Thresholds match the quantity tiers in
/// ai_ship_death_system (>= 0.7 pristine, < 0.3 shattered).
pub fn roll_wreck_loot(ship_type: AiShipType, intact_frac: f32, rng: &mut impl Rng) -> ItemType {
    use ItemType::*;
    let table: &[(ItemType, f32)] = match ship_type {
        AiShipType::RecursiveKingdom => &[(ScrapMetal, 8.0), (FuelCell, 1.0), (AmmoCrate, 1.0)],
        AiShipType::TerranHegemony => &[(ScrapMetal, 4.0), (RareAlloy, 3.0), (AmmoCrate, 3.0)],
        AiShipType::GildedThrone => &[(AmmoCrate, 4.0), (FuelCell, 3.0), (ScrapMetal, 2.0), (RareAlloy, 1.0)],
        AiShipType::CorpseStars => &[(RareAlloy, 4.0), (Crystal, 3.0), (ScrapMetal, 2.0), (FuelCell, 1.0)],
        AiShipType::TheSilence => &[(Crystal, 4.0), (AncientArtifact, 3.0), (FuelCell, 2.0), (ScrapMetal, 1.0)],
        AiShipType::BrokenChoir => &[(AncientArtifact, 4.0), (ScrapMetal, 3.0), (Crystal, 2.0), (BioSample, 1.0)],
        AiShipType::SynthesisCollective => &[(BioSample, 4.0), (AncientArtifact, 2.0), (Crystal, 2.0), (ScrapMetal, 2.0)],
        AiShipType::StellarPreserve => &[(BioSample, 5.0), (Crystal, 2.0), (ScrapMetal, 2.0), (RareAlloy, 1.0)],
        AiShipType::EternalHegemony => &[(AmmoCrate, 4.0), (RareAlloy, 3.0), (ScrapMetal, 2.0), (FuelCell, 1.0)],
        AiShipType::Shepherd => &[(AncientArtifact, 3.0), (RareAlloy, 3.0), (Crystal, 2.0), (AmmoCrate, 1.0), (FuelCell, 1.0)],
        // Luxuries and rare goods from a dozen universes.
        AiShipType::MerchantPrinces => &[(Crystal, 3.0), (RareAlloy, 3.0), (AncientArtifact, 2.0), (FuelCell, 1.0)],
        // Bulk: fuel, scrap, ammo.
        AiShipType::FreeHaulersCompact => &[(FuelCell, 4.0), (ScrapMetal, 4.0), (AmmoCrate, 3.0)],
    };

    let good_mult = if intact_frac >= 0.7 {
        1.5
    } else if intact_frac < 0.3 {
        0.35
    } else {
        1.0
    };
    let weight = |item: ItemType, w: f32| if item == ScrapMetal { w } else { w * good_mult };

    let total: f32 = table.iter().map(|&(i, w)| weight(i, w)).sum();
    let mut roll = rng.gen_range(0.0..total);
    for &(item, w) in table {
        roll -= weight(item, w);
        if roll <= 0.0 {
            return item;
        }
    }
    table[0].0
}

/// Fights happen 20 km off now, with the camera well back, where a blast sized
/// in world units was a couple of pixels and a ship could die unseen. Radius
/// for a blast that peaks at least `min_px` from its centre on screen (the
/// fireball reaches 2.2 radii, see spawn_explosion); close up, `radius` as is.
fn readable_blast(radius: f32, min_px: f32, camera_zoom: f32) -> f32 {
    radius.max(min_px * camera_zoom / 2.2)
}

/// Secondary explosions marching across a fresh wreck for a second or two
/// after the kill, ending in one final boom. Pure spectacle — pops don't
/// deal damage (the ship is already dead), they sell the death.
#[derive(Component)]
pub struct DeathRattle {
    pub timer: Timer,
    pub remaining: u32,
}

/// Ticks death rattles: random block pops with debris, then the final boom.
pub fn update_death_rattle(
    time: Res<Time>,
    mut commands: Commands,
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    camera: Res<crate::camera::CameraState>,
    mut rattle_query: Query<(Entity, &mut DeathRattle, Option<&Velocity>)>,
    children_query: Query<&Children>,
    block_pos_query: Query<(&GlobalTransform, &Sprite), Or<(With<Module>, With<HullSegment>)>>,
    mut boom_events: MessageWriter<AiModuleExploded>,
) {
    let mut rng = rand::thread_rng();
    for (ship_entity, mut rattle, velocity) in rattle_query.iter_mut() {
        rattle.timer.tick(time.delta());
        if !rattle.timer.is_finished() { continue; }

        // Pick a random surviving block to pop at
        let pop_pos = children_query.get(ship_entity).ok().and_then(|children| {
            let blocks: Vec<(Vec2, Color)> = children.iter()
                .filter_map(|c| block_pos_query.get(c).ok())
                .map(|(gt, sprite)| (gt.translation().truncate(), sprite.color))
                .collect();
            if blocks.is_empty() { None } else {
                Some(blocks[rng.gen_range(0..blocks.len())])
            }
        });
        let Some((pos, color)) = pop_pos else {
            commands.entity(ship_entity).try_remove::<DeathRattle>();
            continue;
        };

        let inherited = velocity.map(|v| v.0 * 0.6).unwrap_or(Vec2::ZERO);

        if rattle.remaining > 1 {
            // Secondary pop: flash + chunk spray + attenuated crunch
            let radius = readable_blast(rng.gen_range(14.0..26.0), 10.0, camera.zoom);
            crate::combat::spawn_explosion(&mut commands, &fx, pos, radius, Color::srgb(1.0, 0.6, 0.15));
            crate::vfx::debris::spawn_chunks(&mut commands, &fx, &mut rng, pos, color, inherited);
            boom_events.write(AiModuleExploded { position: pos, blast_damage: 20.0 });
            rattle.remaining -= 1;
            rattle.timer = Timer::from_seconds(rng.gen_range(0.15..0.4), TimerMode::Once);
        } else {
            // Final boom — big flash, big spray, deep audio layer
            // The climax of every ship kill. Was one flat 140-unit square.
            crate::combat::spawn_explosion(&mut commands, &fx, pos, readable_blast(62.0, 34.0, camera.zoom), Color::srgb(1.0, 0.5, 0.1));
            for _ in 0..3 {
                crate::vfx::debris::spawn_chunks(&mut commands, &fx, &mut rng, pos, color, inherited);
            }
            boom_events.write(AiModuleExploded { position: pos, blast_damage: 100.0 });
            commands.entity(ship_entity).try_remove::<DeathRattle>();
        }
    }
}

/// A wreck's blocks: dark, but still a shape against dark space.
const WRECK_TINT: Color = Color::srgb(0.42, 0.42, 0.44);

/// Paint a wreck's block dark for good. The base colour is what lasts: the damage
/// tint repaints every standing block from it each frame, so the sprite alone
/// stayed dark for one frame and the wreck then looked whole.
fn paint_wreck(commands: &mut Commands, block: Entity, sprite: &mut Sprite) {
    // Same rule as the damage tint: a transparent sprite is a block that draws
    // its shape out of children (the angled plates). Painting it would fade a
    // solid square in over the silhouette.
    if sprite.color.alpha() > 0.0 {
        sprite.color = WRECK_TINT;
        commands.entity(block).try_insert(BaseSpriteColor(WRECK_TINT));
    }
}

/// When an AI ship is destroyed, turn it into a wreck IN PLACE — keep the
/// hull/module hierarchy intact (attach Wreck/PointOfInterest/AiShipWreck to
/// the existing root) instead of despawning it and spawning an unrelated
/// generic gray sprite. That old approach erased the actual battle damage
/// the player had just fought for and replaced it with an abstract blob;
/// this way the derelict really is the ship you shot up, sitting there for
/// F-key salvage (world::salvage_wreck_system already works off any entity
/// with Wreck + PointOfInterest, no changes needed there). Remaining
/// modules are deactivated and darkened — the reactor's dead, nothing on
/// this hulk still has power — but nothing is despawned.
pub fn ai_ship_death_system(
    mut commands: Commands,
    mut destroyed_events: MessageReader<AiShipDestroyed>,
    children_query: Query<&Children>,
    mut module_query: Query<(&mut Module, &mut Sprite, Has<DestroyedModule>), Without<HullSegment>>,
    mut hull_query: Query<(&mut Sprite, &HullSegment, Has<HullDestroyed>), Without<Module>>,
    mut shield_query: Query<&mut crate::combat::shields::ShipShield>,
    mut notifications: MessageWriter<ShowNotification>,
) {
    for event in destroyed_events.read() {
        // Dead reactor, dead shield — permanently. update_shields recharges
        // any enabled shield after a few quiet seconds, so without this a
        // wreck's bubble came BACK UP and started absorbing shots again.
        if let Ok(mut shield) = shield_query.get_mut(event.entity) {
            shield.enabled = false;
            shield.current = 0.0;
        }
        let base_loot = match event.ship_type {
            AiShipType::Shepherd => 30,    // legendary hoard
            AiShipType::EternalHegemony => 20,  // colossal wreck
            AiShipType::StellarPreserve => 6,
            AiShipType::SynthesisCollective => 4,
            AiShipType::BrokenChoir => 8,     // rare old loot
            AiShipType::CorpseStars => 5,
            AiShipType::TheSilence => 7,    // intel data
            AiShipType::TerranHegemony => 10,    // massive wreck
            AiShipType::GildedThrone => 6,
            AiShipType::RecursiveKingdom => 2,    // junk
            AiShipType::MerchantPrinces => 10,  // a hold full of cargo
            AiShipType::FreeHaulersCompact => 8,
        };

        // A meltdown takes the ship apart on the way out: the blast guts a
        // share of whatever was still intact, so cracking the core buys a
        // fast kill and pays for it in salvage. Striking colors leaves the
        // hull as-is, which is why it's the best wreck in the game.
        if event.cause == ShipDeathCause::Meltdown {
            if let Ok(children) = children_query.get(event.entity) {
                for (i, child) in children.iter().enumerate() {
                    // Every other block: a shattered hulk, not vapour.
                    if i % 2 != 0 { continue; }
                    if let Ok((mut module, _, is_destroyed)) = module_query.get_mut(child) {
                        if !is_destroyed {
                            module.health = 0.0;
                            let original_type = module.module_type;
                            commands.entity(child).try_insert(DestroyedModule { original_type });
                        }
                    }
                }
            }
        }

        // Power off and darken every block — dead reactor, no power,
        // nothing left running. No instant credit payout here anymore:
        // wreck value is whatever the salvage detail physically carries
        // home (crew::eva_salvage), priced by whichever station buys it.
        let mut block_count = 0u32;
        let mut intact_count = 0u32;
        if let Ok(children) = children_query.get(event.entity) {
            for child in children.iter() {
                if let Ok((mut module, mut sprite, is_destroyed)) = module_query.get_mut(child) {
                    block_count += 1;
                    if !is_destroyed {
                        intact_count += 1;
                    }
                    module.is_active = false;
                    paint_wreck(&mut commands, child, &mut sprite);
                } else if let Ok((mut sprite, _hull, is_destroyed)) = hull_query.get_mut(child) {
                    block_count += 1;
                    if !is_destroyed {
                        intact_count += 1;
                    }
                    paint_wreck(&mut commands, child, &mut sprite);
                }
            }
        }

        // FORENSIC WRECKS — the kill method shapes the loot. A surgical kill
        // (EMP-disable, snipe the reactor, hull mostly whole) leaves a
        // pristine wreck worth extra; grinding the ship to dust leaves
        // scraps. intact fraction is the honest measure of "how gently
        // did you kill this".
        let intact_frac = if block_count > 0 {
            intact_count as f32 / block_count as f32
        } else {
            0.0
        };
        let (loot, condition) = if intact_frac >= 0.7 {
            (((base_loot as f32) * 1.5).ceil() as u32, "Pristine wreck - bonus salvage!")
        } else if intact_frac < 0.3 {
            (((base_loot as f32) * 0.5).ceil() as u32, "Shattered hulk - little left to take.")
        } else {
            (base_loot, "Wreck can be salvaged.")
        };

        commands.entity(event.entity).try_insert((
            AiShipWreck {
                ship_type: event.ship_type,
                loot_remaining: loot,
                intact_frac,
            },
            PointOfInterest {
                poi_type: PoiType::Wreck,
                discovered: true,
            },
            Wreck {
                loot_remaining: loot,
                is_explored: false,
            },
        ));

        // Bigger ships rattle longer before the final boom; shattered kills
        // rattle extra hard — there's more unstable wreckage going up.
        let extra_pops = if intact_frac < 0.3 { 3 } else { 0 };
        commands.entity(event.entity).try_insert(DeathRattle {
            timer: Timer::from_seconds(0.2, TimerMode::Once),
            remaining: (block_count / 5).clamp(3, 10) + extra_pops,
        });

        let type_name = faction_display_name(event.ship_type);

        let headline = match event.cause {
            ShipDeathCause::Struck => format!("{} vessel struck colors - crew abandoned it", type_name),
            ShipDeathCause::Meltdown => format!("{} vessel lost containment - core detonation", type_name),
            ShipDeathCause::Gutted => format!("{} vessel destroyed!", type_name),
        };
        notifications.write(ShowNotification {
            message: format!("{} {} (F: salvage detail)", headline, condition),
            notification_type: NotificationType::Success,
            duration: 4.0,
        });
    }
}

/// HOT WRECKS — fires on a fresh wreck keep eating it. While any block on a
/// wreck is still burning, its remaining loot ticks away (one unit every few
/// seconds). Looting immediately means working next to live fires and
/// cook-offs (burning explosive blocks still detonate); waiting for the
/// burn-out is safe but costs cargo. Fires are finite (BlockBurning has a
/// duration), so a wreck never burns to literally nothing unless it was
/// already ablaze everywhere.
pub fn wreck_fire_consumes_loot(
    fx: Res<crate::vfx::effect_textures::EffectTextures>,
    time: Res<Time>,
    mut commands: Commands,
    mut wreck_query: Query<(Entity, &mut Wreck, &mut AiShipWreck, &Children)>,
    burning_query: Query<(), With<crate::combat::new_projectiles::BlockBurning>>,
    block_pos_query: Query<&GlobalTransform, Or<(With<Module>, With<HullSegment>)>>,
    mut tick: Local<f32>,
) {
    *tick += time.delta_secs();
    if *tick < 4.0 { return; }
    *tick = 0.0;

    let mut rng = rand::thread_rng();
    for (_entity, mut wreck, mut ai_wreck, children) in wreck_query.iter_mut() {
        if wreck.loot_remaining == 0 { continue; }
        let burning_blocks: Vec<Entity> = children.iter()
            .filter(|c| burning_query.get(*c).is_ok())
            .collect();
        if burning_blocks.is_empty() { continue; }

        wreck.loot_remaining = wreck.loot_remaining.saturating_sub(1);
        ai_wreck.loot_remaining = ai_wreck.loot_remaining.saturating_sub(1);

        // Smoke puff over a random burning block so the loss reads visually.
        //
        // Was a flat translucent square that appeared and vanished. A wreck
        // burning off its cargo should look like it is still burning a second
        // later, so this drifts and lingers instead.
        let smoke_at = burning_blocks[rng.gen_range(0..burning_blocks.len())];
        if let Ok(gt) = block_pos_query.get(smoke_at) {
            let at = gt.translation().truncate();
            for _ in 0..2 {
                let drift = Vec2::from_angle(rng.gen_range(0.0..std::f32::consts::TAU))
                    * rng.gen_range(4.0..16.0);
                let grey = rng.gen_range(0.26..0.40);
                commands.spawn((
                    Sprite {
                        image: fx.puff(),
                        color: Color::srgba(grey, grey * 0.95, grey * 0.9, 0.34),
                        custom_size: Some(Vec2::splat(rng.gen_range(26.0..46.0))),
                        ..default()
                    },
                    Transform::from_xyz(at.x, at.y, 0.59),
                    crate::vfx::particles::Particle::wisp(
                        drift,
                        rng.gen_range(1.6..2.8),
                        0.34,
                        false,
                    ),
                ));
            }
        }
    }
}

#[cfg(test)]
mod wreck_paint_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// With the damage tint running, as it does in the game.
    #[test]
    fn a_wreck_stays_dark() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, crate::ship::tint_damaged_hull);
        let block = app
            .world_mut()
            .spawn((
                HullSegment { health: 100.0, max_health: 100.0, ..default() },
                BaseSpriteColor(Color::WHITE),
                Sprite::default(),
            ))
            .id();
        app.update();
        app.world_mut()
            .run_system_once(move |mut commands: Commands, mut sprites: Query<&mut Sprite>| {
                paint_wreck(&mut commands, block, &mut sprites.get_mut(block).unwrap());
            })
            .unwrap();
        app.update();
        app.update();
        assert_eq!(app.world().get::<Sprite>(block).unwrap().color, WRECK_TINT);
    }
}

#[cfg(test)]
mod blast_tests {
    use super::*;

    #[test]
    fn a_death_blast_reads_from_far_off_and_is_unchanged_close_up() {
        assert_eq!(readable_blast(62.0, 34.0, 1.8), 62.0);
        let far = readable_blast(62.0, 34.0, 38.0);
        // peaks 2.2 radii out: 34 px on screen at this zoom
        assert!((far * 2.2 / 38.0 - 34.0).abs() < 1e-3, "{far}");
    }
}
