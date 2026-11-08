use crate::resources::BiomeType;

/// Returns creature spawn weights for a biome
/// Parked with `world::update_biome`: its only caller lives in
/// `src/parked/creatures`, which is not compiled. Kept because the weights are
/// authored data worth more than the few lines of code around them.
#[allow(dead_code)]
pub fn biome_creature_weights(biome: BiomeType) -> Vec<(&'static str, f32)> {
    match biome {
        BiomeType::OpenVoid => vec![
            ("void_drifter", 0.6),
            ("stalker", 0.1),
        ],
        BiomeType::AsteroidField => vec![
            ("void_drifter", 0.4),
            ("stalker", 0.3),
            ("parasite_swarm", 0.1),
        ],
        BiomeType::CrystalFormation => vec![
            ("void_drifter", 0.3),
            ("stalker", 0.2),
            ("parasite_swarm", 0.2),
        ],
        BiomeType::ThermalVents => vec![
            ("void_drifter", 0.2),
            ("stalker", 0.3),
            ("parasite_swarm", 0.3),
        ],
        BiomeType::IceShells => vec![
            ("void_drifter", 0.3),
            ("stalker", 0.4),
        ],
        BiomeType::DeadZone => vec![
            ("stalker", 0.3),
            ("parasite_swarm", 0.3),
            ("leviathan", 0.05),
        ],
        BiomeType::VoidRift => vec![
            ("stalker", 0.2),
            ("parasite_swarm", 0.2),
            ("leviathan", 0.1),
        ],
        BiomeType::AncientRuins => vec![
            ("void_drifter", 0.2),
            ("parasite_swarm", 0.3),
            ("stalker", 0.2),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_biomes_have_creature_weights() {
        let biomes = [
            BiomeType::OpenVoid,
            BiomeType::AsteroidField,
            BiomeType::CrystalFormation,
            BiomeType::ThermalVents,
            BiomeType::IceShells,
            BiomeType::DeadZone,
            BiomeType::VoidRift,
            BiomeType::AncientRuins,
        ];

        for biome in biomes {
            let weights = biome_creature_weights(biome);
            assert!(!weights.is_empty(), "{:?} should have creature spawn weights", biome);

            for (name, weight) in &weights {
                assert!(*weight > 0.0, "Creature '{}' in {:?} has non-positive weight", name, biome);
            }
        }
    }

    #[test]
    fn deep_biomes_have_dangerous_creatures() {
        let deep_weights = biome_creature_weights(BiomeType::VoidRift);
        let creature_names: Vec<&str> = deep_weights.iter().map(|(n, _)| *n).collect();
        assert!(creature_names.contains(&"leviathan"), "VoidRift should contain leviathan");
    }
}
