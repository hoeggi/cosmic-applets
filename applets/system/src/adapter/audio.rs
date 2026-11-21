// SPDX-License-Identifier: GPL-3.0-only

//! Audio for our UI, from upstream's audio applet: its model (`crate::upstream::audio::model`:
//! node names from the active route, unplugged inputs removed, sorted lists, the default
//! device's volume and mute), its icons, its subscription loop and the same client calls
//! (`set_sink_volume` / `set_source_volume` 128 ms after the last change, `sink_mute_toggle` /
//! `source_mute_toggle`, `set_default`, wheel steps at most every 192 ms).
//!
//! Still ours: the device icons in the device lists (headphones, headset, display, …), from the
//! audio service's device icon names; upstream's popup shows no per-device icons.

use crate::upstream::audio::{icons, model};
use cosmic::iced::{Subscription, futures::SinkExt, stream};
use cosmic_settings_audio_client::{self as client, CosmicAudioProxy, Event};
use futures_util::StreamExt;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

pub use icons::{input_icon_name as mic_icon, output_icon_name as volume_icon};

/// Upstream waits this long after a slider change before sending the latest value.
const SEND_DELAY: Duration = Duration::from_millis(128);
/// Upstream applies at most one wheel step per this interval.
const WHEEL_INTERVAL: Duration = Duration::from_millis(192);

#[derive(Debug, Clone)]
pub enum Update {
    Connected,
    Event(Box<Event>),
    Unavailable,
}

/// Upstream's subscription loop: connect, stream events, reconnect after 3 s.
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
                                    Err(err) => tracing::error!(?err, "audio event error"),
                                }
                            }
                        }
                    }
                    Err(err) => tracing::debug!(?err, "cosmic-settings-daemon's audio service"),
                }
                _ = out.send(Update::Unavailable).await;
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        })
    })
}

/// A device row.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: u32,
    /// Upstream's display name, "route - device" (e.g. "Analog Stereo - Pebble V3").
    pub name: String,
    /// The device alone (e.g. "Pebble V3"), for places without room for the route.
    pub device_name: String,
    pub is_sink: bool,
    pub volume: u32,
    pub mute: bool,
    pub device: Option<u32>,
}

/// Upstream's model plus the device icon names (ours).
#[derive(Debug, Default)]
pub struct Model {
    pub connected: bool,
    upstream: model::Model,
    device_icons: BTreeMap<u32, String>,
    last_wheel: Option<Instant>,
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
            Update::Event(event) => {
                match &*event {
                    Event::Device(id, info) => {
                        self.device_icons.insert(*id, info.icon_name.clone());
                    }
                    Event::RemoveDevice(id) => {
                        self.device_icons.remove(id);
                    }
                    _ => {}
                }
                self.upstream.update(*event);
            }
        }
    }

    pub fn default_sink(&self) -> Option<u32> {
        self.upstream.default_sink
    }

    pub fn default_source(&self) -> Option<u32> {
        self.upstream.default_source
    }

    /// Output devices in upstream's (alphabetical) order.
    pub fn sinks(&self) -> Vec<Node> {
        nodes(&self.upstream.sinks, true)
    }

    /// Input devices in upstream's order; unplugged ones are already gone.
    pub fn sources(&self) -> Vec<Node> {
        nodes(&self.upstream.sources, false)
    }

    /// The default output, with upstream's `active_sink` volume and mute.
    pub fn sink(&self) -> Option<Node> {
        let id = self.upstream.default_sink?;
        let mut node = self.sinks().into_iter().find(|n| n.id == id)?;
        node.volume = self.upstream.active_sink.volume;
        node.mute = self.upstream.active_sink.mute;
        Some(node)
    }

    /// The default input, with upstream's `active_source` volume and mute.
    pub fn source(&self) -> Option<Node> {
        let id = self.upstream.default_source?;
        let mut node = self.sources().into_iter().find(|n| n.id == id)?;
        node.volume = self.upstream.active_source.volume;
        node.mute = self.upstream.active_source.mute;
        Some(node)
    }

    /// Icon for a node's device, e.g. headphones or a Bluetooth headset.
    pub fn node_icon(&self, node: &Node) -> &'static str {
        let device_icon = node.device.and_then(|d| self.device_icons.get(&d));
        device_icon_name(device_icon.map(String::as_str), &node.name, node.is_sink)
    }

    /// Upstream's `SetSinkVolume`: unmuted and shown at once, sent after [`SEND_DELAY`].
    pub fn set_sink_volume(&mut self, volume: u32) {
        self.upstream.active_sink.mute = false;
        self.upstream.active_sink.volume = volume;
        self.upstream.active_sink.volume_text = volume.to_string();
    }

    /// Upstream's `SetSourceVolume`: shown at once, sent after [`SEND_DELAY`].
    pub fn set_source_volume(&mut self, volume: u32) {
        self.upstream.active_source.volume = volume;
        self.upstream.active_source.volume_text = volume.to_string();
    }

    /// Upstream's `SetSinkVolumeScroll` throttle: the new volume, or `None` to skip this step.
    pub fn wheel(&mut self, delta: f32, now: Instant) -> Option<u32> {
        let current = self.upstream.active_sink.volume as f32;
        let volume = (current + delta).clamp(0.0, 150.0) as u32;
        if self
            .last_wheel
            .is_some_and(|last| now.duration_since(last) < WHEEL_INTERVAL)
            || volume == self.upstream.active_sink.volume
        {
            return None;
        }
        self.last_wheel = Some(now);
        self.set_sink_volume(volume);
        Some(volume)
    }
}

fn nodes(nodes: &model::Nodes, is_sink: bool) -> Vec<Node> {
    nodes
        .sorted_index
        .iter()
        .map(|&i| i as usize)
        .filter(|&i| i < nodes.id.len())
        .map(|i| Node {
            id: nodes.id[i],
            name: nodes.display[i].to_string(),
            device_name: nodes
                .description
                .get(i)
                .filter(|d| !d.is_empty())
                .cloned()
                .unwrap_or_else(|| nodes.display[i].to_string()),
            is_sink,
            volume: nodes.volume[i],
            mute: nodes.mute[i],
            device: nodes.devices[i],
        })
        .collect()
}

fn device_icon_name(device_icon: Option<&str>, name: &str, is_sink: bool) -> &'static str {
    let hint = format!("{} {}", device_icon.unwrap_or_default(), name).to_lowercase();
    if hint.contains("headset") {
        "audio-headset-symbolic"
    } else if hint.contains("headphone") {
        "audio-headphones-symbolic"
    } else if hint.contains("hdmi") || hint.contains("displayport") || hint.contains(" dp") {
        "video-display-symbolic"
    } else if is_sink {
        "audio-speakers-symbolic"
    } else {
        "audio-input-microphone-symbolic"
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Command {
    SetDefault(u32),
    ToggleSinkMute,
    ToggleSourceMute,
    /// Wheel steps go out at once (upstream sends them without the slider delay).
    SinkVolumeNow(u32),
}

/// One-shot commands; each opens its own connection to the local socket.
pub async fn run(command: Command) {
    let result = async {
        let mut client = client::connect().await?;
        match command {
            Command::SetDefault(id) => client.conn.set_default(id, true).await.map(|_| ()),
            Command::ToggleSinkMute => client.conn.sink_mute_toggle().await.map(|_| ()),
            Command::ToggleSourceMute => client.conn.source_mute_toggle().await.map(|_| ()),
            Command::SinkVolumeNow(volume) => client.conn.set_sink_volume(volume).await.map(|_| ()),
        }
    }
    .await;
    if let Err(err) = result {
        tracing::warn!(?err, ?command, "audio command failed");
    }
}

/// A slider's latest value, sent [`SEND_DELAY`] after the first change of a burst (upstream
/// stores the value in an atomic and sends what's there when the delay ends).
#[derive(Debug, Default, Clone)]
pub struct Delayed {
    value: Arc<AtomicU32>,
    pending: Arc<AtomicBool>,
}

impl Delayed {
    /// Stores the value; returns a future to spawn if no send is already scheduled.
    pub fn set(
        &self,
        sink: bool,
        value: u32,
    ) -> Option<impl std::future::Future<Output = ()> + Send + 'static> {
        self.value.store(value, Ordering::Relaxed);
        if self.pending.swap(true, Ordering::AcqRel) {
            return None;
        }
        let latest = self.value.clone();
        let pending = self.pending.clone();
        Some(async move {
            tokio::time::sleep(SEND_DELAY).await;
            pending.store(false, Ordering::Release);
            let volume = latest.load(Ordering::Relaxed);
            let result = async {
                let mut client = client::connect().await?;
                if sink {
                    client.conn.set_sink_volume(volume).await.map(|_| ())
                } else {
                    client.conn.set_source_volume(volume).await.map(|_| ())
                }
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

    fn node_info(description: &str, profile: &str, is_sink: bool) -> NodeInfo {
        NodeInfo {
            name: description.to_lowercase(),
            description: description.into(),
            device_profile_description: profile.into(),
            device_id: None,
            card_profile_device: None,
            is_sink,
        }
    }

    fn event(model: &mut Model, event: Event) {
        model.update(Update::Event(Box::new(event)));
    }

    #[test]
    fn default_sink_uses_upstreams_active_volume() {
        let mut model = Model::default();
        model.update(Update::Connected);
        event(
            &mut model,
            Event::Node(81, node_info("Pebble V3", "Analog Stereo", true)),
        );
        event(
            &mut model,
            Event::Node(80, node_info("Arctis Nova 7", "Digital Stereo", true)),
        );
        event(&mut model, Event::DefaultSink(81));
        event(&mut model, Event::NodeVolume(81, 40, None));
        let sink = model.sink().expect("default sink");
        assert_eq!((sink.id, sink.volume, sink.mute), (81, 40, false));
        assert_eq!(sink.name, "Analog Stereo - Pebble V3");
        // Upstream sorts the list by name.
        let names: Vec<_> = model.sinks().into_iter().map(|n| n.id).collect();
        assert_eq!(names, [81, 80]);
        model.set_sink_volume(55);
        assert_eq!(model.sink().map(|s| s.volume), Some(55));
    }

    #[test]
    fn wheel_steps_are_throttled_like_upstream() {
        let mut model = Model::default();
        model.update(Update::Connected);
        event(&mut model, Event::Node(1, node_info("Speakers", "", true)));
        event(&mut model, Event::DefaultSink(1));
        event(&mut model, Event::NodeVolume(1, 50, None));
        let t = Instant::now();
        assert_eq!(model.wheel(5.0, t), Some(55));
        assert_eq!(model.wheel(5.0, t + Duration::from_millis(100)), None);
        assert_eq!(model.wheel(5.0, t + Duration::from_millis(200)), Some(60));
    }

    #[test]
    fn icons() {
        assert_eq!(volume_icon(50, true), "audio-volume-muted-symbolic");
        assert_eq!(
            volume_icon(120, false),
            "audio-volume-overamplified-symbolic"
        );
        assert_eq!(mic_icon(70, false), "microphone-sensitivity-high-symbolic");
    }
}
