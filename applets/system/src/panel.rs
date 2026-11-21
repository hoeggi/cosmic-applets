// SPDX-License-Identifier: GPL-3.0-only

//! Which icons the panel button shows, and which of them are alerts.

use crate::config::PanelIcons;

/// Below this a connected Bluetooth device counts as low.
pub const BT_LOW_PERCENT: u8 = 20;

/// What the panel needs to know about each area, gathered from the backends.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// `Some(icon)` when NetworkManager is running.
    pub network: Option<&'static str>,
    pub network_connected: bool,
    pub vpn_connected: bool,
    /// A VPN went down without being switched off here.
    pub vpn_dropped: bool,
    /// `Some(powered)` with a Bluetooth adapter.
    pub bluetooth: Option<bool>,
    /// Lowest battery of a connected Bluetooth device.
    pub bluetooth_lowest: Option<u8>,
    /// `Some((icon, muted))` with an output.
    pub volume: Option<(&'static str, bool)>,
    /// `Some((icon, percentage, low))` with a battery.
    pub battery: Option<(String, f64, bool)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub icon: String,
    /// Shown in the alert colour.
    pub alert: bool,
    /// Text after the icon (the battery percentage).
    pub label: Option<String>,
}

/// Panel items in a fixed order: network, VPN, Bluetooth, volume, battery. Picked icons always
/// show (red while they have a problem); unpicked ones show only as alerts, if alerts are on.
pub fn items(state: &State, picked: &PanelIcons, show_alerts: bool, percentage: bool) -> Vec<Item> {
    let mut items = Vec::new();
    let mut add = |picked: bool, icon: String, alert: bool, label: Option<String>| {
        if picked || (show_alerts && alert) {
            items.push(Item { icon, alert, label });
        }
    };
    if let Some(icon) = state.network {
        add(picked.network, icon.into(), !state.network_connected, None);
    }
    if state.vpn_connected || state.vpn_dropped {
        add(
            picked.vpn && state.vpn_connected,
            "network-vpn-symbolic".into(),
            state.vpn_dropped,
            None,
        );
    }
    if let Some(powered) = state.bluetooth {
        let low = state.bluetooth_lowest.is_some_and(|p| p < BT_LOW_PERCENT);
        let icon = if powered {
            "bluetooth-active-symbolic"
        } else {
            "bluetooth-disabled-symbolic"
        };
        add(picked.bluetooth, icon.into(), low, None);
    }
    if let Some((icon, muted)) = state.volume {
        add(picked.volume, icon.into(), muted, None);
    }
    if let Some((icon, percent, low)) = &state.battery {
        let label = percentage.then(|| format!("{percent:.0}%"));
        add(picked.battery, icon.clone(), *low, label);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normal() -> State {
        State {
            network: Some("network-wireless-signal-excellent-symbolic"),
            network_connected: true,
            bluetooth: Some(true),
            volume: Some(("audio-volume-high-symbolic", false)),
            battery: Some(("battery-level-80-symbolic".into(), 78.0, false)),
            ..State::default()
        }
    }

    fn icons(items: &[Item]) -> Vec<&str> {
        items.iter().map(|i| i.icon.as_str()).collect()
    }

    #[test]
    fn default_shows_everything_with_percentage() {
        let items = items(&normal(), &PanelIcons::default(), true, true);
        assert_eq!(items.len(), 4);
        assert_eq!(items[3].label.as_deref(), Some("78%"));
        assert!(items.iter().all(|i| !i.alert));
    }

    #[test]
    fn unpicked_icons_appear_only_as_alerts() {
        let picked = PanelIcons {
            network: true,
            vpn: false,
            bluetooth: false,
            volume: false,
            microphone: false,
            battery: true,
        };
        let shown = items(&normal(), &picked, true, true);
        assert_eq!(
            icons(&shown),
            [
                "network-wireless-signal-excellent-symbolic",
                "battery-level-80-symbolic"
            ]
        );

        let mut trouble = normal();
        trouble.network_connected = false;
        trouble.volume = Some(("audio-volume-muted-symbolic", true));
        trouble.battery = Some(("battery-level-10-symbolic".into(), 9.0, true));
        let shown = items(&trouble, &picked, true, true);
        assert_eq!(shown.len(), 3, "muted volume appears although unpicked");
        assert!(shown.iter().all(|i| i.alert));

        let quiet = items(&trouble, &picked, false, true);
        assert_eq!(quiet.len(), 2, "without alerts only picked icons show");
    }

    #[test]
    fn vpn_key_while_connected_and_red_when_dropped() {
        let mut state = normal();
        state.vpn_connected = true;
        let shown = items(&state, &PanelIcons::default(), true, false);
        assert!(icons(&shown).contains(&"network-vpn-symbolic"));

        state.vpn_connected = false;
        state.vpn_dropped = true;
        let shown = items(&state, &PanelIcons::default(), true, false);
        let vpn = shown
            .iter()
            .find(|i| i.icon == "network-vpn-symbolic")
            .unwrap();
        assert!(vpn.alert);
    }

    #[test]
    fn low_bluetooth_device_is_an_alert() {
        let mut state = normal();
        state.bluetooth_lowest = Some(15);
        let shown = items(&state, &PanelIcons::default(), true, false);
        assert!(
            shown
                .iter()
                .any(|i| i.icon == "bluetooth-active-symbolic" && i.alert)
        );
    }
}
