// SPDX-License-Identifier: GPL-3.0-only

//! Wired, Wi-Fi and VPN through NetworkManager on the system bus. Like the Bluetooth backend,
//! one connection lives in the subscription and also runs the commands.

use cosmic::iced::{Subscription, stream};
use futures_util::{FutureExt, SinkExt, StreamExt};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};
use zbus::{Connection, MatchRule, MessageStream, proxy};

const NM: &str = "org.freedesktop.NetworkManager";
/// NetworkManager signals constantly (signal strength); re-read at most this often.
const THROTTLE: Duration = Duration::from_secs(1);

type Settings = HashMap<String, HashMap<String, OwnedValue>>;

#[proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    fn get_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    fn activate_connection(
        &self,
        connection: &ObjectPath<'_>,
        device: &ObjectPath<'_>,
        specific_object: &ObjectPath<'_>,
    ) -> zbus::Result<OwnedObjectPath>;
    fn add_and_activate_connection(
        &self,
        connection: HashMap<&str, HashMap<&str, Value<'_>>>,
        device: &ObjectPath<'_>,
        specific_object: &ObjectPath<'_>,
    ) -> zbus::Result<(OwnedObjectPath, OwnedObjectPath)>;
    fn deactivate_connection(&self, active: &ObjectPath<'_>) -> zbus::Result<()>;
    #[zbus(property)]
    fn active_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    #[zbus(property)]
    fn primary_connection(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn wireless_enabled(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_wireless_enabled(&self, enabled: bool) -> zbus::Result<()>;
    #[zbus(property)]
    fn wwan_enabled(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn set_wwan_enabled(&self, enabled: bool) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Device {
    #[zbus(property)]
    fn device_type(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn active_connection(&self) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wired",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Wired {
    #[zbus(property)]
    fn carrier(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn speed(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Wireless {
    fn get_all_access_points(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    fn request_scan(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;
    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn bitrate(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager"
)]
trait AccessPoint {
    #[zbus(property)]
    fn ssid(&self) -> zbus::Result<Vec<u8>>;
    #[zbus(property)]
    fn strength(&self) -> zbus::Result<u8>;
    #[zbus(property)]
    fn flags(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn wpa_flags(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn rsn_flags(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager"
)]
trait Active {
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;
    #[zbus(property, name = "Type")]
    fn kind(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn vpn(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn connection(&self) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Settings",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/Settings"
)]
trait SettingsManager {
    fn list_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager.Settings.Connection",
    default_service = "org.freedesktop.NetworkManager"
)]
trait SavedConnection {
    fn get_settings(&self) -> zbus::Result<Settings>;
}

const DEVICE_ETHERNET: u32 = 1;
const DEVICE_WIFI: u32 = 2;
/// NM_DEVICE_STATE_ACTIVATED, NM_ACTIVE_CONNECTION_STATE_ACTIVATED.
const DEVICE_ACTIVATED: u32 = 100;
const ACTIVE_ACTIVATED: u32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct WiredState {
    pub connected: bool,
    /// Mb/s, when connected.
    pub speed: Option<u32>,
    pub in_use: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccessPointInfo {
    pub ssid: String,
    pub strength: u8,
    pub secured: bool,
    /// A saved connection exists, so connecting needs no password.
    pub known: bool,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActiveWifi {
    pub ssid: String,
    pub strength: u8,
    pub secured: bool,
    /// Mb/s.
    pub bitrate: u32,
    pub in_use: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Vpn {
    pub name: String,
    /// Saved connection.
    pub path: String,
    /// The active connection, while connected or connecting.
    pub active: Option<String>,
    pub connected: bool,
}

/// Which link carries traffic (NetworkManager's primary connection).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Primary {
    #[default]
    None,
    Wired,
    Wifi,
    /// E.g. a mobile broadband or bridged connection.
    Other,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Network {
    pub available: bool,
    /// A Wi-Fi card is present.
    pub has_wifi: bool,
    pub wifi_enabled: bool,
    pub wired: Option<WiredState>,
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

    pub fn connected(&self) -> bool {
        self.primary != Primary::None
    }

    /// Icon of the connection in use.
    pub fn icon(&self) -> &'static str {
        match (self.primary, &self.wifi) {
            (Primary::Wired, _) => "network-wired-symbolic",
            (Primary::Wifi, Some(wifi)) => wifi_icon(wifi.strength),
            (Primary::Other, _) => "network-transmit-receive-symbolic",
            _ if self.has_wifi && !self.wifi_enabled => "network-wireless-disabled-symbolic",
            _ => "network-wireless-no-route-symbolic",
        }
    }
}

pub fn wifi_icon(strength: u8) -> &'static str {
    match strength {
        0..=24 => "network-wireless-signal-weak-symbolic",
        25..=49 => "network-wireless-signal-ok-symbolic",
        50..=74 => "network-wireless-signal-good-symbolic",
        _ => "network-wireless-signal-excellent-symbolic",
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    SetWifi(bool),
    Scan,
    /// Connect to an access point; `password` for a new secured network.
    ConnectWifi {
        ssid: String,
        ap: String,
        password: Option<String>,
    },
    DisconnectWifi,
    /// Connect or disconnect a VPN (saved connection path).
    SetVpn(String, bool),
    /// Airplane mode: Wi-Fi and mobile broadband off (Bluetooth is switched by the app).
    Airplane(bool),
}

#[derive(Debug, Clone)]
pub enum Update {
    Ready(mpsc::UnboundedSender<Command>),
    State(Box<Network>),
}

fn setting_str(settings: &Settings, group: &str, key: &str) -> Option<String> {
    let value = settings.get(group)?.get(key)?.try_clone().ok()?;
    String::try_from(value).ok()
}

fn setting_bytes(settings: &Settings, group: &str, key: &str) -> Option<Vec<u8>> {
    let value = settings.get(group)?.get(key)?.try_clone().ok()?;
    Vec::<u8>::try_from(value).ok()
}

/// Saved connections: Wi-Fi SSIDs (with their path) and VPNs.
struct Saved {
    wifi: HashMap<String, String>,
    vpns: Vec<(String, String)>,
}

async fn saved(connection: &Connection) -> zbus::Result<Saved> {
    let mut saved = Saved {
        wifi: HashMap::new(),
        vpns: Vec::new(),
    };
    for path in SettingsManagerProxy::new(connection)
        .await?
        .list_connections()
        .await?
    {
        let Ok(proxy) = SavedConnectionProxy::builder(connection)
            .path(path.clone())?
            .build()
            .await
        else {
            continue;
        };
        let Ok(settings) = proxy.get_settings().await else {
            continue;
        };
        let kind = setting_str(&settings, "connection", "type").unwrap_or_default();
        let name = setting_str(&settings, "connection", "id").unwrap_or_default();
        match kind.as_str() {
            "802-11-wireless" => {
                if let Some(ssid) = setting_bytes(&settings, "802-11-wireless", "ssid") {
                    saved.wifi.insert(
                        String::from_utf8_lossy(&ssid).into_owned(),
                        path.to_string(),
                    );
                }
            }
            "vpn" | "wireguard" => saved.vpns.push((name, path.to_string())),
            _ => {}
        }
    }
    saved.vpns.sort();
    Ok(saved)
}

async fn read_ap(connection: &Connection, path: OwnedObjectPath) -> Option<AccessPointInfo> {
    let ap = AccessPointProxy::builder(connection)
        .path(path.clone())
        .ok()?
        .build()
        .await
        .ok()?;
    let ssid = String::from_utf8_lossy(&ap.ssid().await.ok()?).into_owned();
    if ssid.is_empty() {
        return None;
    }
    let secured = ap.flags().await.unwrap_or(0) & 1 != 0
        || ap.wpa_flags().await.unwrap_or(0) != 0
        || ap.rsn_flags().await.unwrap_or(0) != 0;
    Some(AccessPointInfo {
        ssid,
        strength: ap.strength().await.unwrap_or(0),
        secured,
        known: false,
        path: path.to_string(),
    })
}

/// Keeps the strongest access point per SSID, strongest first, without `exclude`.
pub fn dedupe(mut aps: Vec<AccessPointInfo>, exclude: Option<&str>) -> Vec<AccessPointInfo> {
    aps.sort_by_key(|a| std::cmp::Reverse(a.strength));
    let mut seen = Vec::new();
    aps.retain(|ap| {
        let keep = Some(ap.ssid.as_str()) != exclude && !seen.contains(&ap.ssid);
        seen.push(ap.ssid.clone());
        keep
    });
    aps
}

async fn read(connection: &Connection) -> zbus::Result<(Network, Option<String>)> {
    let nm = NetworkManagerProxy::new(connection).await?;
    let saved = saved(connection).await.unwrap_or(Saved {
        wifi: HashMap::new(),
        vpns: Vec::new(),
    });
    let primary_path = nm
        .primary_connection()
        .await
        .map(|p| p.to_string())
        .unwrap_or_default();
    let mut net = Network {
        available: true,
        wifi_enabled: nm.wireless_enabled().await.unwrap_or(false),
        ..Network::default()
    };
    let mut wifi_device = None;
    for path in nm.get_devices().await? {
        let Ok(device) = DeviceProxy::builder(connection)
            .path(path.clone())?
            .build()
            .await
        else {
            continue;
        };
        let activated = device.state().await.unwrap_or(0) == DEVICE_ACTIVATED;
        let active = device
            .active_connection()
            .await
            .map(|p| p.to_string())
            .unwrap_or_default();
        let in_use = activated && active != "/" && active == primary_path;
        match device.device_type().await.unwrap_or(0) {
            DEVICE_ETHERNET if net.wired.is_none() => {
                let wired = WiredProxy::builder(connection)
                    .path(path.clone())?
                    .build()
                    .await?;
                net.wired = Some(WiredState {
                    connected: activated,
                    speed: if activated {
                        wired.speed().await.ok().filter(|s| *s > 0)
                    } else {
                        None
                    },
                    in_use,
                });
                if in_use {
                    net.primary = Primary::Wired;
                }
            }
            DEVICE_WIFI if wifi_device.is_none() => {
                net.has_wifi = true;
                wifi_device = Some(path.to_string());
                let wireless = WirelessProxy::builder(connection)
                    .path(path.clone())?
                    .build()
                    .await?;
                let mut aps = Vec::new();
                for ap in wireless.get_all_access_points().await.unwrap_or_default() {
                    if let Some(mut info) = read_ap(connection, ap).await {
                        info.known = saved.wifi.contains_key(&info.ssid);
                        aps.push(info);
                    }
                }
                let active_ap = wireless.active_access_point().await.map(|p| p.to_string());
                if activated
                    && let Ok(active_ap) = active_ap
                    && let Some(current) = aps.iter().find(|ap| ap.path == active_ap)
                {
                    net.wifi = Some(ActiveWifi {
                        ssid: current.ssid.clone(),
                        strength: current.strength,
                        secured: current.secured,
                        bitrate: wireless.bitrate().await.unwrap_or(0) / 1000,
                        in_use,
                    });
                    if in_use {
                        net.primary = Primary::Wifi;
                    }
                }
                let current = net.wifi.as_ref().map(|w| w.ssid.clone());
                net.access_points = dedupe(aps, current.as_deref());
            }
            _ => {}
        }
    }
    if net.primary == Primary::None && primary_path != "/" && !primary_path.is_empty() {
        net.primary = Primary::Other;
    }
    // VPNs: saved ones, with their active connection if up.
    let mut active_by_saved = HashMap::new();
    for path in nm.active_connections().await.unwrap_or_default() {
        let Ok(active) = ActiveProxy::builder(connection)
            .path(path.clone())?
            .build()
            .await
        else {
            continue;
        };
        let kind = active.kind().await.unwrap_or_default();
        if active.vpn().await.unwrap_or(false) || kind == "wireguard" {
            let saved_path = active
                .connection()
                .await
                .map(|p| p.to_string())
                .unwrap_or_default();
            let up = active.state().await.unwrap_or(0) == ACTIVE_ACTIVATED;
            active_by_saved.insert(saved_path, (path.to_string(), up));
        }
    }
    net.vpns = saved
        .vpns
        .into_iter()
        .map(|(name, path)| {
            let active = active_by_saved.get(&path);
            Vpn {
                name,
                active: active.map(|(p, _)| p.clone()),
                connected: active.is_some_and(|(_, up)| *up),
                path,
            }
        })
        .collect();
    Ok((net, wifi_device))
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

async fn run(out: &mut futures_channel::mpsc::Sender<Update>) -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender(NM)?
        .build();
    let mut signals = MessageStream::for_match_rule(rule, &connection, Some(256)).await?;
    let (tx, mut commands) = mpsc::unbounded_channel();
    _ = out.send(Update::Ready(tx)).await;
    loop {
        let (state, wifi_device) = read(&connection).await?;
        _ = out.send(Update::State(Box::new(state.clone()))).await;
        tokio::select! {
            signal = signals.next() => {
                if signal.is_none() {
                    return Ok(());
                }
                tokio::time::sleep(THROTTLE).await;
                while signals.next().now_or_never().flatten().is_some() {}
            }
            Some(command) = commands.recv() => {
                if let Err(err) = execute(&connection, &state, wifi_device.as_deref(), command.clone()).await {
                    tracing::warn!(%err, "network command failed");
                }
            }
        }
    }
}

async fn execute(
    connection: &Connection,
    state: &Network,
    wifi_device: Option<&str>,
    command: Command,
) -> zbus::Result<()> {
    let nm = NetworkManagerProxy::new(connection).await?;
    let root = ObjectPath::try_from("/")?;
    match command {
        Command::SetWifi(on) => nm.set_wireless_enabled(on).await,
        Command::Airplane(on) => {
            nm.set_wireless_enabled(!on).await?;
            // Not every system has mobile broadband; ignore if it can't be switched.
            _ = nm.set_wwan_enabled(!on).await;
            Ok(())
        }
        Command::Scan => {
            let Some(device) = wifi_device else {
                return Ok(());
            };
            WirelessProxy::builder(connection)
                .path(device.to_owned())?
                .build()
                .await?
                .request_scan(HashMap::new())
                .await
        }
        Command::ConnectWifi { ssid, ap, password } => {
            let Some(device) = wifi_device else {
                return Ok(());
            };
            let device = ObjectPath::try_from(device)?;
            let ap = ObjectPath::try_from(ap.as_str())?;
            let known = saved(connection).await?.wifi.get(&ssid).cloned();
            if let Some(saved) = known {
                nm.activate_connection(&ObjectPath::try_from(saved.as_str())?, &device, &ap)
                    .await?;
            } else {
                let mut settings: HashMap<&str, HashMap<&str, Value<'_>>> = HashMap::new();
                if let Some(password) = password {
                    settings.insert(
                        "802-11-wireless-security",
                        HashMap::from([
                            ("key-mgmt", Value::from("wpa-psk")),
                            ("psk", Value::from(password)),
                        ]),
                    );
                }
                nm.add_and_activate_connection(settings, &device, &ap)
                    .await?;
            }
            Ok(())
        }
        Command::DisconnectWifi => {
            let Some(device) = wifi_device else {
                return Ok(());
            };
            let device = DeviceProxy::builder(connection)
                .path(device.to_owned())?
                .build()
                .await?;
            let active = device.active_connection().await?;
            if active.as_str() != "/" {
                nm.deactivate_connection(&active).await?;
            }
            Ok(())
        }
        Command::SetVpn(path, on) => {
            if on {
                nm.activate_connection(&ObjectPath::try_from(path.as_str())?, &root, &root)
                    .await?;
            } else if let Some(active) = state
                .vpns
                .iter()
                .find(|v| v.path == path)
                .and_then(|v| v.active.clone())
            {
                nm.deactivate_connection(&ObjectPath::try_from(active.as_str())?)
                    .await?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ap(ssid: &str, strength: u8) -> AccessPointInfo {
        AccessPointInfo {
            ssid: ssid.into(),
            strength,
            secured: true,
            known: false,
            path: format!("/ap/{ssid}{strength}"),
        }
    }

    #[test]
    fn one_entry_per_ssid_strongest_first_without_current() {
        let list = dedupe(
            vec![
                ap("Home", 40),
                ap("Guest", 90),
                ap("Home", 70),
                ap("Office", 20),
            ],
            Some("Guest"),
        );
        let names: Vec<_> = list.iter().map(|a| (a.ssid.as_str(), a.strength)).collect();
        assert_eq!(names, [("Home", 70), ("Office", 20)]);
    }

    #[test]
    fn icon_follows_the_connection_in_use() {
        let mut net = Network {
            available: true,
            has_wifi: true,
            wifi_enabled: true,
            primary: Primary::Wired,
            ..Network::default()
        };
        assert_eq!(net.icon(), "network-wired-symbolic");
        net.primary = Primary::Wifi;
        net.wifi = Some(ActiveWifi {
            ssid: "Home".into(),
            strength: 80,
            secured: true,
            bitrate: 866,
            in_use: true,
        });
        assert_eq!(net.icon(), "network-wireless-signal-excellent-symbolic");
        net.primary = Primary::None;
        net.wifi = None;
        net.wifi_enabled = false;
        assert_eq!(net.icon(), "network-wireless-disabled-symbolic");
    }
}
