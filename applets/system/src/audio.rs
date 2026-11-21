// Copyright 2026 System76 <info@system76.com>
// Copyright 2026 Daniel Hoeggerl
// SPDX-License-Identifier: GPL-3.0-only

//! Sinks and sources from cosmic-settings-daemon's audio service. The connection loop and the
//! volume debounce are adapted from cosmic-applet-audio.

use cosmic::iced::{Subscription, stream};
use cosmic_settings_audio_client::{self as client, CosmicAudioProxy, Event};
use futures_util::{SinkExt, StreamExt};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

/// How long a slider waits before sending, so a drag sends a few updates instead of hundreds.
const DEBOUNCE: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub enum Update {
    /// (Re)connected: forget everything, the daemon replays the current state.
    Connected,
    Event(Box<Event>),
    /// The daemon isn't reachable; retrying.
    Unavailable,
}

/// Streams the daemon's audio events, reconnecting every 3 s while it is unavailable.
pub fn subscription() -> Subscription<Update> {
    Subscription::run(|| {
        stream::channel(8, async |mut out: futures_channel::mpsc::Sender<Update>| {
            loop {
                match client::connect().await {
                    Ok(mut client) => {
                        if let Ok(Ok(mut events)) = client.recv_events().await {
                            _ = out.send(Update::Connected).await;
                            while let Some(event) = events.next().await {
                                match event {
                                    Ok(event) => _ = out.send(Update::Event(Box::new(event))).await,
                                    Err(err) => tracing::warn!(?err, "audio event error"),
                                }
                            }
                            tracing::warn!("audio service closed the event stream");
                        }
                    }
                    Err(err) => tracing::debug!(?err, "audio service unavailable"),
                }
                _ = out.send(Update::Unavailable).await;
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        })
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: u32,
    pub name: String,
    pub is_sink: bool,
    pub volume: u32,
    pub mute: bool,
    pub device: Option<u32>,
}

/// What the popup shows about audio.
#[derive(Debug, Default)]
pub struct Model {
    pub connected: bool,
    pub nodes: BTreeMap<u32, Node>,
    pub default_sink: Option<u32>,
    pub default_source: Option<u32>,
    devices: BTreeMap<u32, String>,
}

impl Model {
    pub fn update(&mut self, update: Update) {
        match update {
            Update::Connected => {
                *self = Model {
                    connected: true,
                    ..Model::default()
                }
            }
            Update::Unavailable => *self = Model::default(),
            Update::Event(event) => self.apply(*event),
        }
    }

    fn apply(&mut self, event: Event) {
        match event {
            Event::Node(id, info) => {
                let node = self.nodes.entry(id).or_insert(Node {
                    id,
                    name: String::new(),
                    is_sink: info.is_sink,
                    volume: 0,
                    mute: false,
                    device: info.device_id,
                });
                node.is_sink = info.is_sink;
                node.device = info.device_id;
                node.name = display_name(&info.description, &info.device_profile_description);
            }
            Event::NodeVolume(id, volume, _balance) => {
                if let Some(node) = self.nodes.get_mut(&id) {
                    node.volume = volume;
                }
            }
            Event::NodeMute(id, mute) => {
                if let Some(node) = self.nodes.get_mut(&id) {
                    node.mute = mute;
                }
            }
            Event::RemoveNode(id) => {
                self.nodes.remove(&id);
            }
            Event::DefaultSink(id) => self.default_sink = Some(id),
            Event::DefaultSource(id) => self.default_source = Some(id),
            Event::Device(id, info) => {
                self.devices.insert(id, info.icon_name);
            }
            Event::RemoveDevice(id) => {
                self.devices.remove(&id);
            }
            _ => {}
        }
    }

    pub fn sinks(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values().filter(|n| n.is_sink)
    }

    pub fn sources(&self) -> impl Iterator<Item = &Node> {
        self.nodes.values().filter(|n| !n.is_sink)
    }

    pub fn sink(&self) -> Option<&Node> {
        self.default_sink.and_then(|id| self.nodes.get(&id))
    }

    pub fn source(&self) -> Option<&Node> {
        self.default_source.and_then(|id| self.nodes.get(&id))
    }

    /// Icon for a node's device, e.g. headphones or a Bluetooth headset.
    pub fn node_icon(&self, node: &Node) -> &'static str {
        let device_icon = node.device.and_then(|d| self.devices.get(&d));
        device_icon_name(device_icon.map(String::as_str), &node.name, node.is_sink)
    }
}

/// The node's description, unless it only repeats its device's profile description.
fn display_name(description: &str, profile: &str) -> String {
    if description.is_empty() {
        profile.to_owned()
    } else {
        description.to_owned()
    }
}

fn device_icon_name(device_icon: Option<&str>, name: &str, is_sink: bool) -> &'static str {
    let hint = format!("{} {}", device_icon.unwrap_or_default(), name).to_lowercase();
    if hint.contains("headset") {
        "audio-headset-symbolic"
    } else if hint.contains("headphone") {
        "audio-headphones-symbolic"
    } else if hint.contains("hdmi") || hint.contains("displayport") || hint.contains("video") {
        "video-display-symbolic"
    } else if is_sink {
        "audio-speakers-symbolic"
    } else {
        "audio-input-microphone-symbolic"
    }
}

/// Speaker icon for a volume and mute state.
pub fn volume_icon(volume: u32, mute: bool) -> &'static str {
    match (mute, volume) {
        (true, _) | (_, 0) => "audio-volume-muted-symbolic",
        (_, 1..=33) => "audio-volume-low-symbolic",
        (_, 34..=66) => "audio-volume-medium-symbolic",
        _ => "audio-volume-high-symbolic",
    }
}

pub fn mic_icon(volume: u32, mute: bool) -> &'static str {
    match (mute, volume) {
        (true, _) | (_, 0) => "microphone-sensitivity-muted-symbolic",
        (_, 1..=33) => "microphone-sensitivity-low-symbolic",
        (_, 34..=66) => "microphone-sensitivity-medium-symbolic",
        _ => "microphone-sensitivity-high-symbolic",
    }
}

/// One-shot commands. Each opens its own connection: they are rare, and the socket is local.
#[derive(Debug, Clone, Copy)]
pub enum Command {
    SetDefault(u32),
    SetMute(u32, bool),
}

pub async fn run(command: Command) {
    let result = async {
        let mut client = client::connect().await?;
        match command {
            Command::SetDefault(id) => client.conn.set_default(id, true).await.map(|_| ()),
            Command::SetMute(id, mute) => client.conn.set_node_mute(id, mute).await.map(|_| ()),
        }
    }
    .await;
    if let Err(err) = result {
        tracing::warn!(?err, ?command, "audio command failed");
    }
}

/// A slider's latest value, sent at most every [`DEBOUNCE`].
#[derive(Debug, Default, Clone)]
pub struct Debounced {
    value: Arc<AtomicU32>,
    pending: Arc<AtomicBool>,
}

impl Debounced {
    /// Stores the value; returns a future to spawn if no send is already scheduled.
    pub fn set(
        &self,
        node: u32,
        value: u32,
    ) -> Option<impl std::future::Future<Output = ()> + Send + 'static> {
        self.value.store(value, Ordering::Relaxed);
        if self.pending.swap(true, Ordering::AcqRel) {
            return None;
        }
        let latest = self.value.clone();
        let pending = self.pending.clone();
        Some(async move {
            tokio::time::sleep(DEBOUNCE).await;
            pending.store(false, Ordering::Release);
            let volume = latest.load(Ordering::Relaxed);
            let result = async {
                let mut client = client::connect().await?;
                client.conn.set_node_volume(node, volume).await.map(|_| ())
            }
            .await;
            if let Err(err) = result {
                tracing::warn!(?err, "setting the volume failed");
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_settings_audio_client::NodeInfo;

    fn node_info(description: &str, is_sink: bool) -> NodeInfo {
        NodeInfo {
            name: "alsa_output.x".into(),
            description: description.into(),
            device_profile_description: "Analog Stereo".into(),
            device_id: Some(7),
            card_profile_device: None,
            is_sink,
        }
    }

    #[test]
    fn tracks_default_sink_and_its_volume() {
        let mut model = Model::default();
        model.update(Update::Connected);
        model.update(Update::Event(Box::new(Event::Node(
            1,
            node_info("Speakers", true),
        ))));
        model.update(Update::Event(Box::new(Event::Node(
            2,
            node_info("Mic", false),
        ))));
        model.update(Update::Event(Box::new(Event::NodeVolume(1, 64, None))));
        model.update(Update::Event(Box::new(Event::DefaultSink(1))));
        let sink = model.sink().unwrap();
        assert_eq!((sink.name.as_str(), sink.volume), ("Speakers", 64));
        assert_eq!(model.sources().count(), 1);
        model.update(Update::Event(Box::new(Event::RemoveNode(1))));
        assert!(model.sink().is_none());
    }

    #[test]
    fn reconnect_forgets_old_nodes() {
        let mut model = Model::default();
        model.update(Update::Connected);
        model.update(Update::Event(Box::new(Event::Node(
            1,
            node_info("Speakers", true),
        ))));
        model.update(Update::Unavailable);
        assert!(!model.connected && model.nodes.is_empty());
    }

    #[test]
    fn icons() {
        assert_eq!(volume_icon(50, true), "audio-volume-muted-symbolic");
        assert_eq!(volume_icon(80, false), "audio-volume-high-symbolic");
        assert_eq!(volume_icon(20, false), "audio-volume-low-symbolic");
        assert_eq!(
            device_icon_name(Some("audio-headset-bluetooth"), "WH-1000XM5", true),
            "audio-headset-symbolic"
        );
        assert_eq!(
            device_icon_name(None, "HDMI / DisplayPort", true),
            "video-display-symbolic"
        );
        assert_eq!(
            device_icon_name(None, "Built-in", false),
            "audio-input-microphone-symbolic"
        );
    }
}
