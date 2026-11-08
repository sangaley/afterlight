use bevy::prelude::*;
use std::collections::HashMap;

// ============================================================================
// TIER 2+3 SUB-COMPONENT DATA MODEL
// Defines how modules are customized at the component and parameter level.
// ============================================================================

/// A tunable parameter with a name, range, default, and gameplay description.
/// This is the atomic unit of Tier 3 customization.
#[derive(Clone, Debug)]
pub struct ParameterDef {
    pub name: String,
    pub description: String,
    /// What stat this affects (for tooltip: "+15% range")
    pub affects: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    /// The "green zone" — range where the parameter is safe/optimal
    pub optimal_min: f32,
    pub optimal_max: f32,
    /// Unit label for display ("mm", "%", "rpm", "MW")
    pub unit: String,
    /// How much changing this parameter by 1 unit affects related stats
    pub stat_scale: f32,
    /// Step size for the slider (0.0 = continuous)
    pub step: f32,
}

/// A ship-component slot that can be swapped between types.
/// This is Tier 2 — picking which ship-component goes in each slot.
#[derive(Clone, Debug)]
pub struct SubComponentSlotDef {
    pub slot_name: String,
    pub description: String,
    /// Available ship-component types for this slot
    pub options: Vec<SubComponentOption>,
    /// Index of the default option
    pub default_option: usize,
}

/// One option for a ship-component slot
#[derive(Clone, Debug)]
pub struct SubComponentOption {
    pub name: String,
    pub description: String,
    /// Stat modifiers applied when this option is selected
    pub stat_modifiers: HashMap<String, f32>,
    /// Tier 3 parameters available for this specific option
    pub parameters: Vec<ParameterDef>,
}

/// Runtime state of a module's customization
#[derive(Component, Clone, Debug)]
pub struct ModuleCustomization {
    /// Which option is selected per slot (slot_name → option index)
    pub slot_selections: HashMap<String, usize>,
    /// Current Tier 3 parameter values (slot_name.param_name → value)
    pub parameter_values: HashMap<String, f32>,
}

impl Default for ModuleCustomization {
    fn default() -> Self {
        Self {
            slot_selections: HashMap::new(),
            parameter_values: HashMap::new(),
        }
    }
}

impl ModuleCustomization {
    /// Get a parameter value, falling back to its definition's default
    pub fn get_param(&self, key: &str, default: f32) -> f32 {
        self.parameter_values.get(key).copied().unwrap_or(default)
    }

    /// Set a parameter value, clamped to its valid range
    pub fn set_param(&mut self, key: &str, value: f32, param_def: &ParameterDef) {
        let clamped = value.clamp(param_def.min, param_def.max);
        let snapped = if param_def.step > 0.0 {
            (clamped / param_def.step).round() * param_def.step
        } else {
            clamped
        };
        self.parameter_values.insert(key.to_string(), snapped);
    }

    /// Check if a parameter is in its optimal range
    pub fn is_param_optimal(&self, key: &str, param_def: &ParameterDef) -> bool {
        let value = self.get_param(key, param_def.default);
        value >= param_def.optimal_min && value <= param_def.optimal_max
    }
}

/// Defines the full customization schema for a module type.
/// This is the "blueprint" — what CAN be customized.
#[derive(Clone, Debug)]
pub struct ModuleCustomizationDef {
    pub slots: Vec<SubComponentSlotDef>,
}

/// Global registry of customization definitions per module type
#[derive(Resource, Default)]
pub struct CustomizationRegistry {
    pub defs: HashMap<String, ModuleCustomizationDef>,
}

impl CustomizationRegistry {
    pub fn get(&self, module_key: &str) -> Option<&ModuleCustomizationDef> {
        self.defs.get(module_key)
    }

    pub fn register(&mut self, module_key: &str, def: ModuleCustomizationDef) {
        self.defs.insert(module_key.to_string(), def);
    }
}

/// Helper to create a parameter definition with common defaults
pub fn param(
    name: &str,
    description: &str,
    affects: &str,
    min: f32,
    max: f32,
    default: f32,
    unit: &str,
) -> ParameterDef {
    ParameterDef {
        name: name.to_string(),
        description: description.to_string(),
        affects: affects.to_string(),
        min,
        max,
        default,
        optimal_min: default * 0.7,
        optimal_max: default * 1.3,
        unit: unit.to_string(),
        stat_scale: 1.0,
        step: 0.0,
    }
}

/// Helper with step size
pub fn param_stepped(
    name: &str,
    description: &str,
    affects: &str,
    min: f32,
    max: f32,
    default: f32,
    unit: &str,
    step: f32,
) -> ParameterDef {
    ParameterDef {
        name: name.to_string(),
        description: description.to_string(),
        affects: affects.to_string(),
        min,
        max,
        default,
        optimal_min: default * 0.7,
        optimal_max: default * 1.3,
        unit: unit.to_string(),
        stat_scale: 1.0,
        step,
    }
}
