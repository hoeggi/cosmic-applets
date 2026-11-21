// SPDX-License-Identifier: GPL-3.0-only

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

/// A block of the main popup. Their order and visibility are user settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Section {
    /// Battery charge, time left and power mode; hidden without a battery.
    Battery,
    Volume,
    Microphone,
    /// Display brightness; hidden without a backlight.
    Brightness,
    /// The Wi-Fi (or wired) tile and airplane mode.
    Network,
    Bluetooth,
    /// One row per running media player, between the tiles and the power row.
    Media,
    /// Lock, log out, suspend, restart and shut down.
    PowerButtons,
}

impl Section {
    pub const ALL: [Section; 8] = [
        Section::Battery,
        Section::Volume,
        Section::Microphone,
        Section::Brightness,
        Section::Network,
        Section::Bluetooth,
        Section::Media,
        Section::PowerButtons,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionEntry {
    pub section: Section,
    pub enabled: bool,
}

/// Which icons the panel button shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelIcons {
    pub network: bool,
    /// Only while a VPN is connected.
    pub vpn: bool,
    pub bluetooth: bool,
    pub volume: bool,
    /// Only while an app is recording.
    pub microphone: bool,
    pub battery: bool,
}

impl Default for PanelIcons {
    fn default() -> Self {
        Self {
            network: true,
            vpn: true,
            bluetooth: true,
            volume: true,
            microphone: true,
            battery: true,
        }
    }
}

/// Stored per field as RON under `~/.config/cosmic/<APP_ID>/v1/<field>`.
#[derive(Debug, Clone, CosmicConfigEntry, PartialEq)]
#[version = 1]
pub struct Config {
    /// Main popup sections in display order.
    pub sections: Vec<SectionEntry>,
    pub panel_icons: PanelIcons,
    /// Percentage next to the battery icon in the panel.
    pub battery_percentage: bool,
    /// Unpicked panel icons appear, in red, while something needs attention.
    pub show_alerts: bool,
    /// Restart and Shut down ask first, with a countdown.
    pub confirm_shutdown: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sections: Section::ALL
                .iter()
                .map(|&section| SectionEntry {
                    section,
                    enabled: true,
                })
                .collect(),
            panel_icons: PanelIcons::default(),
            battery_percentage: true,
            show_alerts: true,
            confirm_shutdown: true,
        }
    }
}

impl Config {
    /// Enabled sections in display order. Sections missing from a hand-edited config are
    /// appended, so a new section never disappears silently.
    pub fn visible_sections(&self) -> Vec<Section> {
        let mut seen = Vec::new();
        let mut out = Vec::new();
        for entry in &self.sections {
            if seen.contains(&entry.section) {
                continue;
            }
            seen.push(entry.section);
            if entry.enabled {
                out.push(entry.section);
            }
        }
        out.extend(Section::ALL.iter().filter(|s| !seen.contains(s)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_show_everything_in_order() {
        assert_eq!(Config::default().visible_sections(), Section::ALL.to_vec());
    }

    #[test]
    fn disabled_duplicate_and_missing_sections() {
        let config = Config {
            sections: vec![
                SectionEntry {
                    section: Section::Volume,
                    enabled: true,
                },
                SectionEntry {
                    section: Section::Battery,
                    enabled: false,
                },
                SectionEntry {
                    section: Section::Volume,
                    enabled: false,
                },
            ],
            ..Config::default()
        };
        let visible = config.visible_sections();
        assert_eq!(visible[0], Section::Volume);
        assert!(!visible.contains(&Section::Battery));
        // The duplicate entry is ignored, the rest are appended in their default order.
        assert_eq!(visible.len(), 7);
        assert_eq!(visible[1], Section::Microphone);
    }
}
