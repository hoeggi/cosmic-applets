// SPDX-License-Identifier: GPL-3.0-only

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

/// A slider in the main popup, below the battery header. Their order is a user setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Slider {
    Volume,
    Microphone,
    /// Display brightness; hidden without a backlight.
    Brightness,
}

impl Slider {
    pub const ALL: [Slider; 3] = [Slider::Volume, Slider::Microphone, Slider::Brightness];
}

/// A tile in the main popup's two-column grid. Their order fills the grid top left, top right,
/// bottom left, bottom right; a hidden or unavailable tile leaves no gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tile {
    /// Neutral: shows the connection in use and opens the Network page.
    Network,
    Bluetooth,
    /// Default output; the body toggles mute, the chevron opens the Sound page.
    Output,
    /// Only on machines with Wi-Fi.
    Airplane,
    /// Only with a VPN profile; off by default.
    Vpn,
}

impl Tile {
    pub const ALL: [Tile; 5] = [
        Tile::Network,
        Tile::Bluetooth,
        Tile::Output,
        Tile::Airplane,
        Tile::Vpn,
    ];

    fn default_enabled(self) -> bool {
        !matches!(self, Tile::Vpn)
    }
}

/// One orderable item and whether it's shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry<T> {
    pub item: T,
    pub enabled: bool,
}

/// Every item exactly once, in the configured order: duplicates are dropped and items missing
/// from an older or hand-edited config are appended with their default, so a new item never
/// disappears silently.
pub fn normalized<T: Copy + PartialEq>(
    entries: &[Entry<T>],
    all: &[T],
    default_enabled: impl Fn(T) -> bool,
) -> Vec<Entry<T>> {
    let mut out: Vec<Entry<T>> = Vec::new();
    for entry in entries {
        if all.contains(&entry.item) && !out.iter().any(|e| e.item == entry.item) {
            out.push(*entry);
        }
    }
    for &item in all {
        if !out.iter().any(|e| e.item == item) {
            out.push(Entry {
                item,
                enabled: default_enabled(item),
            });
        }
    }
    out
}

/// Moves the entry at `index` one up (or down); no-op at the ends.
pub fn move_entry<T>(entries: &mut [Entry<T>], index: usize, up: bool) {
    let other = if up {
        index.checked_sub(1)
    } else {
        Some(index + 1)
    };
    if let Some(other) = other.filter(|o| *o < entries.len())
        && index < entries.len()
    {
        entries.swap(index, other);
    }
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
    /// The battery header at the top of the main popup.
    pub battery_header: bool,
    /// Sliders, top to bottom.
    pub sliders: Vec<Entry<Slider>>,
    /// Tiles, top left to bottom right.
    pub tiles: Vec<Entry<Tile>>,
    /// One row per playing or paused media player, below the tiles.
    pub media_controls: bool,
    /// Lock, log out, suspend, restart, shut down and COSMIC Settings.
    pub power_buttons: bool,
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
            battery_header: true,
            sliders: normalized(&[], &Slider::ALL, |_| true),
            tiles: normalized(&[], &Tile::ALL, Tile::default_enabled),
            media_controls: true,
            power_buttons: true,
            panel_icons: PanelIcons::default(),
            battery_percentage: true,
            show_alerts: true,
            confirm_shutdown: true,
        }
    }
}

impl Config {
    pub fn slider_entries(&self) -> Vec<Entry<Slider>> {
        normalized(&self.sliders, &Slider::ALL, |_| true)
    }

    pub fn tile_entries(&self) -> Vec<Entry<Tile>> {
        normalized(&self.tiles, &Tile::ALL, Tile::default_enabled)
    }

    /// Enabled sliders, top to bottom.
    pub fn visible_sliders(&self) -> Vec<Slider> {
        enabled(self.slider_entries())
    }

    /// Enabled tiles, top left to bottom right.
    pub fn visible_tiles(&self) -> Vec<Tile> {
        enabled(self.tile_entries())
    }
}

fn enabled<T>(entries: Vec<Entry<T>>) -> Vec<T> {
    entries
        .into_iter()
        .filter(|e| e.enabled)
        .map(|e| e.item)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let config = Config::default();
        assert_eq!(config.visible_sliders(), Slider::ALL.to_vec());
        assert_eq!(
            config.visible_tiles(),
            [Tile::Network, Tile::Bluetooth, Tile::Output, Tile::Airplane]
        );
    }

    #[test]
    fn duplicates_dropped_missing_appended() {
        let config = Config {
            tiles: vec![
                Entry {
                    item: Tile::Airplane,
                    enabled: true,
                },
                Entry {
                    item: Tile::Network,
                    enabled: false,
                },
                Entry {
                    item: Tile::Airplane,
                    enabled: false,
                },
            ],
            ..Config::default()
        };
        let entries = config.tile_entries();
        let order: Vec<_> = entries.iter().map(|e| e.item).collect();
        assert_eq!(
            order,
            [
                Tile::Airplane,
                Tile::Network,
                Tile::Bluetooth,
                Tile::Output,
                Tile::Vpn
            ]
        );
        assert_eq!(
            config.visible_tiles(),
            [Tile::Airplane, Tile::Bluetooth, Tile::Output]
        );
    }

    #[test]
    fn moving_stays_inside_the_list() {
        let mut entries = Config::default().slider_entries();
        move_entry(&mut entries, 0, true);
        assert_eq!(entries[0].item, Slider::Volume, "top stays put");
        move_entry(&mut entries, 0, false);
        assert_eq!(entries[1].item, Slider::Volume);
        move_entry(&mut entries, 2, false);
        assert_eq!(entries[2].item, Slider::Brightness, "bottom stays put");
    }
}
