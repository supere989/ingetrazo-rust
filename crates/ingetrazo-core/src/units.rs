// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Marco Sumari Tellez and IngeTrazo contributors.

//! Length unit parsing and formatting (Meters, Centimeters, Millimeters, Inches, Feet).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LengthUnit {
    Meters,
    Centimeters,
    Millimeters,
    Inches,
    Feet,
}

impl Default for LengthUnit {
    fn default() -> Self {
        Self::Meters
    }
}

impl LengthUnit {
    /// Conversion factor from internal units (Meters) to this unit.
    pub fn to_unit_scale(self) -> f64 {
        match self {
            Self::Meters => 1.0,
            Self::Centimeters => 100.0,
            Self::Millimeters => 1000.0,
            Self::Inches => 39.37007874,
            Self::Feet => 3.280839895,
        }
    }

    /// Conversion factor from this unit to internal units (Meters).
    pub fn from_unit_scale(self) -> f64 {
        match self { Self::Meters=>1., Self::Centimeters=>0.01, Self::Millimeters=>0.001, Self::Inches=>0.0254, Self::Feet=>0.3048 }
    }

    pub fn code(self) -> &'static str {
        match self {Self::Meters=>"m",Self::Centimeters=>"cm",Self::Millimeters=>"mm",Self::Inches=>"in",Self::Feet=>"ft"}
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Self::Meters => "m",
            Self::Centimeters => "cm",
            Self::Millimeters => "mm",
            Self::Inches => "\"",
            Self::Feet => "'",
        }
    }
}

pub mod vcb;

/// Parse one source VCB measurement, including fractions and decimal commas.
pub fn parse_length(text: &str, default_unit: LengthUnit) -> Result<f64, String> {
    vcb::parse_single(text, default_unit.from_unit_scale())
}

/// Document units retain composite imperial modes in the original payload.
/// A modeled unit change takes precedence over the loaded five-unit projection.
pub fn model_unit(scene: &crate::scene::Scene) -> &str {
    let raw=scene.model_units()["length"].as_str();
    let projected=match raw {Some("cm")=>LengthUnit::Centimeters,Some("mm")=>LengthUnit::Millimeters,Some("in")=>LengthUnit::Inches,Some("ft")=>LengthUnit::Feet,_=>LengthUnit::Meters};
    if scene.unit==projected {
        if let Some(code @ ("m"|"cm"|"mm"|"in"|"ft"|"ft-in"|"in-frac"|"ft-in-frac"))=raw {return code;}
    }
    scene.unit.code()
}
pub const MODEL_UNIT_CODES: [&str;8]=["m","cm","mm","in","ft","ft-in","in-frac","ft-in-frac"];
/// Authored document precision; display formatters clamp it separately.
pub fn model_precision(scene:&crate::scene::Scene)->i64 {
    if scene.model_units()["length"].as_str().is_some_and(|u|MODEL_UNIT_CODES.contains(&u)) {
        scene.model_units()["precision"].as_i64().unwrap_or(2)
    }else{2}
}
pub fn model_scale(scene: &crate::scene::Scene)->f64 {
    match model_unit(scene) {"cm"=>0.01,"mm"=>0.001,"in"|"ft-in"|"in-frac"|"ft-in-frac"=>0.0254,"ft"=>0.3048,_=>1.}
}

/// Format length in meters into string according to specified unit and precision.
pub fn format_length(meters: f64, unit: LengthUnit, decimals: usize) -> String {
    let scaled = meters * unit.to_unit_scale();
    let separator = if matches!(unit, LengthUnit::Feet | LengthUnit::Inches) { "" } else { " " };
    format!("{:.*}{}{}", decimals.min(6), scaled, separator, unit.suffix())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_length_units() {
        assert!((parse_length("1.5m", LengthUnit::Meters).unwrap() - 1.5).abs() < 1e-6);
        assert!((parse_length("1500mm", LengthUnit::Meters).unwrap() - 1.5).abs() < 1e-6);
        assert!((parse_length("25cm", LengthUnit::Meters).unwrap() - 0.25).abs() < 1e-6);
        assert!((parse_length("2.0", LengthUnit::Meters).unwrap() - 2.0).abs() < 1e-6);
    }
}
