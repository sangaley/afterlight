use crate::components::*;

/// Stat calculator for custom modules
/// Implements formulas that calculate final stats from ship-components
pub struct StatCalculator;

impl StatCalculator {
    /// Calculate weapon stats from base stats and ship-components
    pub fn calculate_weapon_stats(
        base: &WeaponStats,
        subcomponents: &[SubComponentType],
    ) -> WeaponStats {
        let mut stats = base.clone();

        // Apply modifiers from each ship-component
        for subcomp in subcomponents {
            match subcomp {
                SubComponentType::BarrelComponent { length, caliber, thickness } => {
                    // Barrel length → +range (longer barrel = more range)
                    // Formula: range = base × (1 + length/10)
                    stats.range = base.range * (1.0 + length / 10.0);

                    // Caliber → +damage (larger caliber = more damage)
                    // Formula: damage = base × (caliber/50) where 50 is baseline caliber
                    stats.damage = base.damage * (caliber / 50.0);

                    // Thickness affects power cost (thicker = heavier = more power)
                    stats.power_cost = base.power_cost * (1.0 + (thickness - 5.0) / 10.0);
                }
                SubComponentType::ChamberComponent { volume: _, pressure } => {
                    // Chamber pressure → +damage +fire_rate
                    // Higher pressure = more projectile velocity = more damage
                    // Formula: damage multiplier = 1 + (pressure-100)/200
                    let pressure_mult = 1.0 + (pressure - 100.0) / 200.0;
                    stats.damage *= pressure_mult;

                    // Higher pressure also increases fire rate slightly
                    stats.fire_rate *= 1.0 + (pressure - 100.0) / 400.0;

                    // Higher pressure requires more power
                    stats.power_cost *= 1.0 + (pressure - 100.0) / 200.0;
                }
                SubComponentType::LoaderComponent { mechanism, speed } => {
                    // Loader mechanism → fire_rate multiplier
                    let mechanism_mult = match mechanism {
                        LoaderMechanism::Manual => 0.5,
                        LoaderMechanism::Automatic => 1.0,
                        LoaderMechanism::Rotary => 1.5,
                    };

                    // Formula: fire_rate = base × mechanism_mult × speed
                    stats.fire_rate = base.fire_rate * mechanism_mult * speed;
                }
                SubComponentType::MagazineComponent { capacity } => {
                    // Magazine capacity directly sets max ammo
                    stats.max_ammo = *capacity;
                }
                _ => {} // Ignore non-weapon components
            }
        }

        stats
    }

    /// Calculate engine stats from base stats and ship-components
    pub fn calculate_engine_stats(
        base: &EngineStats,
        subcomponents: &[SubComponentType],
    ) -> EngineStats {
        let mut stats = base.clone();

        for subcomp in subcomponents {
            match subcomp {
                SubComponentType::CombustionChamber { efficiency } => {
                    // Combustion efficiency → +thrust -fuel_use
                    // Formula: thrust = base × efficiency
                    stats.thrust = base.thrust * efficiency;

                    // Better efficiency = better fuel economy
                    // Formula: fuel_efficiency = base / (efficiency × 0.5 + 0.5)
                    stats.fuel_efficiency = base.fuel_efficiency / (efficiency * 0.5 + 0.5);
                }
                SubComponentType::PropellerBlade { pitch, count } => {
                    // Propeller count → +thrust +noise
                    // More blades = more thrust but more noise
                    stats.thrust = base.thrust * (*count as f32 / 4.0) * pitch;
                    stats.noise = base.noise * (*count as f32 / 4.0) * pitch;
                }
                SubComponentType::FuelTank { capacity: _ } => {
                    // Fuel tank affects range but not handled here
                    // (would be handled by fuel system)
                }
                _ => {} // Ignore non-engine components
            }
        }

        stats
    }

    /// Calculate reactor stats from base stats and ship-components
    pub fn calculate_reactor_stats(
        base: &ReactorStats,
        subcomponents: &[SubComponentType],
    ) -> ReactorStats {
        let mut stats = base.clone();

        for subcomp in subcomponents {
            match subcomp {
                SubComponentType::FuelRod { enrichment, count } => {
                    // Fuel enrichment → +power +heat +explosion_risk
                    // Formula: power = base × enrichment × count
                    stats.power_output = base.power_output * enrichment * (*count as f32);

                    // Higher enrichment = more heat
                    stats.heat_generation = base.heat_generation * enrichment * (*count as f32);

                    // Higher enrichment = higher explosion risk
                    stats.explosion_risk = (enrichment - 1.0) * (*count as f32);
                }
                SubComponentType::Coolant { flow_rate } => {
                    // Coolant flow → -heat
                    // Formula: heat = heat / (flow_rate × 0.01)
                    let flow_factor = (flow_rate * 0.01).max(0.001);
                    stats.heat_generation = stats.heat_generation / flow_factor;
                }
                SubComponentType::Shielding { thickness } => {
                    // Shielding → -explosion_risk
                    // Formula: risk = risk / thickness
                    if *thickness > 0.0 {
                        stats.explosion_risk = stats.explosion_risk / *thickness;
                    }
                }
                _ => {} // Ignore non-reactor components
            }
        }

        stats
    }

    /// Calculate life support stats from base stats and ship-components
    pub fn calculate_life_support_stats(
        base: &LifeSupportStats,
        subcomponents: &[SubComponentType],
    ) -> LifeSupportStats {
        let mut stats = base.clone();

        for subcomp in subcomponents {
            match subcomp {
                SubComponentType::OxygenScrubber { filter_size } => {
                    // Scrubber size → +o2_generation
                    // Formula: o2_generation = base × filter_size
                    stats.o2_generation = base.o2_generation * filter_size;
                }
                SubComponentType::CO2Absorber { efficiency } => {
                    // Absorber efficiency → +co2_filtering
                    // Formula: co2_filtering = base × efficiency
                    stats.co2_filtering = base.co2_filtering * efficiency;
                }
                _ => {} // Ignore non-life-support components
            }
        }

        // How many crew this life support can actually keep breathing.
        // Derived from the real consumption rate rather than a second
        // hardcoded number — the two drifted apart once OXYGEN_PER_CREW moved.
        stats.crew_capacity =
            (stats.o2_generation / crate::resources::OXYGEN_PER_CREW).floor() as u32;

        stats
    }

    /// Calculate all stats for a custom module based on its type and ship-components
    pub fn calculate_stats(
        module_type: ModuleType,
        subcomponents: &[SubComponentType],
        base_stats: &CalculatedStats,
    ) -> CalculatedStats {
        let mut calculated = CalculatedStats::default();

        // Determine which stats to calculate based on module category
        match module_type.category() {
            ModuleCategory::Weapons => {
                if let Some(ref base_weapon) = base_stats.weapon {
                    calculated.weapon = Some(Self::calculate_weapon_stats(base_weapon, subcomponents));
                }
            }
            ModuleCategory::Propulsion => {
                if let Some(ref base_engine) = base_stats.engine {
                    calculated.engine = Some(Self::calculate_engine_stats(base_engine, subcomponents));
                }
            }
            ModuleCategory::Power => {
                if let Some(ref base_reactor) = base_stats.reactor {
                    calculated.reactor = Some(Self::calculate_reactor_stats(base_reactor, subcomponents));
                }
            }
            ModuleCategory::LifeSupport => {
                if let Some(ref base_life_support) = base_stats.life_support {
                    calculated.life_support = Some(Self::calculate_life_support_stats(base_life_support, subcomponents));
                }
            }
            _ => {
                // Other categories don't have custom stats yet
            }
        }

        calculated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_barrel_increases_range() {
        let base = WeaponStats {
            damage: 50.0,
            range: 100.0,
            fire_rate: 1.0,
            max_ammo: 10,
            power_cost: 10.0,
        };

        let subcomps = vec![SubComponentType::BarrelComponent {
            length: 10.0,
            caliber: 50.0,
            thickness: 5.0,
        }];

        let result = StatCalculator::calculate_weapon_stats(&base, &subcomps);

        // Range should be doubled (1 + 10/10 = 2.0)
        assert!((result.range - 200.0).abs() < 0.1);
    }

    #[test]
    fn test_weapon_caliber_increases_damage() {
        let base = WeaponStats {
            damage: 50.0,
            range: 100.0,
            fire_rate: 1.0,
            max_ammo: 10,
            power_cost: 10.0,
        };

        let subcomps = vec![SubComponentType::BarrelComponent {
            length: 5.0,
            caliber: 100.0, // Double baseline
            thickness: 5.0,
        }];

        let result = StatCalculator::calculate_weapon_stats(&base, &subcomps);

        // Damage should be doubled (100/50 = 2.0)
        assert!((result.damage - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_reactor_enrichment_increases_power() {
        let base = ReactorStats {
            power_output: 100.0,
            heat_generation: 50.0,
            explosion_risk: 0.1,
        };

        let subcomps = vec![SubComponentType::FuelRod {
            enrichment: 2.0,
            count: 4,
        }];

        let result = StatCalculator::calculate_reactor_stats(&base, &subcomps);

        // Power should be 8x (2.0 enrichment × 4 rods)
        assert!((result.power_output - 800.0).abs() < 0.1);
    }
}
