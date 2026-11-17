// SPDX-License-Identifier: MPL-2.0

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

/// Refresh intervals offered in the settings, in minutes. Open-Meteo's models update hourly.
pub const REFRESH_CHOICES: [u64; 4] = [10, 15, 30, 60];

/// A saved place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub name: String,
    /// Region and country, e.g. "Tirol, Österreich"; empty for coordinates typed in by hand.
    pub detail: String,
    pub latitude: f64,
    pub longitude: f64,
    /// Metres; shown in the settings only.
    pub elevation: Option<f64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Units {
    /// °C, km/h, mm (snow in cm).
    #[default]
    Metric,
    /// °F, mph, inch.
    Imperial,
}

/// Stored per field as RON under `~/.config/cosmic/<APP_ID>/v1/<field>`.
#[derive(Debug, Clone, CosmicConfigEntry, PartialEq)]
#[version = 1]
pub struct Config {
    /// Empty until the first place is added; nothing is fetched before that.
    pub locations: Vec<Location>,
    /// Index into `locations` of the place shown in the panel.
    pub active: usize,
    pub units: Units,
    /// Temperature next to the icon in the panel.
    pub panel_temperature: bool,
    /// Start of rain or snow within the next 2 hours in the panel.
    pub panel_precipitation: bool,
    /// Today's high and low in the panel.
    pub panel_high_low: bool,
    /// Air quality and UV in the popup (one more request per refresh).
    pub show_air: bool,
    /// Pollen in the popup (Europe only; same request as air quality).
    pub show_pollen: bool,
    pub refresh_mins: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            locations: Vec::new(),
            active: 0,
            units: Units::Metric,
            panel_temperature: true,
            panel_precipitation: false,
            panel_high_low: false,
            show_air: false,
            show_pollen: false,
            refresh_mins: 15,
        }
    }
}

impl Config {
    /// The active place; falls back to the first one if `active` is out of range.
    pub fn location(&self) -> Option<&Location> {
        self.locations
            .get(self.active)
            .or_else(|| self.locations.first())
    }

    pub fn wants_air(&self) -> bool {
        self.show_air || self.show_pollen
    }

    /// What a fetch depends on; a change means the shown data is outdated.
    pub fn request(&self) -> (Option<Location>, Units, bool) {
        (self.location().cloned(), self.units, self.wants_air())
    }

    pub fn refresh_mins(&self) -> u64 {
        self.refresh_mins.max(REFRESH_CHOICES[0])
    }
}

impl Location {
    /// Name for coordinates entered by hand.
    pub fn from_coordinates(latitude: f64, longitude: f64) -> Self {
        Self {
            name: format!("{latitude:.4}, {longitude:.4}"),
            detail: String::new(),
            latitude,
            longitude,
            elevation: None,
        }
    }
}

/// Parses a latitude / longitude typed by hand (dot or comma as decimal separator).
pub fn parse_coordinate(input: &str, limit: f64) -> Option<f64> {
    let value: f64 = input.trim().replace(',', ".").parse().ok()?;
    (value.is_finite() && value.abs() <= limit).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(name: &str) -> Location {
        Location {
            name: name.into(),
            ..Location::from_coordinates(48.2, 16.4)
        }
    }

    #[test]
    fn active_location_falls_back_to_the_first() {
        let mut config = Config::default();
        assert_eq!(config.location(), None);
        config.locations = vec![place("a"), place("b")];
        config.active = 1;
        assert_eq!(config.location().unwrap().name, "b");
        config.active = 5;
        assert_eq!(config.location().unwrap().name, "a");
    }

    #[test]
    fn coordinates() {
        assert_eq!(parse_coordinate(" 48,2428 ", 90.0), Some(48.2428));
        assert_eq!(parse_coordinate("-16.3", 180.0), Some(-16.3));
        assert_eq!(parse_coordinate("91", 90.0), None);
        assert_eq!(parse_coordinate("abc", 90.0), None);
        assert_eq!(parse_coordinate("NaN", 90.0), None);
        assert_eq!(
            Location::from_coordinates(48.24, 16.37).name,
            "48.2400, 16.3700"
        );
    }
}
