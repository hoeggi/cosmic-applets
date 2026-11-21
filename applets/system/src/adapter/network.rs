// SPDX-License-Identifier: GPL-3.0-only

//! Network state for our UI, built from upstream's NetworkManager model
//! (`crate::upstream::network`, through `nmrs`). Commands use the same `nmrs` calls as upstream's
//! applet: `set_wireless_enabled`, `set_airplane_mode`, `connect_to_bssid`, `disconnect`,
//! `connect_by_uuid` and `disconnect_by_uuid`.

// `nmrs::ConnectionError` is large; its results are only passed up to be logged.
#![allow(clippy::result_large_err)]

use crate::upstream::network::{
    self as up, AccessPoint, ActiveConnectionInfo, AppletSnapshot, ConnectionSettings, NetworkType,
};
use cosmic::iced::{Subscription, futures::SinkExt, stream};
use futures_util::StreamExt;
use nmrs::NetworkManager;
use secure_string::SecureString;
use std::time::Duration;
use tokio::sync::mpsc;

pub use crate::upstream::network::wifi_icon;

/// Events often come in bursts (a connection touches several objects); wait this long before
/// taking a new snapshot.
const THROTTLE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq)]
pub struct WiredState {
    /// Kernel interface name, e.g. `enp0s31f6` or `enx00e04c68…` for a USB adapter.
    pub interface: String,
    pub connected: bool,
    /// Mb/s, when connected and known.
    pub speed: Option<u32>,
    /// Carries the traffic (the first connected wired device while wired is in use).
    pub in_use: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccessPointInfo {
    pub ssid: String,
    pub strength: u8,
    pub secured: bool,
    /// A saved connection exists, so connecting needs no password.
    pub known: bool,
    /// Upstream's access point, passed back to connect.
    pub ap: AccessPoint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveWifi {
    pub ssid: String,
    pub strength: u8,
    pub secured: bool,
    pub in_use: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Vpn {
    pub name: String,
    pub uuid: String,
    pub connected: bool,
}

/// Which link carries traffic.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Primary {
    #[default]
    None,
    Wired,
    Wifi,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Network {
    pub available: bool,
    /// A Wi-Fi card is present.
    pub has_wifi: bool,
    pub wifi_enabled: bool,
    pub airplane_mode: bool,
    /// Every Ethernet device, connected ones first.
    pub wired: Vec<WiredState>,
    pub wifi: Option<ActiveWifi>,
    /// Other networks in range, strongest first, one per SSID.
    pub access_points: Vec<AccessPointInfo>,
    pub vpns: Vec<Vpn>,
    pub primary: Primary,
}

impl Network {
    pub fn vpn_connected(&self) -> bool {
        self.vpns.iter().any(|v| v.connected)
    }

    pub fn wired_connected(&self) -> bool {
        self.wired.iter().any(|w| w.connected)
    }

    pub fn connected(&self) -> bool {
        self.primary != Primary::None
    }

    /// Icon of the connection in use.
    pub fn icon(&self) -> &'static str {
        match (self.primary, &self.wifi) {
            (Primary::Wired, _) => "network-wired-symbolic",
            (Primary::Wifi, Some(wifi)) => wifi_icon(wifi.strength),
            _ if self.has_wifi && !self.wifi_enabled => "network-wireless-disabled-symbolic",
            _ => "network-wireless-no-route-symbolic",
        }
    }
}

/// A wired device as the mapping needs it (`nmrs::Device` can't be built in tests).
#[derive(Debug, Clone, PartialEq)]
pub struct WiredDevice {
    pub interface: String,
    pub connected: bool,
    pub speed: Option<u32>,
}

impl From<&nmrs::Device> for WiredDevice {
    fn from(device: &nmrs::Device) -> Self {
        let connected = device.state == nmrs::DeviceState::Activated;
        Self {
            interface: device.interface.clone(),
            connected,
            speed: device.speed_mbps.filter(|s| connected && *s > 0),
        }
    }
}

/// Our view of upstream's snapshot. `has_wifi` comes from upstream's Wi-Fi device list.
pub fn build(snapshot: &AppletSnapshot, wired: &[WiredDevice]) -> Network {
    let state = &snapshot.state;
    let wired_active = state
        .active_conns
        .iter()
        .any(|c| matches!(c, ActiveConnectionInfo::Wired { .. }));
    let wifi_active = state.active_conns.iter().find_map(|c| match c {
        ActiveConnectionInfo::WiFi { name, strength, .. } => Some((name.clone(), *strength)),
        _ => None,
    });
    // NetworkManager prefers wired over Wi-Fi by route metric unless configured otherwise.
    let primary = if wired_active || wired.iter().any(|w| w.connected) {
        Primary::Wired
    } else if wifi_active.is_some() {
        Primary::Wifi
    } else {
        Primary::None
    };

    let mut wired_states: Vec<WiredState> = wired
        .iter()
        .map(|w| WiredState {
            interface: w.interface.clone(),
            connected: w.connected,
            speed: w.speed,
            in_use: false,
        })
        .collect();
    wired_states.sort_by_key(|w| !w.connected);
    if primary == Primary::Wired
        && let Some(first) = wired_states.iter_mut().find(|w| w.connected)
    {
        first.in_use = true;
    }

    let current_ssid = wifi_active.as_ref().map(|(ssid, _)| ssid.clone());
    let wifi = wifi_active.map(|(ssid, strength)| {
        let secured = state
            .wireless_access_points
            .iter()
            .chain(&state.known_access_points)
            .find(|ap| *ap.ssid == *ssid)
            .is_some_and(|ap| ap.network_type != NetworkType::Open);
        ActiveWifi {
            ssid,
            strength,
            secured,
            in_use: primary == Primary::Wifi,
        }
    });

    let mut seen = Vec::new();
    let access_points = state
        .wireless_access_points
        .iter()
        .filter(|ap| Some(ap.ssid.as_ref()) != current_ssid.as_deref())
        .filter(|ap| {
            let new = !seen.contains(&ap.ssid);
            seen.push(ap.ssid.clone());
            new
        })
        .map(|ap| AccessPointInfo {
            ssid: ap.ssid.to_string(),
            strength: ap.strength,
            secured: ap.network_type != NetworkType::Open,
            known: snapshot.ssid_to_uuid.contains_key(ap.ssid.as_ref()),
            ap: ap.clone(),
        })
        .collect();

    let active_vpns: Vec<&str> = state
        .active_conns
        .iter()
        .filter_map(|c| match c {
            ActiveConnectionInfo::Vpn { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    let vpns = snapshot
        .known_vpns
        .iter()
        .map(|(uuid, settings)| {
            let name = match settings {
                ConnectionSettings::Vpn { id } | ConnectionSettings::Wireguard { id } => id.clone(),
            };
            Vpn {
                connected: active_vpns.contains(&name.as_str()),
                name,
                uuid: uuid.to_string(),
            }
        })
        .collect();

    Network {
        available: true,
        has_wifi: !snapshot.devices.is_empty(),
        wifi_enabled: state.wifi_enabled,
        airplane_mode: state.airplane_mode,
        wired: wired_states,
        wifi,
        access_points,
        vpns,
        primary,
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    SetWifi(bool),
    Scan,
    /// Connect to an access point; `password` for a new secured network.
    ConnectWifi {
        ap: AccessPoint,
        password: Option<String>,
    },
    DisconnectWifi,
    /// Connect or disconnect a VPN by its connection UUID.
    SetVpn(String, bool),
    /// NetworkManager's airplane mode (Wi-Fi, mobile broadband and Bluetooth radios).
    Airplane(bool),
}

#[derive(Debug, Clone)]
pub enum Update {
    Ready(mpsc::UnboundedSender<Command>),
    State(Box<Network>),
}

pub fn subscription() -> Subscription<Update> {
    Subscription::run(|| {
        stream::channel(8, async |mut out: futures_channel::mpsc::Sender<Update>| {
            loop {
                if let Err(err) = run(&mut out).await {
                    tracing::debug!(%err, "NetworkManager unavailable");
                }
                _ = out.send(Update::State(Box::default())).await;
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        })
    })
}

async fn snapshot(nm: &NetworkManager) -> nmrs::Result<Network> {
    let applet = up::snapshot_to_applet(nm.snapshot().await?);
    let wired: Vec<WiredDevice> = applet.wired_devices.iter().map(WiredDevice::from).collect();
    Ok(build(&applet, &wired))
}

async fn run(out: &mut futures_channel::mpsc::Sender<Update>) -> nmrs::Result<()> {
    let nm = NetworkManager::new().await?;
    let mut events = nm.network_events().await?;
    let (tx, mut commands) = mpsc::unbounded_channel();
    _ = out.send(Update::Ready(tx)).await;
    let mut state = snapshot(&nm).await?;
    _ = out.send(Update::State(Box::new(state.clone()))).await;
    loop {
        tokio::select! {
            event = events.next() => {
                match event {
                    None => return Ok(()),
                    Some(Err(err)) => tracing::debug!(%err, "network event"),
                    Some(Ok(_)) => {}
                }
                tokio::time::sleep(THROTTLE).await;
                // Drain what arrived meanwhile; one snapshot covers it all.
                while let Some(Some(_)) = futures_util::FutureExt::now_or_never(events.next()) {}
            }
            Some(command) = commands.recv() => {
                if let Err(err) = execute(&nm, &state, command).await {
                    tracing::warn!(%err, "network command failed");
                }
            }
        }
        state = snapshot(&nm).await?;
        _ = out.send(Update::State(Box::new(state.clone()))).await;
    }
}

/// The same `nmrs` calls as upstream's message handlers.
async fn execute(nm: &NetworkManager, state: &Network, command: Command) -> nmrs::Result<()> {
    match command {
        Command::SetWifi(on) => nm.set_wireless_enabled(on).await,
        Command::Airplane(on) => nm.set_airplane_mode(on).await,
        Command::Scan => nm.scan_networks(None).await,
        Command::ConnectWifi { ap, password } => {
            let ssid = ap.ssid.to_string();
            let bssid = ap.hw_address.to_string();
            let security = up::wifi_security(&ap, None, password.map(SecureString::from));
            nm.connect_to_bssid(&ssid, Some(&bssid), ap.interface.as_deref(), security)
                .await
        }
        Command::DisconnectWifi => {
            let interface = state.wifi.as_ref().and_then(|wifi| {
                state
                    .access_points
                    .iter()
                    .find(|a| a.ssid == wifi.ssid)
                    .and_then(|a| a.ap.interface.clone())
            });
            nm.disconnect(interface.as_deref()).await
        }
        Command::SetVpn(uuid, true) => nm
            .connect_by_uuid(&uuid, nmrs::ConnectByUuidConfig::default())
            .await
            .map(|_| ()),
        Command::SetVpn(uuid, false) => nm.disconnect_by_uuid(&uuid).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::upstream::network::{
        DeviceInfo, DeviceState, DeviceType, HwAddress, NetworkManagerState,
    };
    use indexmap::IndexMap;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn ap(ssid: &str, strength: u8, network_type: NetworkType) -> AccessPoint {
        AccessPoint {
            ssid: Arc::from(ssid),
            network_type,
            hw_address: HwAddress::default(),
            strength,
            state: DeviceState::Disconnected,
            working: false,
            wps_push: false,
            interface: Some("wlp0s20f3".into()),
        }
    }

    fn snapshot(active_conns: Vec<ActiveConnectionInfo>, aps: Vec<AccessPoint>) -> AppletSnapshot {
        AppletSnapshot {
            state: NetworkManagerState {
                wifi_enabled: true,
                active_conns,
                wireless_access_points: aps,
                ..NetworkManagerState::default()
            },
            devices: vec![DeviceInfo {
                interface: "wlp0s20f3".into(),
                device_type: DeviceType::Wifi,
                active_connection: None,
                known_connections: Vec::new(),
            }],
            known_vpns: IndexMap::new(),
            ssid_to_uuid: BTreeMap::new(),
            captive_portal_url: None,
            wired_devices: Vec::new(),
        }
    }

    fn wired_conn() -> ActiveConnectionInfo {
        ActiveConnectionInfo::Wired {
            name: "Wired connection 2".into(),
            hw_address: "00:E0:4C:68:00:01".into(),
            speed: 1000,
            ip4_address: None,
            ip6_address: None,
        }
    }

    fn wifi_conn(ssid: &str) -> ActiveConnectionInfo {
        ActiveConnectionInfo::WiFi {
            name: ssid.into(),
            ip4_address: None,
            ip6_address: None,
            state: nmrs::ActiveConnectionState::Activated,
            strength: 80,
            hw_address: String::new(),
        }
    }

    #[test]
    fn usb_ethernet_connected_while_built_in_port_is_unplugged() {
        let wired = [
            WiredDevice {
                interface: "enp0s31f6".into(),
                connected: false,
                speed: None,
            },
            WiredDevice {
                interface: "enx00e04c680001".into(),
                connected: true,
                speed: Some(1000),
            },
        ];
        let net = build(&snapshot(vec![wired_conn()], Vec::new()), &wired);
        assert_eq!(net.primary, Primary::Wired);
        assert_eq!(net.icon(), "network-wired-symbolic");
        assert_eq!(net.wired.len(), 2, "both ports are listed");
        assert_eq!(net.wired[0].interface, "enx00e04c680001");
        assert!(net.wired[0].connected && net.wired[0].in_use);
        assert!(!net.wired[1].connected && !net.wired[1].in_use);
    }

    #[test]
    fn wifi_is_standby_while_wired_is_in_use() {
        let wired = [WiredDevice {
            interface: "enx00e04c680001".into(),
            connected: true,
            speed: Some(1000),
        }];
        let snap = snapshot(
            vec![wired_conn(), wifi_conn("Home-5G")],
            vec![ap("Home-5G", 80, NetworkType::Password)],
        );
        let net = build(&snap, &wired);
        assert_eq!(net.primary, Primary::Wired);
        let wifi = net.wifi.expect("Wi-Fi stays connected");
        assert!(!wifi.in_use && wifi.secured);
        assert!(
            net.access_points.is_empty(),
            "the current network isn't listed again"
        );
    }

    #[test]
    fn access_points_one_per_ssid_without_the_current_one() {
        let snap = snapshot(
            vec![wifi_conn("Home-5G")],
            vec![
                ap("Guest", 90, NetworkType::Open),
                ap("Home-5G", 80, NetworkType::Password),
                ap("Office", 40, NetworkType::Password),
                ap("Office", 20, NetworkType::Password),
            ],
        );
        let net = build(&snap, &[]);
        assert_eq!(net.primary, Primary::Wifi);
        let list: Vec<_> = net
            .access_points
            .iter()
            .map(|a| (a.ssid.as_str(), a.strength, a.secured))
            .collect();
        assert_eq!(list, [("Guest", 90, false), ("Office", 40, true)]);
    }

    #[test]
    fn vpns_connected_by_name() {
        let mut snap = snapshot(
            vec![
                wired_conn(),
                ActiveConnectionInfo::Vpn {
                    name: "Office VPN".into(),
                    ip4_address: None,
                    ip6_address: None,
                },
            ],
            Vec::new(),
        );
        snap.known_vpns.insert(
            Arc::from("uuid-1"),
            ConnectionSettings::Vpn {
                id: "Office VPN".into(),
            },
        );
        snap.known_vpns.insert(
            Arc::from("uuid-2"),
            ConnectionSettings::Wireguard { id: "Home".into() },
        );
        let net = build(&snap, &[]);
        assert!(net.vpn_connected());
        assert_eq!(
            net.vpns
                .iter()
                .map(|v| (v.name.as_str(), v.uuid.as_str(), v.connected))
                .collect::<Vec<_>>(),
            [("Office VPN", "uuid-1", true), ("Home", "uuid-2", false)]
        );
    }

    #[test]
    fn nothing_connected() {
        let mut snap = snapshot(Vec::new(), Vec::new());
        snap.state.wifi_enabled = false;
        let net = build(&snap, &[]);
        assert_eq!(net.primary, Primary::None);
        assert_eq!(net.icon(), "network-wireless-disabled-symbolic");
    }
}
