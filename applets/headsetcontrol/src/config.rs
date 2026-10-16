// SPDX-License-Identifier: MPL-2.0

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use std::collections::BTreeMap;

/// Setting key → last value set through the applet.
pub type DeviceValues = BTreeMap<String, i32>;

/// Stored per field as RON under `~/.config/cosmic/<APP_ID>/v1/<field>`.
#[derive(Debug, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    /// Show settings the headset cannot report back (values are remembered).
    pub show_write_only: bool,
    /// Resend remembered write-only values when the headset comes back online.
    pub reapply_on_reconnect: bool,
    /// Remembered values per device, keyed by `vid:pid`.
    pub stored: BTreeMap<String, DeviceValues>,
    /// Status poll interval while the popup is closed.
    pub poll_secs: u64,
    /// Notify once when the battery drops to this percentage (0 = off).
    pub low_battery_percent: u8,
    /// Use headsetcontrol's mock device (development).
    pub test_device: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_write_only: false,
            reapply_on_reconnect: false,
            stored: BTreeMap::new(),
            poll_secs: 60,
            low_battery_percent: 20,
            test_device: false,
        }
    }
}

impl Config {
    pub fn values(&self, device_key: &str) -> Option<&DeviceValues> {
        self.stored.get(device_key)
    }
}
