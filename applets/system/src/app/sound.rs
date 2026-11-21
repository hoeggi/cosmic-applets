// SPDX-License-Identifier: GPL-3.0-only

//! Volume and microphone sliders, the Output tile and the Sound page.

use super::{AppModel, Message, Page, audio_task};
use crate::audio::{self, Node};
use crate::fl;
use crate::ui;
use cosmic::prelude::*;
use cosmic::widget;
use cosmic_applet_common::widgets::section_divider;

impl AppModel {
    pub(super) fn set_sink_volume(&mut self, volume: u32) -> Task<cosmic::Action<Message>> {
        let Some(id) = self.audio.default_sink else {
            return Task::none();
        };
        self.set_volume(id, volume, true)
    }

    pub(super) fn set_source_volume(&mut self, volume: u32) -> Task<cosmic::Action<Message>> {
        let Some(id) = self.audio.default_source else {
            return Task::none();
        };
        self.set_volume(id, volume, false)
    }

    /// Shows the new value at once and sends it debounced. Moving a slider unmutes, as the
    /// built-in applet does.
    fn set_volume(&mut self, id: u32, volume: u32, sink: bool) -> Task<cosmic::Action<Message>> {
        let mut tasks = Vec::new();
        if let Some(node) = self.audio.nodes.get_mut(&id) {
            node.volume = volume;
            if node.mute && volume > 0 {
                node.mute = false;
                tasks.push(audio_task(audio::Command::SetMute(id, false)));
            }
        }
        let debounced = if sink {
            &self.sink_volume
        } else {
            &self.source_volume
        };
        if let Some(send) = debounced.set(id, volume) {
            tasks.push(Task::perform(send, |()| cosmic::Action::App(Message::Done)));
        }
        Task::batch(tasks)
    }

    pub(super) fn sink_slider(&self) -> Option<Element<'_, Message>> {
        let sink = self.audio.sink()?;
        Some(slider(sink, Message::ToggleSinkMute, Message::SinkVolume))
    }

    pub(super) fn source_slider(&self) -> Option<Element<'_, Message>> {
        let source = self.audio.source()?;
        Some(slider(
            source,
            Message::ToggleSourceMute,
            Message::SourceVolume,
        ))
    }

    /// Body toggles mute; the chevron opens the Sound page.
    pub(super) fn output_tile(&self) -> Element<'_, Message> {
        let (icon, status, on) = match self.audio.sink() {
            Some(sink) => (self.audio.node_icon(sink), sink.name.clone(), !sink.mute),
            None => ("audio-speakers-symbolic", fl!("no-output"), false),
        };
        ui::tile(
            icon,
            fl!("output"),
            status,
            on,
            Some(Message::ToggleSinkMute),
            Some(Message::Open(Page::Sound)),
        )
    }

    pub(super) fn view_sound(&self) -> Element<'_, Message> {
        let mut column = widget::Column::new()
            .padding([8, 0])
            .push(ui::page_header(
                fl!("sound"),
                None,
                Message::Open(Page::Main),
                Vec::new(),
            ))
            .push(section_divider());
        if !self.audio.connected {
            column = column.push(ui::note(fl!("audio-unavailable")));
        } else {
            if let Some(slider) = self.sink_slider() {
                column = column.push(slider);
            }
            column = column.push(ui::group_label(fl!("output")));
            for sink in self.audio.sinks() {
                column = column.push(self.node_row(sink, self.audio.default_sink));
            }
            column = column.push(section_divider());
            if let Some(slider) = self.source_slider() {
                column = column.push(slider);
            }
            column = column.push(ui::group_label(fl!("input")));
            for source in self.audio.sources() {
                column = column.push(self.node_row(source, self.audio.default_source));
            }
        }
        column
            .push(section_divider())
            .push(ui::settings_link(
                fl!("sound-settings"),
                Message::OpenSettings("sound"),
            ))
            .into()
    }

    fn node_row<'a>(&'a self, node: &'a Node, default: Option<u32>) -> Element<'a, Message> {
        let selected = default == Some(node.id);
        ui::choice_row(
            self.audio.node_icon(node),
            node.name.clone(),
            None,
            selected,
            Vec::new(),
            (!selected).then_some(Message::SetDefault(node.id)),
        )
    }
}

fn slider<'a>(
    node: &Node,
    toggle_mute: Message,
    on_change: fn(u32) -> Message,
) -> Element<'a, Message> {
    let (icon, label) = if node.is_sink {
        (audio::volume_icon(node.volume, node.mute), node.volume)
    } else {
        (audio::mic_icon(node.volume, node.mute), node.volume)
    };
    let label = if node.mute {
        fl!("muted")
    } else {
        format!("{label}%")
    };
    ui::slider_row(icon, Some(toggle_mute), node.volume, label, on_change, None)
}
