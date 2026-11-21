// SPDX-License-Identifier: GPL-3.0-only

//! Battery, power profiles, charge limit and display brightness for our UI, from upstream's
//! battery applet: its backend (`crate::upstream::battery`: power profiles through
//! system76-power or power-profiles-daemon, the system76-power charge limit), its icon and
//! brightness logic (`crate::upstream::battery::app`), and the same cosmic-settings
//! subscriptions it uses (UPower display device, cosmic-settings-daemon brightness).
//!
//! Still ours, because upstream's applet has no such parts: the batteries of connected devices
//! (mice, headsets, … from UPower) and the charging text on the Battery page.

use crate::upstream::battery::{
    self as up, Power, PowerProfileRequest, PowerProfileUpdate, app as up_app,
};
use cosmic::iced::{Subscription, futures::SinkExt, stream};
use cosmic_settings_daemon_subscription as settings_daemon;
use cosmic_settings_upower_subscription::device::{DeviceDbusEvent, device_subscription};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use zbus::{Connection, OwnedObjectPath, proxy};

/// Below this the battery counts as low (header turns red, alert).
pub const LOW_PERCENT: f64 = 10.0;
/// Upstream's charge limit stops charging at 80 % (`set_charging_limit`: thresholds 70, 80).
pub const LIMIT_PERCENT: u32 = 80;
/// Connected devices' batteries are re-read this often; UPower doesn't signal them all.
const PERIPHERAL_POLL: Duration = Duration::from_secs(60);

/// The power profiles upstream offers, in its order.
pub const PROFILES: [Power; 3] = [Power::Battery, Power::Balanced, Power::Performance];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Charge {
    Charging,
    Discharging,
    Full,
    /// Plugged in and held at the charge limit.
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

/// The battery as the popup shows it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Battery {
    /// A laptop battery is present; without one the battery section is hidden.
    pub present: bool,
    pub percentage: f64,
    pub charge: Option<Charge>,
    /// Seconds until empty while on battery (upstream reads only time to empty).
    pub seconds_left: Option<i64>,
    /// `Some(enabled)` when system76-power supports a charge limit.
    pub charge_limit: Option<bool>,
    pub peripherals: Vec<Peripheral>,
    /// `None` until a power-profile backend answered.
    pub profile: Option<Power>,
    /// Upstream's battery icon (`cosmic-applet-battery-level-…`, installed with cosmic-applets).
    pub icon: String,
}

impl Battery {
    pub fn is_low(&self) -> bool {
        self.present && self.charge == Some(Charge::Discharging) && self.percentage < LOW_PERCENT
    }
}

/// Our charging state from what upstream reads (on battery and percent), plus the limit.
pub fn charge(percent: f64, on_battery: bool, charge_limit: Option<bool>) -> Charge {
    if on_battery {
        Charge::Discharging
    } else if charge_limit == Some(true) && percent >= f64::from(LIMIT_PERCENT) - 1.0 {
        Charge::NotCharging
    } else if percent >= 99.0 {
        Charge::Full
    } else {
        Charge::Charging
    }
}

/// "3 h 40 min", "25 min".
pub fn duration_text(seconds: i64) -> (i64, i64) {
    let minutes = (seconds.max(0) + 30) / 60;
    (minutes / 60, minutes % 60)
}

/// Upstream's brightness icon for a slider value (its function returns one of four names).
pub fn display_icon(percent: Option<f64>) -> &'static str {
    match up_app::display_icon_name(percent).as_str() {
        "cosmic-applet-battery-display-brightness-high-symbolic" => {
            "cosmic-applet-battery-display-brightness-high-symbolic"
        }
        "cosmic-applet-battery-display-brightness-medium-symbolic" => {
            "cosmic-applet-battery-display-brightness-medium-symbolic"
        }
        "cosmic-applet-battery-display-brightness-low-symbolic" => {
            "cosmic-applet-battery-display-brightness-low-symbolic"
        }
        _ => "cosmic-applet-battery-display-brightness-off-symbolic",
    }
}

/// Raw brightness for a slider percentage, snapped like upstream.
pub fn brightness_raw(percent: u32, max: i32) -> i32 {
    let raw = (f64::from(percent.min(100)) * f64::from(max) / 100.0).round() as i32;
    up_app::snap_screen_brightness(raw, Some(max))
}

#[derive(Debug, Clone)]
pub enum Event {
    Device(DeviceDbusEvent),
    Profile(PowerProfileUpdate),
    ChargeLimit(Option<bool>),
    Peripherals(Vec<Peripheral>),
    SessionBus(Connection),
    SettingsDaemon(settings_daemon::Event),
}

/// Upstream's subscriptions plus our connected-device poll. The settings-daemon subscription
/// needs the session bus, which [`connect_session`] delivers first, as in upstream's applet.
pub fn subscription(session: Option<&Connection>) -> Subscription<Event> {
    let mut subs = vec![
        device_subscription("system-applet-upower").map(Event::Device),
        up::power_profile_subscription("system-applet-profiles").map(Event::Profile),
        peripherals_subscription().map(Event::Peripherals),
    ];
    if let Some(conn) = session {
        subs.push(settings_daemon::subscription(conn.clone()).map(Event::SettingsDaemon));
    }
    Subscription::batch(subs)
}

pub async fn connect_session() -> Option<Event> {
    match Connection::session().await {
        Ok(conn) => Some(Event::SessionBus(conn)),
        Err(err) => {
            tracing::error!(%err, "failed to connect to the session bus");
            None
        }
    }
}

/// Upstream's `get_charging_limit`: `None` without system76-power's charge thresholds.
pub async fn read_charge_limit() -> Event {
    Event::ChargeLimit(up::get_charging_limit().await.ok())
}

/// Upstream's `set_charging_limit` / `unset_charging_limit`.
pub async fn write_charge_limit(enabled: bool) {
    let result = if enabled {
        up::set_charging_limit().await
    } else {
        up::unset_charging_limit().await
    };
    if let Err(err) = result {
        tracing::warn!(%err, "cannot change the charge limit");
    }
}

/// What upstream's battery applet keeps, plus our connected devices.
#[derive(Debug, Default)]
pub struct Model {
    no_battery: bool,
    seen_battery: bool,
    on_battery: bool,
    percent: f64,
    time_to_empty: i64,
    charging_limit: Option<bool>,
    profile: Option<Power>,
    profile_sender: Option<UnboundedSender<PowerProfileRequest>>,
    peripherals: Vec<Peripheral>,
    pub session: Option<Connection>,
    settings_sender: Option<UnboundedSender<settings_daemon::Request>>,
    max_screen_brightness: Option<i32>,
    screen_brightness: Option<i32>,
    dragging_screen_brightness: bool,
}

impl Model {
    /// Upstream's handlers for these events.
    pub fn event(&mut self, event: Event) {
        match event {
            Event::Device(DeviceDbusEvent::Update {
                on_battery,
                percent,
                time_to_empty,
            }) => {
                self.no_battery = false;
                self.seen_battery = true;
                self.on_battery = on_battery;
                self.percent = percent.clamp(0.0, 100.0);
                self.time_to_empty = time_to_empty;
            }
            Event::Device(DeviceDbusEvent::NoBattery) => self.no_battery = true,
            Event::Profile(PowerProfileUpdate::Init(profile, tx)) => {
                self.profile = Some(profile);
                self.profile_sender = Some(tx);
            }
            Event::Profile(PowerProfileUpdate::Update { profile }) => self.profile = Some(profile),
            Event::Profile(PowerProfileUpdate::Error(err)) => {
                tracing::debug!(err, "power profile backend");
            }
            Event::ChargeLimit(limit) => self.charging_limit = limit,
            Event::Peripherals(list) => self.peripherals = list,
            Event::SessionBus(conn) => self.session = Some(conn),
            Event::SettingsDaemon(settings_daemon::Event::Sender(tx)) => {
                self.settings_sender = Some(tx);
            }
            Event::SettingsDaemon(settings_daemon::Event::MaxDisplayBrightness(max)) => {
                self.max_screen_brightness = Some(max);
            }
            Event::SettingsDaemon(settings_daemon::Event::DisplayBrightness(brightness)) => {
                if !self.dragging_screen_brightness {
                    self.screen_brightness = Some(brightness);
                }
            }
        }
    }

    pub fn view(&self) -> Battery {
        let present = self.seen_battery && !self.no_battery;
        Battery {
            present,
            percentage: self.percent,
            charge: present.then(|| charge(self.percent, self.on_battery, self.charging_limit)),
            seconds_left: (self.on_battery && self.time_to_empty > 0).then_some(self.time_to_empty),
            charge_limit: self.charging_limit.filter(|_| present),
            peripherals: self.peripherals.clone(),
            profile: self.profile,
            icon: up_app::battery_icon_name(self.percent, self.on_battery, self.charging_limit),
        }
    }

    /// Upstream's `SelectProfile`.
    pub fn set_profile(&mut self, profile: Power) {
        if let Some(tx) = &self.profile_sender {
            _ = tx.send(PowerProfileRequest::Set(profile));
        }
    }

    /// Upstream's `SetChargingLimit`: shown at once, written in the background.
    pub fn set_charge_limit(&mut self, enabled: bool) {
        self.charging_limit = Some(enabled);
    }

    /// The slider's percentage (upstream's `screen_brightness_percent`), `None` without a
    /// backlight.
    pub fn brightness_percent(&self) -> Option<u32> {
        self.max_screen_brightness?;
        up_app::screen_brightness_percent(self.screen_brightness, self.max_screen_brightness)
            .map(|p| (p * 100.0).round() as u32)
    }

    pub fn brightness_icon(&self) -> &'static str {
        display_icon(up_app::screen_brightness_percent(
            self.screen_brightness,
            self.max_screen_brightness,
        ))
    }

    /// Upstream's `SetScreenBrightness`: snap and show at once. Returns true when a debounce
    /// loop has to start (upstream's `SetScreenBrightnessDebounced`).
    pub fn drag_brightness(&mut self, percent: u32) -> bool {
        let Some(max) = self.max_screen_brightness else {
            return false;
        };
        self.screen_brightness = Some(brightness_raw(percent, max));
        if self.dragging_screen_brightness {
            false
        } else {
            self.dragging_screen_brightness = true;
            true
        }
    }

    /// Upstream's `SetScreenBrightnessDebounced`: sends while dragging; false once released.
    pub fn send_brightness_while_dragging(&self) -> bool {
        if !self.dragging_screen_brightness {
            return false;
        }
        self.send_brightness();
        true
    }

    /// Upstream's `ReleaseScreenBrightness`.
    pub fn release_brightness(&mut self) {
        self.dragging_screen_brightness = false;
        self.send_brightness();
    }

    fn send_brightness(&self) {
        if let (Some(tx), Some(b)) = (&self.settings_sender, self.screen_brightness) {
            _ = tx.send(settings_daemon::Request::SetDisplayBrightness(b));
        }
    }
}

// ---- Connected devices' batteries (ours; upstream's battery applet doesn't list them) ----

#[proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower"
)]
trait UPower {
    fn enumerate_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower"
)]
trait Device {
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<f64>;
    #[zbus(property, name = "Type")]
    fn kind(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn power_supply(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn model(&self) -> zbus::Result<String>;
}

/// UPower's line-power device type, never a peripheral.
const TYPE_LINE_POWER: u32 = 1;

async fn read_peripherals(connection: &Connection) -> zbus::Result<Vec<Peripheral>> {
    let mut list = Vec::new();
    for path in UPowerProxy::new(connection)
        .await?
        .enumerate_devices()
        .await?
    {
        let Ok(device) = DeviceProxy::builder(connection).path(path).build().await else {
            continue;
        };
        let kind = device.kind().await.unwrap_or(0);
        if kind == TYPE_LINE_POWER || device.power_supply().await.unwrap_or(true) {
            continue;
        }
        let name = device.model().await.unwrap_or_default();
        if let Ok(percentage) = device.percentage().await
            && !name.is_empty()
        {
            list.push(Peripheral {
                name,
                kind,
                percentage,
            });
        }
    }
    Ok(list)
}

fn peripherals_subscription() -> Subscription<Vec<Peripheral>> {
    Subscription::run(|| {
        stream::channel(
            2,
            async |mut out: futures_channel::mpsc::Sender<Vec<Peripheral>>| {
                loop {
                    match Connection::system().await {
                        Ok(connection) => match read_peripherals(&connection).await {
                            Ok(list) => _ = out.send(list).await,
                            Err(err) => tracing::debug!(%err, "UPower devices"),
                        },
                        Err(err) => tracing::debug!(%err, "system bus"),
                    }
                    tokio::time::sleep(PERIPHERAL_POLL).await;
                }
            },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charge_states_from_upstream_values() {
        assert_eq!(charge(55.0, true, Some(true)), Charge::Discharging);
        assert_eq!(charge(55.0, false, None), Charge::Charging);
        assert_eq!(charge(100.0, false, Some(false)), Charge::Full);
        assert_eq!(charge(80.0, false, Some(true)), Charge::NotCharging);
        assert_eq!(charge(60.0, false, Some(true)), Charge::Charging);
    }

    #[test]
    fn view_hides_the_battery_until_upower_reports_one() {
        let mut model = Model::default();
        assert!(!model.view().present);
        model.event(Event::Device(DeviceDbusEvent::Update {
            on_battery: true,
            percent: 9.0,
            time_to_empty: 1500,
        }));
        model.event(Event::ChargeLimit(Some(false)));
        let battery = model.view();
        assert!(battery.present && battery.is_low());
        assert_eq!(battery.seconds_left, Some(1500));
        assert_eq!(battery.icon, "cosmic-applet-battery-level-5-symbolic");
        model.event(Event::Device(DeviceDbusEvent::NoBattery));
        let battery = model.view();
        assert!(!battery.present && battery.charge_limit.is_none());
    }

    #[test]
    fn upstream_icons_and_limit() {
        let mut model = Model::default();
        model.event(Event::Device(DeviceDbusEvent::Update {
            on_battery: false,
            percent: 79.0,
            time_to_empty: 0,
        }));
        model.event(Event::ChargeLimit(Some(true)));
        let battery = model.view();
        assert_eq!(battery.charge, Some(Charge::NotCharging));
        assert_eq!(battery.seconds_left, None);
        assert_eq!(
            battery.icon,
            "cosmic-applet-battery-level-80-limited-charging-symbolic"
        );
    }

    #[test]
    fn brightness_like_upstream() {
        let mut model = Model::default();
        assert_eq!(model.brightness_percent(), None, "no backlight");
        model.event(Event::SettingsDaemon(
            settings_daemon::Event::MaxDisplayBrightness(24242),
        ));
        model.event(Event::SettingsDaemon(
            settings_daemon::Event::DisplayBrightness(12121),
        ));
        assert_eq!(model.brightness_percent(), Some(50));
        assert_eq!(
            model.brightness_icon(),
            "cosmic-applet-battery-display-brightness-medium-symbolic"
        );
        // Coarse panels snap to 5 % steps, as upstream does.
        assert_eq!(brightness_raw(52, 20), 10);
        assert!(model.drag_brightness(80));
        assert!(!model.drag_brightness(81), "the debounce loop already runs");
        model.event(Event::SettingsDaemon(
            settings_daemon::Event::DisplayBrightness(1),
        ));
        assert_eq!(
            model.brightness_percent(),
            Some(81),
            "echoes ignored while dragging"
        );
    }
}
