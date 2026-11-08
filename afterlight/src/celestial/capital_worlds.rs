//! Capital worlds: where a faction rules from.
//!
//! A capital used to be a flag on a system: a defence bonus in the border war
//! and a ring on the map. Now the capital system's biggest planet -- the one
//! its starport already orbits (diplomacy::stations) -- is a named, inhabited
//! world: "Vigil Prime", with its cities lit on the night side and a line on
//! who lives there and what it makes. The starport over it is the faction's
//! High Command. When the border war moves a capital, the old world goes
//! back to being a number and the new one lights up.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use rand::{Rng, SeedableRng};

use super::components::{CelestialBody, Planet, Star, StarSystemMember};
use super::resources::{GalaxyMap, SystemStreamingManager};
use crate::ai_ship::components::AiShipType;

/// A planet that is a faction's capital world. Keeps the name it had, to give
/// back if the capital moves.
#[derive(Component, Clone, Debug)]
pub struct CapitalWorld {
    pub faction: AiShipType,
    pub numbered_name: String,
}

/// The lit cities on a capital world's night side.
#[derive(Component, Clone, Copy, Debug)]
pub struct CapitalLights {
    pub world: Entity,
}

/// Texture side, in pixels, of the city-lights overlay.
const LIGHTS_SIZE: u32 = 1024;

/// One texture for every capital's lights, made once.
#[derive(Resource, Default)]
struct LightsImage(Option<Handle<Image>>);

/// The capital world's name: its system's, and "Prime".
pub fn capital_world_name(system_name: &str) -> String {
    format!("{system_name} Prime")
}

/// Who lives on a faction's capital world, and what it makes: one line for
/// the map and the starport.
pub fn capital_blurb(faction: AiShipType) -> (&'static str, &'static str) {
    use AiShipType::*;
    match faction {
        TerranHegemony => ("nine billion, under martial law since the unification wars", "railguns, battleship hulls, plate"),
        GildedThrone => ("four hundred million shareholders and their staff", "shield emitters, and debt"),
        MerchantPrinces => ("no fixed number: the Guild registers its people by contract", "luxuries from a dozen universes"),
        FreeHaulersCompact => ("two hundred million haulers, miners and their families", "fuel, ore and salvage"),
        BrokenChoir => ("uncounted: every census returns the same voice", "nothing anyone can name"),
        SynthesisCollective => ("one mind in six hundred million bodies", "grafts and living weapons"),
        CorpseStars => ("a few million, and their dead, who still hold office", "Abyssal Alloy and relics"),
        StellarPreserve => ("forty million keepers, and everything they keep", "specimens and containment arks"),
        TheSilence => ("eleven million, every one of them quiet", "nothing; they ask for nothing"),
        RecursiveKingdom => ("one person, two billion times over", "copies"),
        EternalHegemony => ("an empire's worth of ghosts", "the past"),
        Shepherd => ("everyone it has gathered", "one"),
    }
}

/// The city lights: cities -- clusters of warm points -- over one half of a
/// disc (+x), thickest away from the edge between day and night, none on
/// the day side. Fine-grained: a planet is 25-120 km across, and at 128
/// pixels every light was a block half a kilometre wide.
fn lights_image(seed: u64) -> Image {
    let n = LIGHTS_SIZE as usize;
    let mut data = vec![0u8; n * n * 4];
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let c = n as f32 / 2.0;
    let mut put = |x: f32, y: f32, bright: u8, warm: f32, data: &mut Vec<u8>| {
        let (dx, dy) = (x - c, y - c);
        if (dx * dx + dy * dy).sqrt() > c * DISC || dx <= 0.0 || x < 0.0 || y < 0.0 || x >= n as f32 || y >= n as f32 {
            return;
        }
        let i = (y as usize * n + x as usize) * 4;
        // Lights replace the shadow under them, at full opacity.
        data[i] = bright;
        data[i + 1] = (bright as f32 * warm) as u8;
        data[i + 2] = (bright as f32 * warm * 0.45) as u8;
        data[i + 3] = 255;
    };
    // The night: a shadow over the +x half, fading in from the terminator,
    // so the lights read (the planet art is lit all over).
    for y in 0..n {
        for x in (n / 2)..n {
            let (dx, dy) = (x as f32 - c, y as f32 - c);
            if (dx * dx + dy * dy).sqrt() > c * NIGHT_DISC {
                continue;
            }
            let t = (dx / (c * 0.35)).clamp(0.0, 1.0);
            let i = (y * n + x) * 4;
            data[i + 3] = (t * t * (3.0 - 2.0 * t) * NIGHT_SHADE * 255.0) as u8;
        }
    }
    for _ in 0..CITIES {
        // A city somewhere on the night half, fewer near the terminator.
        let (cx, cy) = loop {
            let x = rng.gen_range(c..n as f32);
            let y = rng.gen_range(0.0..n as f32);
            if rng.gen::<f32>() < ((x - c) / c).sqrt() {
                break (x, y);
            }
        };
        let size = rng.gen_range(3.0..12.0_f32);
        for _ in 0..(size * size * 0.7) as usize {
            let a = rng.gen_range(0.0..std::f32::consts::TAU);
            let d = rng.gen::<f32>().powf(1.8) * size;
            let bright = rng.gen_range(170..=255u8);
            put(cx + a.cos() * d, cy + a.sin() * d, bright, rng.gen_range(0.62..0.8), &mut data);
        }
    }
    // And the scattered lights between them: towns, roads.
    for _ in 0..2_500 {
        let x = rng.gen_range(c..n as f32);
        let y = rng.gen_range(0.0..n as f32);
        if rng.gen::<f32>() < ((x - c) / c).sqrt() {
            put(x, y, rng.gen_range(70..=160u8), 0.7, &mut data);
        }
    }
    Image::new(
        Extent3d { width: LIGHTS_SIZE, height: LIGHTS_SIZE, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// How many cities light a capital world's night side.
const CITIES: usize = 140;
/// How dark the night side's shadow gets, and how far out it reaches (just
/// past the lights, inside the art's glow).
const NIGHT_SHADE: f32 = 0.62;
const NIGHT_DISC: f32 = 0.825;
/// The planet art's disc fills 82-84% of its image (the rest is its glow):
/// lights stay inside it, or they hang in space past the limb.
const DISC: f32 = 0.78;

/// Mark the loaded system's capital world (and unmark one that no longer is).
fn mark_capital_world(
    mut commands: Commands,
    streaming: Res<SystemStreamingManager>,
    galaxy: Res<GalaxyMap>,
    mut planets: Query<(Entity, &mut CelestialBody, &StarSystemMember, Option<&CapitalWorld>), With<Planet>>,
    lights: Query<(Entity, &CapitalLights)>,
    mut image: ResMut<LightsImage>,
    mut images: ResMut<Assets<Image>>,
) {
    let system = streaming.loaded_system.and_then(|id| galaxy.systems.iter().find(|s| s.id == id));
    let ruler = system.filter(|s| s.capital).and_then(|s| s.faction);
    // The capital world is the system's biggest planet: the one its starport
    // orbits (diplomacy::stations picks it the same way).
    let world = ruler.and_then(|_| {
        let id = system?.id;
        planets
            .iter()
            .filter(|(_, _, m, _)| m.system_id == id)
            .max_by(|a, b| a.1.radius.total_cmp(&b.1.radius))
            .map(|(e, ..)| e)
    });
    for (e, mut body, _, capital) in planets.iter_mut() {
        let should = Some(e) == world;
        match (capital, should) {
            (None, true) => {
                let faction = ruler.unwrap();
                let numbered = body.name.clone();
                body.name = capital_world_name(&system.unwrap().name);
                info!("CAPITAL WORLD {} ({:?}) was {}", body.name, faction, numbered);
                // try_: a load or a jump clears the system's planets the same
                // frame, and a command on a planet already gone panicked.
                commands.entity(e).try_insert(CapitalWorld { faction, numbered_name: numbered });
                let handle = image.0.get_or_insert_with(|| images.add(lights_image(7))).clone();
                let size = body.radius * 2.0;
                // Its own entity, not the planet's child: it follows the
                // planet (turn_lights_to_night), and goes when the planet does.
                commands.spawn((
                    Sprite { image: handle, custom_size: Some(Vec2::splat(size)), color: Color::WHITE.with_alpha(0.85), ..default() },
                    Transform::from_xyz(0.0, 0.0, -0.89),
                    CapitalLights { world: e },
                ));
            }
            (Some(c), false) => {
                body.name = c.numbered_name.clone();
                commands.entity(e).try_remove::<CapitalWorld>();
                for (l, lit) in lights.iter() {
                    if lit.world == e {
                        commands.entity(l).try_despawn();
                    }
                }
            }
            (Some(c), true) if Some(c.faction) != ruler => {
                commands.entity(e).try_insert(CapitalWorld { faction: ruler.unwrap(), numbered_name: c.numbered_name.clone() });
            }
            _ => {}
        }
    }
}

/// Keep the lights on their world, the lit half turned away from the star;
/// lights whose world is gone (unloaded, or no longer the capital) go too.
fn turn_lights_to_night(
    mut commands: Commands,
    star: Query<&Transform, (With<Star>, Without<CapitalLights>)>,
    worlds: Query<&Transform, (With<CapitalWorld>, Without<CapitalLights>)>,
    mut lights: Query<(Entity, &CapitalLights, &mut Transform)>,
) {
    let star = star.iter().next().map(|t| t.translation.truncate());
    for (e, lit, mut tf) in lights.iter_mut() {
        let Ok(world) = worlds.get(lit.world) else {
            commands.entity(e).try_despawn();
            continue;
        };
        let at = world.translation.truncate();
        let away = star.map_or(Vec2::X, |s| (at - s).normalize_or(Vec2::X));
        tf.translation.x = at.x;
        tf.translation.y = at.y;
        tf.rotation = Quat::from_rotation_z(away.to_angle());
    }
}

pub struct CapitalWorldsPlugin;

impl Plugin for CapitalWorldsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LightsImage>()
            .add_systems(Update, (mark_capital_world, turn_lights_to_night).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lights_are_on_the_night_side_only() {
        let image = lights_image(7);
        let data = image.data.as_ref().unwrap();
        let n = LIGHTS_SIZE as usize;
        let mut lit_day = 0;
        let mut lit_night = 0;
        for y in 0..n {
            for x in 0..n {
                let i = (y * n + x) * 4;
                if data[i + 3] > 0 && x < n / 2 {
                    lit_day += 1;
                }
                if data[i + 3] == 255 && data[i] > 0 {
                    lit_night += 1;
                }
            }
        }
        assert_eq!(lit_day, 0, "anything on the day side");
        assert!(lit_night > 1_000, "only {lit_night} lights");
        // None past the planet art's disc.
        let c = n as f32 / 2.0;
        for y in 0..n {
            for x in 0..n {
                let i = (y * n + x) * 4;
                let r = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
                if data[i] > 0 {
                    assert!(r <= c * DISC + 1.0, "a light at {r:.0} px, past the disc");
                }
                if data[i + 3] > 0 {
                    assert!(r <= c * NIGHT_DISC + 1.0, "shadow at {r:.0} px, past the disc");
                }
            }
        }
    }

    #[test]
    fn every_capital_faction_has_a_line() {
        for f in super::super::territory::TERRITORIAL {
            let (people, makes) = capital_blurb(f);
            assert!(!people.is_empty() && !makes.is_empty());
        }
        assert_eq!(capital_world_name("Vigil"), "Vigil Prime");
    }

    /// Writes the lights texture to $AFTERLIGHT_DUMP_LIGHTS, to look at.
    #[test]
    #[ignore = "writes a file; run deliberately"]
    fn dump_lights() {
        let Ok(path) = std::env::var("AFTERLIGHT_DUMP_LIGHTS") else { return };
        let image = lights_image(7);
        image.try_into_dynamic().unwrap().save(path).unwrap();
    }
}
