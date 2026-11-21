// SPDX-License-Identifier: GPL-3.0-only

//! Bluetooth state for our UI, from upstream's BlueZ backend (`crate::upstream::bluetooth`,
//! through `bluer`). Requests, the optimistic status changes and the PIN confirmation work as in
//! upstream's applet.

use crate::upstream::bluetooth::{
    self as up, BluerDevice, BluerDeviceStatus, BluerEvent, BluerRequest, BluerState,
};
use bluer::Address;
use cosmic::iced::Subscription;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

/// Upstream's popup polls every 3 s while it is open and every 10 s otherwise; we do the same for
/// the Bluetooth page.
const TICK_SCANNING: Duration = Duration::from_secs(3);
const TICK_IDLE: Duration = Duration::from_secs(10);

/// What a device row shows and offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceState {
    Connected,
    /// Paired, not connected.
    Paired,
    Connecting,
    Disconnecting,
    Pairing,
    /// Found by a scan, not paired.
    New,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BtDevice {
    pub address: Address,
    pub name: String,
    pub icon: &'static str,
    pub state: DeviceState,
    pub battery: Option<u8>,
}

impl BtDevice {
    pub fn connected(&self) -> bool {
        self.state == DeviceState::Connected
    }
}

/// A device asks to confirm a pairing PIN (upstream's `RequestConfirmation`).
#[derive(Debug, Clone, PartialEq)]
pub struct Confirmation {
    pub device: String,
    pub icon: &'static str,
    pub pin: String,
}

/// The adapter and devices; `available` is false until BlueZ answered.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bluetooth {
    pub available: bool,
    pub powered: bool,
    pub discovering: bool,
    /// Paired devices (connected, paired, or switching between the two), upstream's order.
    pub known: Vec<BtDevice>,
    /// Unpaired devices with a name or a known type, as upstream's "Other Bluetooth devices".
    pub nearby: Vec<BtDevice>,
    pub confirmation: Option<Confirmation>,
}

impl Bluetooth {
    pub fn connected(&self) -> impl Iterator<Item = &BtDevice> {
        self.known.iter().filter(|d| d.connected())
    }
}

fn device(d: &BluerDevice) -> BtDevice {
    BtDevice {
        address: d.address,
        name: d.name.clone(),
        icon: d.icon,
        state: match d.status {
            BluerDeviceStatus::Connected => DeviceState::Connected,
            BluerDeviceStatus::Paired => DeviceState::Paired,
            BluerDeviceStatus::Connecting => DeviceState::Connecting,
            BluerDeviceStatus::Disconnecting => DeviceState::Disconnecting,
            BluerDeviceStatus::Pairing => DeviceState::Pairing,
            BluerDeviceStatus::Disconnected => DeviceState::New,
        },
        battery: d.battery_percent,
    }
}

/// Our view of upstream's state, split into known and nearby devices the way upstream's popup
/// does. The device asking for a PIN confirmation is shown in the confirmation instead.
pub fn build(
    state: &BluerState,
    available: bool,
    discovering: bool,
    confirm: Option<&BluerDevice>,
    pin: Option<&str>,
) -> Bluetooth {
    let asking = |d: &&BluerDevice| confirm.is_none_or(|c| c.address != d.address);
    let known = state
        .devices
        .iter()
        .filter(asking)
        .filter(|d| {
            matches!(
                d.status,
                BluerDeviceStatus::Connected
                    | BluerDeviceStatus::Paired
                    | BluerDeviceStatus::Connecting
                    | BluerDeviceStatus::Disconnecting
            )
        })
        .map(device)
        .collect();
    let nearby = state
        .devices
        .iter()
        .filter(asking)
        .filter(|d| {
            matches!(
                d.status,
                BluerDeviceStatus::Disconnected | BluerDeviceStatus::Pairing
            ) && (d.has_name() || d.is_known_device_type())
        })
        .map(device)
        .collect();
    Bluetooth {
        available,
        powered: state.bluetooth_enabled,
        discovering: discovering && state.bluetooth_enabled,
        known,
        nearby,
        confirmation: confirm.zip(pin).map(|(d, pin)| Confirmation {
            device: d.name.clone(),
            icon: d.icon,
            pin: pin.to_owned(),
        }),
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    SetPowered(bool),
    /// Scans while the Bluetooth page is open.
    Scan(bool),
    Connect(Address),
    Disconnect(Address),
    /// Pairs; upstream's backend trusts and connects after pairing.
    Pair(Address),
    /// Answers a PIN confirmation.
    Confirm(bool),
}

pub fn subscription() -> Subscription<BluerEvent> {
    up::bluetooth_subscription("system-applet-bluetooth")
}

/// Upstream's state plus what upstream's applet keeps beside it.
#[derive(Debug, Default)]
pub struct Model {
    state: BluerState,
    sender: Option<Sender<BluerRequest>>,
    available: bool,
    discovering: bool,
    confirm: Option<(BluerDevice, String, Sender<bool>)>,
}

impl Model {
    pub fn view(&self) -> Bluetooth {
        let confirm = self.confirm.as_ref();
        build(
            &self.state,
            self.available,
            self.discovering,
            confirm.map(|(d, _, _)| d),
            confirm.map(|(_, pin, _)| pin.as_str()),
        )
    }

    /// Upstream's `Message::BluetoothEvent` handling.
    pub fn event(&mut self, event: BluerEvent) {
        match event {
            BluerEvent::RequestResponse {
                req,
                state,
                err_msg,
            } => {
                if let Some(err) = err_msg {
                    tracing::warn!(?req, err, "bluetooth request failed");
                }
                self.state = state;
            }
            BluerEvent::Init { sender, state } => {
                self.sender = Some(sender);
                self.state = state;
                self.available = true;
            }
            BluerEvent::DevicesChanged { state } => self.state = state,
            // Upstream's applet exits here and is restarted by the panel; we just show
            // Bluetooth as unavailable.
            BluerEvent::Finished => {
                tracing::warn!("bluetooth subscription finished");
                *self = Model::default();
            }
            BluerEvent::AgentEvent(up::BluerAgentEvent::RequestConfirmation(d, code, tx)) => {
                self.confirm = Some((d, code, tx));
            }
            // Upstream's applet ignores the other agent events too.
            BluerEvent::AgentEvent(_) => {}
        }
    }

    /// Upstream's `Message::Request` handling: mark the device first, then send the request.
    pub fn command(&mut self, command: Command) {
        let request = match command {
            Command::Scan(on) => {
                self.discovering = on;
                up::set_discovery(on);
                tokio::spawn(up::set_tick(if on { TICK_SCANNING } else { TICK_IDLE }));
                return;
            }
            Command::Confirm(ok) => {
                if let Some((_, _, tx)) = self.confirm.take() {
                    tokio::spawn(async move {
                        _ = tx.send(ok).await;
                    });
                }
                return;
            }
            Command::SetPowered(on) => {
                self.state.bluetooth_enabled = on;
                BluerRequest::SetBluetoothEnabled(on)
            }
            Command::Connect(address) => {
                self.mark(address, BluerDeviceStatus::Connecting);
                BluerRequest::ConnectDevice(address)
            }
            Command::Disconnect(address) => {
                self.mark(address, BluerDeviceStatus::Disconnecting);
                BluerRequest::DisconnectDevice(address)
            }
            Command::Pair(address) => {
                self.mark(address, BluerDeviceStatus::Pairing);
                BluerRequest::PairDevice(address)
            }
        };
        if let Some(tx) = self.sender.clone() {
            tokio::spawn(async move {
                _ = tx.send(request).await;
            });
        }
    }

    fn mark(&mut self, address: Address, status: BluerDeviceStatus) {
        if let Some(d) = self.state.devices.iter_mut().find(|d| d.address == address) {
            d.status = status;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev(name: &str, last: u8, status: BluerDeviceStatus, icon: &'static str) -> BluerDevice {
        let is_paired = !matches!(
            status,
            BluerDeviceStatus::Disconnected | BluerDeviceStatus::Pairing
        );
        BluerDevice {
            name: name.into(),
            icon,
            address: Address::new([0, 0, 0, 0, 0, last]),
            status,
            battery_percent: None,
            is_paired,
            is_trusted: true,
        }
    }

    fn state() -> BluerState {
        BluerState {
            bluetooth_enabled: true,
            devices: vec![
                dev(
                    "WH-1000XM5",
                    1,
                    BluerDeviceStatus::Connected,
                    "audio-headphones-symbolic",
                ),
                dev(
                    "MX Keys",
                    2,
                    BluerDeviceStatus::Paired,
                    "input-keyboard-symbolic",
                ),
                dev(
                    "Pixel 9",
                    3,
                    BluerDeviceStatus::Disconnected,
                    "smartphone-symbolic",
                ),
                // No name (upstream uses the address) and no known type: hidden, like upstream.
                dev(
                    "00:00:00:00:00:04",
                    4,
                    BluerDeviceStatus::Disconnected,
                    "bluetooth-symbolic",
                ),
            ],
        }
    }

    #[test]
    fn known_and_nearby_like_upstream() {
        let bt = build(&state(), true, true, None, None);
        let names = |l: &[BtDevice]| l.iter().map(|d| d.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&bt.known), ["WH-1000XM5", "MX Keys"]);
        assert_eq!(names(&bt.nearby), ["Pixel 9"]);
        assert_eq!(bt.connected().count(), 1);
        assert!(bt.discovering);
    }

    #[test]
    fn device_asking_for_a_pin_moves_to_the_confirmation() {
        let s = state();
        let asking = s.devices[2].clone();
        let bt = build(&s, true, false, Some(&asking), Some("123456"));
        assert!(bt.nearby.is_empty());
        assert_eq!(
            bt.confirmation,
            Some(Confirmation {
                device: "Pixel 9".into(),
                icon: "smartphone-symbolic",
                pin: "123456".into(),
            })
        );
    }

    #[test]
    fn powered_off_never_discovers() {
        let mut s = state();
        s.bluetooth_enabled = false;
        let bt = build(&s, true, true, None, None);
        assert!(!bt.powered && !bt.discovering);
    }
}
