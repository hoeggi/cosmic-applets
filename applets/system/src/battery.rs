// Copyright 2023 System76 <info@system76.com>
// Copyright 2026 Daniel Hoeggerl
// SPDX-License-Identifier: GPL-3.0-only

//! Battery (UPower), power profiles (power-profiles-daemon interface, also served by
//! system76-power) and display brightness (cosmic-settings-daemon). The charge limit uses
//! system76-power's thresholds like cosmic-applet-battery, and UPower's only without it.

use cosmic::iced::{Subscription, stream};
use futures_util::{SinkExt, StreamExt, stream::select_all};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;
use zbus::{Connection, proxy, zvariant::OwnedObjectPath};

/// Re-read the battery at least this often; UPower only signals some changes.
const POLL: Duration = Duration::from_secs(30);
/// Below this the battery counts as low (header turns red, alert).
pub const LOW_PERCENT: f64 = 10.0;

#[proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower"
)]
trait UPower {
    fn enumerate_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
    fn get_display_device(&self) -> zbus::Result<OwnedObjectPath>;
    #[zbus(property)]
    fn on_battery(&self) -> zbus::Result<bool>;
}

#[proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower"
)]
trait Device {
    fn enable_charge_threshold(&self, enabled: bool) -> zbus::Result<()>;
    #[zbus(property)]
    fn is_present(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(property, name = "Type")]
    fn kind(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn power_supply(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn time_to_empty(&self) -> zbus::Result<i64>;
    #[zbus(property)]
    fn time_to_full(&self) -> zbus::Result<i64>;
    #[zbus(property)]
    fn energy_rate(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn model(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn charge_threshold_supported(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn charge_threshold_enabled(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn charge_end_threshold(&self) -> zbus::Result<u32>;
}

#[proxy(
    interface = "org.freedesktop.UPower.PowerProfiles",
    default_service = "org.freedesktop.UPower.PowerProfiles",
    default_path = "/org/freedesktop/UPower/PowerProfiles"
)]
trait PowerProfiles {
    #[zbus(property)]
    fn active_profile(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn set_active_profile(&self, profile: &str) -> zbus::Result<()>;
    #[zbus(property)]
    fn profiles(
        &self,
    ) -> zbus::Result<Vec<std::collections::HashMap<String, zbus::zvariant::OwnedValue>>>;
}

#[proxy(
    interface = "com.system76.PowerDaemon",
    default_service = "com.system76.PowerDaemon",
    default_path = "/com/system76/PowerDaemon"
)]
trait PowerDaemon {
    fn get_charge_thresholds(&self) -> zbus::Result<(u8, u8)>;
    fn set_charge_thresholds(&self, thresholds: &(u8, u8)) -> zbus::Result<()>;
}

const POWER_DAEMON: &str = "com.system76.PowerDaemon";
/// Thresholds (start, end) for "Limit charge to 80 %" and for charging fully, as upstream sets them.
const LIMITED: (u8, u8) = (70, 80);
const FULL: (u8, u8) = (90, 100);

/// system76-power's thresholds count as a limit when charging stops before 100 %.
fn s76_limited((start, end): (u8, u8)) -> bool {
    end > start && end < 100
}

/// `Some` while system76-power owns its name on the bus.
async fn power_daemon(connection: &Connection) -> Option<PowerDaemonProxy<'_>> {
    let dbus = zbus::fdo::DBusProxy::new(connection).await.ok()?;
    let name = zbus::names::BusName::try_from(POWER_DAEMON).ok()?;
    if !dbus.name_has_owner(name).await.unwrap_or(false) {
        return None;
    }
    PowerDaemonProxy::new(connection).await.ok()
}

#[proxy(
    interface = "com.system76.CosmicSettingsDaemon",
    default_service = "com.system76.CosmicSettingsDaemon",
    default_path = "/com/system76/CosmicSettingsDaemon"
)]
trait SettingsDaemon {
    #[zbus(property)]
    fn display_brightness(&self) -> zbus::Result<i32>;
    #[zbus(property)]
    fn set_display_brightness(&self, value: i32) -> zbus::Result<()>;
    #[zbus(property)]
    fn max_display_brightness(&self) -> zbus::Result<i32>;
}

/// UPower device types (subset).
const TYPE_LINE_POWER: u32 = 1;
const TYPE_BATTERY: u32 = 2;
/// UPower battery states (subset).
const STATE_CHARGING: u32 = 1;
const STATE_FULLY_CHARGED: u32 = 4;
const STATE_PENDING_CHARGE: u32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Charge {
    Charging,
    Discharging,
    Full,
    /// Plugged in but not charging (e.g. held at a charge limit).
    NotCharging,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Peripheral {
    pub name: String,
    pub kind: u32,
    pub percentage: f64,
}

impl Peripheral {
    pub fn icon(&self) -> &'static str {
        match self.kind {
            5 => "input-mouse-symbolic",
            6 => "input-keyboard-symbolic",
            8 => "phone-symbolic",
            12 => "input-gaming-symbolic",
            14 => "input-touchpad-symbolic",
            17 => "audio-headset-symbolic",
            18 => "audio-speakers-symbolic",
            19 => "audio-headphones-symbolic",
            _ => "bluetooth-active-symbolic",
        }
    }
}

/// The battery as the popup shows it. `None` fields mean the system doesn't report them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Battery {
    /// A laptop battery is present; without one the battery section is hidden.
    pub present: bool,
    pub percentage: f64,
    pub charge: Option<Charge>,
    /// Seconds until empty (discharging) or full (charging).
    pub seconds_left: Option<i64>,
    pub watts: Option<f64>,
    /// `Some` when the hardware supports a charge limit: (enabled, end threshold).
    pub charge_limit: Option<(bool, u32)>,
    pub peripherals: Vec<Peripheral>,
    pub profile: Option<String>,
    pub profiles: Vec<String>,
}

impl Battery {
    pub fn is_low(&self) -> bool {
        self.present && self.charge == Some(Charge::Discharging) && self.percentage < LOW_PERCENT
    }

    /// Icon name for the charge, in 10 % steps, as in the Adwaita/Pop themes.
    pub fn icon(&self) -> String {
        battery_icon(self.percentage, self.charge)
    }
}

pub fn battery_icon(percentage: f64, charge: Option<Charge>) -> String {
    let level = ((percentage / 10.0).round() as u32 * 10).min(100);
    match charge {
        Some(Charge::Full) => "battery-level-100-charged-symbolic".into(),
        Some(Charge::Charging) => format!("battery-level-{level}-charging-symbolic"),
        _ => format!("battery-level-{level}-symbolic"),
    }
}

/// "3 h 40 min", "25 min".
pub fn duration_text(seconds: i64) -> (i64, i64) {
    let minutes = (seconds.max(0) + 30) / 60;
    (minutes / 60, minutes % 60)
}

fn charge_from(state: u32, on_battery: bool) -> Charge {
    match state {
        STATE_CHARGING => Charge::Charging,
        STATE_FULLY_CHARGED => Charge::Full,
        STATE_PENDING_CHARGE => Charge::NotCharging,
        _ if on_battery => Charge::Discharging,
        _ => Charge::NotCharging,
    }
}

async fn read(connection: &Connection) -> zbus::Result<Battery> {
    let upower = UPowerProxy::new(connection).await?;
    let on_battery = upower.on_battery().await.unwrap_or(false);
    let mut battery = Battery::default();
    for path in upower.enumerate_devices().await? {
        let Ok(device) = DeviceProxy::builder(connection).path(path)?.build().await else {
            continue;
        };
        let kind = device.kind().await.unwrap_or(0);
        if kind == TYPE_LINE_POWER {
            continue;
        }
        let power_supply = device.power_supply().await.unwrap_or(false);
        if kind == TYPE_BATTERY && power_supply {
            if battery.present || !device.is_present().await.unwrap_or(false) {
                continue;
            }
            battery.present = true;
            battery.percentage = device.percentage().await.unwrap_or(0.0);
            let charge = charge_from(device.state().await.unwrap_or(0), on_battery);
            battery.charge = Some(charge);
            battery.seconds_left = match charge {
                Charge::Discharging => device.time_to_empty().await.ok(),
                Charge::Charging => device.time_to_full().await.ok(),
                _ => None,
            }
            .filter(|s| *s > 0);
            battery.watts = device.energy_rate().await.ok().filter(|w| *w > 0.0);
            if device.charge_threshold_supported().await.unwrap_or(false) {
                battery.charge_limit = Some((
                    device.charge_threshold_enabled().await.unwrap_or(false),
                    device.charge_end_threshold().await.unwrap_or(80),
                ));
            }
        } else if !power_supply {
            let name = device.model().await.unwrap_or_default();
            if let Ok(percentage) = device.percentage().await
                && !name.is_empty()
            {
                battery.peripherals.push(Peripheral {
                    name,
                    kind,
                    percentage,
                });
            }
        }
    }
    if battery.present
        && let Some(daemon) = power_daemon(connection).await
    {
        // system76-power wins over UPower; unsupported hardware fails the call.
        battery.charge_limit = daemon
            .get_charge_thresholds()
            .await
            .ok()
            .map(|t| (s76_limited(t), u32::from(LIMITED.1)));
    }
    if let Ok(profiles) = PowerProfilesProxy::new(connection).await {
        battery.profile = profiles.active_profile().await.ok();
        battery.profiles = profiles
            .profiles()
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| {
                p.get("Profile")
                    .and_then(|v| String::try_from(v.clone()).ok())
            })
            .collect();
    }
    Ok(battery)
}

/// Emits the battery on start, on UPower changes and every [`POLL`].
pub fn subscription() -> Subscription<Battery> {
    Subscription::run(|| {
        stream::channel(
            4,
            async |mut out: futures_channel::mpsc::Sender<Battery>| {
                loop {
                    if let Err(err) = watch(&mut out).await {
                        tracing::warn!(%err, "UPower unavailable");
                    }
                    tokio::time::sleep(POLL).await;
                }
            },
        )
    })
}

async fn watch(out: &mut futures_channel::mpsc::Sender<Battery>) -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let upower = UPowerProxy::new(&connection).await?;
    let display = DeviceProxy::builder(&connection)
        .path(upower.get_display_device().await?)?
        .build()
        .await?;
    let mut changes = select_all([
        display
            .receive_percentage_changed()
            .await
            .map(|_| ())
            .boxed(),
        display.receive_state_changed().await.map(|_| ()).boxed(),
        upower
            .receive_on_battery_changed()
            .await
            .map(|_| ())
            .boxed(),
    ]);
    if let Ok(profiles) = PowerProfilesProxy::new(&connection).await {
        changes.push(
            profiles
                .receive_active_profile_changed()
                .await
                .map(|_| ())
                .boxed(),
        );
    }
    loop {
        _ = out.send(read(&connection).await?).await;
        tokio::select! {
            _ = changes.next() => {}
            () = tokio::time::sleep(POLL) => {}
        }
    }
}

pub async fn set_profile(profile: String) {
    let result = async {
        let connection = Connection::system().await?;
        PowerProfilesProxy::new(&connection)
            .await?
            .set_active_profile(&profile)
            .await
    }
    .await;
    if let Err(err) = result {
        tracing::warn!(%err, profile, "cannot set the power profile");
    }
}

pub async fn set_charge_limit(enabled: bool) {
    let result = async {
        let connection = Connection::system().await?;
        if let Some(daemon) = power_daemon(&connection).await {
            let thresholds = if enabled { LIMITED } else { FULL };
            return daemon.set_charge_thresholds(&thresholds).await;
        }
        let upower = UPowerProxy::new(&connection).await?;
        for path in upower.enumerate_devices().await? {
            let device = DeviceProxy::builder(&connection)
                .path(path)?
                .build()
                .await?;
            if device.kind().await.unwrap_or(0) == TYPE_BATTERY
                && device.charge_threshold_supported().await.unwrap_or(false)
            {
                device.enable_charge_threshold(enabled).await?;
            }
        }
        Ok::<_, zbus::Error>(())
    }
    .await;
    if let Err(err) = result {
        tracing::warn!(%err, "cannot change the charge limit");
    }
}

/// Display brightness in percent, or `None` without a backlight.
pub fn brightness_subscription() -> Subscription<Option<u32>> {
    Subscription::run(|| {
        stream::channel(
            4,
            async |mut out: futures_channel::mpsc::Sender<Option<u32>>| {
                loop {
                    if let Err(err) = watch_brightness(&mut out).await {
                        tracing::debug!(%err, "brightness unavailable");
                    }
                    _ = out.send(None).await;
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            },
        )
    })
}

async fn watch_brightness(
    out: &mut futures_channel::mpsc::Sender<Option<u32>>,
) -> zbus::Result<()> {
    let connection = Connection::session().await?;
    let daemon = SettingsDaemonProxy::new(&connection).await?;
    let mut changes = select_all([
        daemon
            .receive_display_brightness_changed()
            .await
            .map(|_| ())
            .boxed(),
        daemon
            .receive_max_display_brightness_changed()
            .await
            .map(|_| ())
            .boxed(),
    ]);
    loop {
        let max = daemon.max_display_brightness().await?;
        let value = daemon.display_brightness().await?;
        _ = out.send(to_percent(value, max)).await;
        if changes.next().await.is_none() {
            return Ok(());
        }
    }
}

fn to_percent(value: i32, max: i32) -> Option<u32> {
    (max > 0 && value >= 0).then(|| ((value as f64 * 100.0 / max as f64).round() as u32).min(100))
}

pub async fn set_brightness(percent: u32) {
    let result = async {
        let connection = Connection::session().await?;
        let daemon = SettingsDaemonProxy::new(&connection).await?;
        let max = daemon.max_display_brightness().await?;
        // Never all the way to black: 1 % keeps the screen readable.
        let value = (max as f64 * f64::from(percent.max(1)) / 100.0).round() as i32;
        daemon.set_display_brightness(value).await
    }
    .await;
    if let Err(err) = result {
        tracing::warn!(%err, "cannot set the brightness");
    }
}

/// The brightness slider's latest value, sent at most every 100 ms.
#[derive(Debug, Default, Clone)]
pub struct Debounced {
    value: Arc<AtomicU32>,
    pending: Arc<AtomicBool>,
}

impl Debounced {
    pub fn is_pending(&self) -> bool {
        self.pending.load(Ordering::Acquire)
    }

    /// Stores the value; returns a future to spawn if no send is already scheduled.
    pub fn set(&self, percent: u32) -> Option<impl Future<Output = ()> + Send + 'static> {
        self.value.store(percent, Ordering::Relaxed);
        if self.pending.swap(true, Ordering::AcqRel) {
            return None;
        }
        let latest = self.value.clone();
        let pending = self.pending.clone();
        Some(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            pending.store(false, Ordering::Release);
            set_brightness(latest.load(Ordering::Relaxed)).await;
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_in_ten_percent_steps() {
        assert_eq!(
            battery_icon(78.0, Some(Charge::Discharging)),
            "battery-level-80-symbolic"
        );
        assert_eq!(
            battery_icon(74.0, Some(Charge::Charging)),
            "battery-level-70-charging-symbolic"
        );
        assert_eq!(
            battery_icon(100.0, Some(Charge::Full)),
            "battery-level-100-charged-symbolic"
        );
        assert_eq!(battery_icon(3.0, None), "battery-level-0-symbolic");
    }

    #[test]
    fn low_only_when_discharging() {
        let mut battery = Battery {
            present: true,
            percentage: 9.0,
            charge: Some(Charge::Discharging),
            ..Battery::default()
        };
        assert!(battery.is_low());
        battery.charge = Some(Charge::Charging);
        assert!(!battery.is_low());
    }

    #[test]
    fn durations_and_brightness() {
        assert_eq!(duration_text(13_200), (3, 40));
        assert_eq!(duration_text(1_490), (0, 25));
        assert_eq!(to_percent(24242, 24242), Some(100));
        assert_eq!(to_percent(12121, 24242), Some(50));
        assert_eq!(to_percent(5, 0), None);
    }

    #[test]
    fn s76_thresholds() {
        assert!(s76_limited(LIMITED));
        assert!(!s76_limited(FULL));
        assert!(!s76_limited((0, 100)), "the default (0, 100) is no limit");
        assert!(s76_limited((40, 60)));
    }

    #[test]
    fn charge_states() {
        assert_eq!(charge_from(2, true), Charge::Discharging);
        assert_eq!(charge_from(0, false), Charge::NotCharging);
        assert_eq!(charge_from(4, false), Charge::Full);
    }
}
