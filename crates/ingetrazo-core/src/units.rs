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
        1.0 / self.to_unit_scale()
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

/// Parse a length string typed in the VCB (e.g. "1.5m", "1500mm", "25cm", "12\"", "3'") into meters.
pub fn parse_length(text: &str, default_unit: LengthUnit) -> Result<f64, String> {
    let s = text.trim();
    if s.is_empty() {
        return Err("Empty input".to_string());
    }

    // Check unit suffix
    if let Some(rest) = s.strip_suffix("mm") {
        let val: f64 = rest.trim().parse().map_err(|e| format!("Invalid number: {e}"))?;
        return Ok(val * LengthUnit::Millimeters.from_unit_scale());
    }
    if let Some(rest) = s.strip_suffix("cm") {
        let val: f64 = rest.trim().parse().map_err(|e| format!("Invalid number: {e}"))?;
        return Ok(val * LengthUnit::Centimeters.from_unit_scale());
    }
    if let Some(rest) = s.strip_suffix('m') {
        let val: f64 = rest.trim().parse().map_err(|e| format!("Invalid number: {e}"))?;
        return Ok(val * LengthUnit::Meters.from_unit_scale());
    }
    if let Some(rest) = s.strip_suffix('"').or_else(|| s.strip_suffix("in")) {
        let val: f64 = rest.trim().parse().map_err(|e| format!("Invalid number: {e}"))?;
        return Ok(val * LengthUnit::Inches.from_unit_scale());
    }
    if let Some(rest) = s.strip_suffix('\'').or_else(|| s.strip_suffix("ft")) {
        let val: f64 = rest.trim().parse().map_err(|e| format!("Invalid number: {e}"))?;
        return Ok(val * LengthUnit::Feet.from_unit_scale());
    }

    // No suffix: use default unit
    let val: f64 = s.parse().map_err(|e| format!("Invalid number: {e}"))?;
    Ok(val * default_unit.from_unit_scale())
}

/// Format length in meters into string according to specified unit and precision.
pub fn format_length(meters: f64, unit: LengthUnit, decimals: usize) -> String {
    let scaled = meters * unit.to_unit_scale();
    format!("{:.*} {}", decimals, scaled, unit.suffix())
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
