// SPDX-License-Identifier: GPL-3.0-or-later
//! Ported from pop-os/cosmic-applets cosmic-applet-network/src/app.rs at ecb6b61.
//!
//! Only the NetworkManager state model is taken: the types, `snapshot_to_applet` and the small
//! helpers it needs, unchanged except where marked `system-applet:`. Upstream's file has no
//! copyright line; its crate is licensed GPL-3.0-or-later. The applet struct, its UI and the
//! secret-agent handling are left out; `crate::adapter::network` builds our view model from
//! this.

// system-applet: not every upstream field and helper is read by our adapter, and upstream's code
// predates a clippy lint; allowed here instead of changing the code.
#![allow(dead_code, clippy::unnecessary_sort_by)]

use indexmap::IndexMap;
use nmrs::{
    ActiveConnection, ActiveConnectionState, ConnectType, ConnectivityState, EapOptions,
    NetworkSnapshot, WifiSecurity,
};
use secure_string::SecureString;
use std::{collections::BTreeMap, fmt, str::FromStr, sync::Arc};

type Uuid = Arc<str>;
type Ssid = Arc<str>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkType {
    Open,
    Password,
    Eap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    Unknown,
    Unmanaged,
    Unavailable,
    Disconnected,
    NeedAuth,
    Activated,
    Failed,
    Other,
}

impl From<&nmrs::DeviceState> for DeviceState {
    fn from(state: &nmrs::DeviceState) -> Self {
        match state {
            nmrs::DeviceState::Unmanaged => Self::Unmanaged,
            nmrs::DeviceState::Unavailable => Self::Unavailable,
            nmrs::DeviceState::Disconnected => Self::Disconnected,
            nmrs::DeviceState::NeedAuth => Self::NeedAuth,
            nmrs::DeviceState::Activated => Self::Activated,
            nmrs::DeviceState::Failed => Self::Failed,
            nmrs::DeviceState::Other(_) => Self::Unknown,
            _ => Self::Other,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HwAddress([u8; 6]);

impl HwAddress {
    fn as_string(self) -> String {
        self.0
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<Vec<_>>()
            .join(":")
    }
}

impl FromStr for HwAddress {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0; 6];
        let mut parts = value.split(':');
        for byte in &mut bytes {
            let Some(part) = parts.next() else {
                return Err(());
            };
            *byte = u8::from_str_radix(part, 16).map_err(|_| ())?;
        }
        if parts.next().is_some() {
            return Err(());
        }
        Ok(Self(bytes))
    }
}

impl fmt::Display for HwAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_string())
    }
}

fn same_access_point(left: &AccessPoint, right: &AccessPoint) -> bool {
    left.ssid == right.ssid
        && left.hw_address == right.hw_address
        && left.interface == right.interface
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPoint {
    pub ssid: Ssid,
    pub network_type: NetworkType,
    pub hw_address: HwAddress,
    pub strength: u8,
    pub state: DeviceState,
    pub working: bool,
    pub wps_push: bool,
    pub interface: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ActiveConnectionInfo {
    Wired {
        name: String,
        hw_address: String,
        speed: u32,
        ip4_address: Option<String>,
        ip6_address: Option<String>,
    },
    WiFi {
        name: String,
        ip4_address: Option<String>,
        ip6_address: Option<String>,
        state: ActiveConnectionState,
        strength: u8,
        hw_address: String,
    },
    Vpn {
        name: String,
        ip4_address: Option<String>,
        ip6_address: Option<String>,
    },
}

impl ActiveConnectionInfo {
    fn name(&self) -> &str {
        match self {
            Self::Wired { name, .. } | Self::WiFi { name, .. } | Self::Vpn { name, .. } => name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkManagerState {
    pub wifi_enabled: bool,
    pub airplane_mode: bool,
    pub connectivity: ConnectivityState,
    pub active_conns: Vec<ActiveConnectionInfo>,
    pub known_access_points: Vec<AccessPoint>,
    pub wireless_access_points: Vec<AccessPoint>,
}

impl Default for NetworkManagerState {
    fn default() -> Self {
        Self {
            wifi_enabled: true,
            airplane_mode: false,
            connectivity: ConnectivityState::Unknown,
            active_conns: Vec::new(),
            known_access_points: Vec::new(),
            wireless_access_points: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceType {
    Wifi,
}

#[derive(Clone, Debug)]
pub struct DeviceConnection {
    pub id: String,
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub interface: String,
    pub device_type: DeviceType,
    pub active_connection: Option<(DeviceConnection,)>,
    pub known_connections: Vec<DeviceConnection>,
}

#[derive(Debug, Clone)]
pub struct AppletSnapshot {
    pub state: NetworkManagerState,
    pub devices: Vec<DeviceInfo>,
    pub known_vpns: IndexMap<Uuid, ConnectionSettings>,
    pub ssid_to_uuid: BTreeMap<Box<str>, Box<str>>,
    pub captive_portal_url: Option<String>,
    // system-applet: every Ethernet device, also unplugged ones, for the Wired section.
    pub wired_devices: Vec<nmrs::Device>,
}

#[derive(Clone, Debug)]
pub enum ConnectionSettings {
    Vpn { id: String },
    Wireguard { id: String },
}

// system-applet: public, used by the adapter and the UI.
pub fn wifi_icon(strength: u8) -> &'static str {
    if strength < 25 {
        "network-wireless-signal-weak-symbolic"
    } else if strength < 50 {
        "network-wireless-signal-ok-symbolic"
    } else if strength < 75 {
        "network-wireless-signal-good-symbolic"
    } else {
        "network-wireless-signal-excellent-symbolic"
    }
}

fn network_type(security: nmrs::SecurityFeatures) -> NetworkType {
    match security.preferred_connect_type() {
        ConnectType::Open | ConnectType::Owe => NetworkType::Open,
        ConnectType::Eap => NetworkType::Eap,
        ConnectType::Psk | ConnectType::Sae => NetworkType::Password,
        _ => NetworkType::Password,
    }
}

// system-applet: public, used by the adapter to connect.
pub fn wifi_security(
    access_point: &AccessPoint,
    identity: Option<String>,
    password: Option<SecureString>,
) -> WifiSecurity {
    match access_point.network_type {
        NetworkType::Open => WifiSecurity::Open,
        NetworkType::Eap => WifiSecurity::WpaEap {
            opts: EapOptions::new(
                identity.unwrap_or_default(),
                password
                    .as_ref()
                    .map(|password| password.unsecure().to_owned())
                    .unwrap_or_default(),
            ),
        },
        NetworkType::Password => WifiSecurity::WpaPsk {
            psk: password
                .as_ref()
                .map(|password| password.unsecure().to_owned())
                .unwrap_or_default(),
        },
    }
}

// system-applet: public, called by the adapter.
pub fn snapshot_to_applet(snapshot: NetworkSnapshot) -> AppletSnapshot {
    let summary = snapshot.applet_summary();
    let mut known_vpns = IndexMap::new();
    for vpn in summary.saved_vpns.values() {
        let uuid: Uuid = Arc::from(vpn.uuid.as_str());
        let entry = match vpn.kind {
            Some(nmrs::VpnKind::WireGuard) => ConnectionSettings::Wireguard { id: vpn.id.clone() },
            _ => ConnectionSettings::Vpn { id: vpn.id.clone() },
        };
        known_vpns.insert(uuid, entry);
    }

    let mut ssid_to_uuid = BTreeMap::new();
    for (ssid, profiles) in &summary.known_wifi {
        if let Some(profile) = profiles.first() {
            ssid_to_uuid.insert(
                ssid.clone().into_boxed_str(),
                profile.uuid.clone().into_boxed_str(),
            );
        }
    }

    let active_conns = snapshot
        .active_connections
        .iter()
        .filter_map(|conn| match conn {
            ActiveConnection::Wired(wired) => Some(ActiveConnectionInfo::Wired {
                name: wired.id.clone(),
                hw_address: wired.hw_address.clone().unwrap_or_default(),
                speed: wired.speed_mbps.unwrap_or_default(),
                ip4_address: wired.ip4_address.clone(),
                ip6_address: wired.ip6_address.clone(),
            }),
            ActiveConnection::Wifi(wifi)
                if known_vpns.values().any(|connection| {
                    matches!(
                        connection,
                        ConnectionSettings::Vpn { id } | ConnectionSettings::Wireguard { id }
                            if id == &wifi.ssid
                    )
                }) =>
            {
                Some(ActiveConnectionInfo::Vpn {
                    name: wifi.ssid.clone(),
                    ip4_address: wifi.ip4_address.clone(),
                    ip6_address: wifi.ip6_address.clone(),
                })
            }
            ActiveConnection::Wifi(wifi) => Some(ActiveConnectionInfo::WiFi {
                name: wifi.ssid.clone(),
                ip4_address: wifi.ip4_address.clone(),
                ip6_address: wifi.ip6_address.clone(),
                state: wifi.state,
                strength: wifi.strength.unwrap_or_default(),
                hw_address: wifi.bssid.clone().unwrap_or_default(),
            }),
            ActiveConnection::Vpn(vpn) => Some(ActiveConnectionInfo::Vpn {
                name: vpn.id.clone(),
                ip4_address: vpn.ip4_address.clone(),
                ip6_address: vpn.ip6_address.clone(),
            }),
            ActiveConnection::Other(_) | _ => None,
        })
        .collect();

    let mut wireless_access_points = summary
        .wifi_groups
        .iter()
        .filter(|group| !group.ssid.is_empty() && !group.strongest.ssid_bytes.is_empty())
        .map(|group| {
            let strongest = &group.strongest;
            AccessPoint {
                ssid: Arc::from(group.ssid.as_str()),
                network_type: network_type(strongest.security),
                hw_address: HwAddress::from_str(&strongest.bssid).unwrap_or_default(),
                strength: strongest.strength,
                state: DeviceState::from(&strongest.device_state),
                working: false,
                wps_push: strongest.security.wps,
                interface: Some(group.interface.clone()),
            }
        })
        .collect::<Vec<_>>();
    wireless_access_points.sort_by(|a, b| b.strength.cmp(&a.strength));

    let known_access_points = summary
        .wifi_groups
        .iter()
        .filter(|group| group.known || group.active)
        .map(|group| {
            let strongest = &group.strongest;
            let device_state = DeviceState::from(&strongest.device_state);
            AccessPoint {
                ssid: Arc::from(group.ssid.as_str()),
                network_type: network_type(strongest.security),
                hw_address: HwAddress::from_str(&strongest.bssid).unwrap_or_default(),
                strength: strongest.strength,
                state: if group.active {
                    DeviceState::Activated
                } else if matches!(device_state, DeviceState::Activated) {
                    DeviceState::Disconnected
                } else {
                    device_state
                },
                working: false,
                wps_push: strongest.security.wps,
                interface: Some(group.interface.clone()),
            }
        })
        .collect::<Vec<_>>();

    let devices = snapshot
        .wifi_devices
        .iter()
        .map(|device| {
            let known_connections = summary
                .known_wifi
                .iter()
                .filter(|(ssid, _)| {
                    summary
                        .wifi_groups
                        .iter()
                        .any(|group| group.interface == device.interface && group.ssid == **ssid)
                })
                .map(|(ssid, _)| DeviceConnection { id: ssid.clone() })
                .collect();
            DeviceInfo {
                interface: device.interface.clone(),
                device_type: DeviceType::Wifi,
                active_connection: device
                    .active_ssid
                    .as_ref()
                    .map(|ssid| (DeviceConnection { id: ssid.clone() },)),
                known_connections,
            }
        })
        .collect();

    AppletSnapshot {
        state: NetworkManagerState {
            wifi_enabled: snapshot.wifi.enabled,
            airplane_mode: summary.airplane_mode.is_airplane_mode(),
            connectivity: summary.connectivity.state,
            active_conns,
            known_access_points,
            wireless_access_points,
        },
        devices,
        known_vpns,
        ssid_to_uuid,
        captive_portal_url: summary.connectivity.captive_portal_url.clone(),
        // system-applet: see `AppletSnapshot::wired_devices`.
        wired_devices: snapshot.wired_devices.clone(),
    }
}

fn active_conn_hw_address(conn: &ActiveConnectionInfo) -> HwAddress {
    match conn {
        ActiveConnectionInfo::Wired { hw_address, .. }
        | ActiveConnectionInfo::WiFi { hw_address, .. } => {
            HwAddress::from_str(hw_address).unwrap_or_default()
        }
        ActiveConnectionInfo::Vpn { .. } => HwAddress::default(),
    }
}
