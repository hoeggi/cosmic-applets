// SPDX-License-Identifier: GPL-3.0-only

//! Bluetooth through BlueZ on the system bus. One connection lives in the subscription and also
//! runs the commands: BlueZ ends a discovery when the connection that started it goes away.

use cosmic::iced::{Subscription, stream};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use zbus::fdo::ObjectManagerProxy;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};
use zbus::{Connection, MatchRule, MessageStream, proxy};

#[proxy(interface = "org.bluez.Adapter1", default_service = "org.bluez")]
trait Adapter {
    fn start_discovery(&self) -> zbus::Result<()>;
    fn stop_discovery(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn set_powered(&self, powered: bool) -> zbus::Result<()>;
}

#[proxy(interface = "org.bluez.Device1", default_service = "org.bluez")]
trait Device {
    fn connect(&self) -> zbus::Result<()>;
    fn disconnect(&self) -> zbus::Result<()>;
    fn pair(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn set_trusted(&self, trusted: bool) -> zbus::Result<()>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct BtDevice {
    pub path: String,
    pub name: String,
    pub icon: Option<String>,
    pub paired: bool,
    pub connected: bool,
    pub battery: Option<u8>,
}

impl BtDevice {
    /// Symbolic icon from BlueZ's freedesktop icon name.
    pub fn icon_name(&self) -> &'static str {
        match self.icon.as_deref().unwrap_or_default() {
            "audio-headset" => "audio-headset-symbolic",
            "audio-headphones" => "audio-headphones-symbolic",
            "audio-card" => "audio-speakers-symbolic",
            "input-mouse" => "input-mouse-symbolic",
            "input-keyboard" => "input-keyboard-symbolic",
            "input-gaming" => "input-gaming-symbolic",
            "input-tablet" => "input-tablet-symbolic",
            "phone" => "phone-symbolic",
            "computer" => "computer-symbolic",
            _ => "bluetooth-active-symbolic",
        }
    }
}

/// The adapter and devices; `available` is false without an adapter or BlueZ.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bluetooth {
    pub available: bool,
    pub powered: bool,
    pub discovering: bool,
    pub devices: Vec<BtDevice>,
}

impl Bluetooth {
    pub fn connected(&self) -> impl Iterator<Item = &BtDevice> {
        self.devices.iter().filter(|d| d.connected)
    }

    pub fn paired(&self) -> impl Iterator<Item = &BtDevice> {
        self.devices.iter().filter(|d| d.paired)
    }

    /// Unpaired devices found by a scan, by name.
    pub fn nearby(&self) -> impl Iterator<Item = &BtDevice> {
        self.devices.iter().filter(|d| !d.paired)
    }
}

#[derive(Debug, Clone)]
pub enum Command {
    SetPowered(bool),
    /// Scans while the Bluetooth page is open.
    Scan(bool),
    Connect(String),
    Disconnect(String),
    /// Pairs, trusts and connects.
    Pair(String),
}

#[derive(Debug, Clone)]
pub enum Update {
    Ready(mpsc::UnboundedSender<Command>),
    State(Bluetooth),
}

type Objects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

fn prop<T: TryFrom<OwnedValue>>(props: &HashMap<String, OwnedValue>, key: &str) -> Option<T> {
    props
        .get(key)
        .and_then(|v| v.try_clone().ok())
        .and_then(|v| T::try_from(v).ok())
}

/// Builds the state from BlueZ's managed objects (first adapter only).
pub fn from_objects(objects: &Objects) -> (Bluetooth, Option<String>) {
    let mut state = Bluetooth::default();
    let mut adapter_path = None;
    let mut paths: Vec<_> = objects.keys().collect();
    paths.sort_by_key(|p| p.as_str());
    for path in paths {
        let interfaces = &objects[path];
        if let Some(adapter) = interfaces.get("org.bluez.Adapter1")
            && adapter_path.is_none()
        {
            adapter_path = Some(path.to_string());
            state.available = true;
            state.powered = prop(adapter, "Powered").unwrap_or(false);
            state.discovering = prop(adapter, "Discovering").unwrap_or(false);
        }
    }
    let Some(adapter) = adapter_path.clone() else {
        return (state, None);
    };
    for (path, interfaces) in objects {
        let Some(device) = interfaces.get("org.bluez.Device1") else {
            continue;
        };
        if !path.as_str().starts_with(&format!("{adapter}/")) {
            continue;
        }
        let name: Option<String> = prop(device, "Alias").or_else(|| prop(device, "Name"));
        let paired = prop(device, "Paired").unwrap_or(false);
        // Unnamed scan results are just addresses; leave them out.
        let Some(name) = name.filter(|n| !n.is_empty() && (paired || !looks_like_address(n)))
        else {
            continue;
        };
        state.devices.push(BtDevice {
            path: path.to_string(),
            name,
            icon: prop(device, "Icon"),
            paired,
            connected: prop(device, "Connected").unwrap_or(false),
            battery: interfaces
                .get("org.bluez.Battery1")
                .and_then(|b| prop(b, "Percentage")),
        });
    }
    // Connected first, then by name.
    state.devices.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then_with(|| a.name.cmp(&b.name))
    });
    (state, Some(adapter))
}

/// "AA-BB-CC-DD-EE-FF", which BlueZ uses as the alias of nameless devices.
fn looks_like_address(name: &str) -> bool {
    name.len() == 17 && name.chars().filter(|c| *c == '-' || *c == ':').count() == 5
}

pub fn subscription() -> Subscription<Update> {
    Subscription::run(|| {
        stream::channel(8, async |mut out: futures_channel::mpsc::Sender<Update>| {
            loop {
                if let Err(err) = run(&mut out).await {
                    tracing::debug!(%err, "BlueZ unavailable");
                }
                _ = out.send(Update::State(Bluetooth::default())).await;
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
        })
    })
}

async fn run(out: &mut futures_channel::mpsc::Sender<Update>) -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let manager = ObjectManagerProxy::builder(&connection)
        .destination("org.bluez")?
        .path("/")?
        .build()
        .await?;
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .sender("org.bluez")?
        .build();
    let mut signals = MessageStream::for_match_rule(rule, &connection, Some(64)).await?;
    let (tx, mut commands) = mpsc::unbounded_channel();
    _ = out.send(Update::Ready(tx)).await;
    let mut adapter = None;
    loop {
        let objects: Objects = manager
            .get_managed_objects()
            .await?
            .into_iter()
            .map(|(path, interfaces)| {
                let interfaces = interfaces
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v))
                    .collect();
                (path, interfaces)
            })
            .collect();
        let (state, path) = from_objects(&objects);
        adapter = path.or(adapter);
        _ = out.send(Update::State(state)).await;
        tokio::select! {
            signal = signals.next() => {
                if signal.is_none() {
                    return Ok(());
                }
                // Signals come in bursts; read once after they settle.
                tokio::time::sleep(Duration::from_millis(150)).await;
                while futures_util::FutureExt::now_or_never(signals.next()).flatten().is_some() {}
            }
            Some(command) = commands.recv() => {
                if let Err(err) = execute(&connection, adapter.as_deref(), command.clone()).await {
                    tracing::warn!(%err, ?command, "Bluetooth command failed");
                }
            }
        }
    }
}

async fn execute(
    connection: &Connection,
    adapter: Option<&str>,
    command: Command,
) -> zbus::Result<()> {
    let adapter_proxy = || async {
        let path = adapter.ok_or_else(|| zbus::Error::Failure("no adapter".into()))?;
        AdapterProxy::builder(connection)
            .path(path.to_owned())?
            .build()
            .await
    };
    let device =
        |path: String| async move { DeviceProxy::builder(connection).path(path)?.build().await };
    match command {
        Command::SetPowered(on) => adapter_proxy().await?.set_powered(on).await,
        Command::Scan(true) => adapter_proxy().await?.start_discovery().await,
        Command::Scan(false) => adapter_proxy().await?.stop_discovery().await,
        Command::Connect(path) => device(path).await?.connect().await,
        Command::Disconnect(path) => device(path).await?.disconnect().await,
        Command::Pair(path) => {
            let device = device(path).await?;
            device.pair().await?;
            device.set_trusted(true).await?;
            device.connect().await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::Value;

    fn props(pairs: &[(&str, Value<'static>)]) -> HashMap<String, OwnedValue> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), OwnedValue::try_from(v.clone()).unwrap()))
            .collect()
    }

    fn path(p: &str) -> OwnedObjectPath {
        OwnedObjectPath::try_from(p.to_owned()).unwrap()
    }

    #[test]
    fn builds_state_from_managed_objects() {
        let mut objects: Objects = HashMap::new();
        objects.insert(
            path("/org/bluez/hci0"),
            HashMap::from([(
                "org.bluez.Adapter1".to_owned(),
                props(&[
                    ("Powered", Value::from(true)),
                    ("Discovering", Value::from(false)),
                ]),
            )]),
        );
        objects.insert(
            path("/org/bluez/hci0/dev_AA"),
            HashMap::from([
                (
                    "org.bluez.Device1".to_owned(),
                    props(&[
                        ("Alias", Value::from("WH-1000XM5")),
                        ("Paired", Value::from(true)),
                        ("Connected", Value::from(true)),
                        ("Icon", Value::from("audio-headphones")),
                    ]),
                ),
                (
                    "org.bluez.Battery1".to_owned(),
                    props(&[("Percentage", Value::from(90u8))]),
                ),
            ]),
        );
        objects.insert(
            path("/org/bluez/hci0/dev_BB"),
            HashMap::from([(
                "org.bluez.Device1".to_owned(),
                props(&[
                    ("Alias", Value::from("11-22-33-44-55-66")),
                    ("Paired", Value::from(false)),
                ]),
            )]),
        );
        let (state, adapter) = from_objects(&objects);
        assert_eq!(adapter.as_deref(), Some("/org/bluez/hci0"));
        assert!(state.available && state.powered);
        assert_eq!(state.devices.len(), 1, "nameless scan results are hidden");
        let headphones = &state.devices[0];
        assert_eq!(headphones.battery, Some(90));
        assert!(headphones.connected);
        assert_eq!(headphones.icon_name(), "audio-headphones-symbolic");
    }

    #[test]
    fn no_adapter_means_unavailable() {
        let (state, adapter) = from_objects(&HashMap::new());
        assert!(!state.available && adapter.is_none());
    }
}
