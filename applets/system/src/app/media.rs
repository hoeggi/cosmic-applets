// SPDX-License-Identifier: GPL-3.0-only

//! Media controls: one row per running player, between the tiles and the power row.

use super::{AppModel, Message};
use crate::adapter::media::PlayerRow;
use crate::fl;
use crate::upstream::audio::mpris_subscription::MprisRequest;
use cosmic::iced::{Alignment, ContentFit, Length};
use cosmic::prelude::*;
use cosmic::widget::{self, button, icon, text};

/// Size of the album art or its placeholder.
const ART: f32 = 40.0;

// Upstream's icon names for the controls.
const GO_BACK: &str = "media-skip-backward-symbolic";
const GO_NEXT: &str = "media-skip-forward-symbolic";
const PAUSE: &str = "media-playback-pause-symbolic";
const PLAY: &str = "media-playback-start-symbolic";

impl AppModel {
    /// The rows, or nothing when no player runs.
    pub(super) fn media_rows(&self) -> Vec<Element<'_, Message>> {
        self.media.rows().into_iter().map(player_row).collect()
    }
}

/// Art, title and "artist · app"; the row raises the player, the buttons control it. All three
/// buttons are always shown; those the player can't use are disabled.
fn player_row<'a>(row: PlayerRow) -> Element<'a, Message> {
    let art: Element<'a, Message> = match &row.art {
        Some(path) => widget::icon(icon::from_path(path.clone()))
            .size(ART as u16)
            .content_fit(ContentFit::Cover)
            .into(),
        None => widget::container(
            icon::from_name("audio-x-generic-symbolic")
                .size(20)
                .symbolic(true)
                .icon(),
        )
        .width(Length::Fixed(ART))
        .height(Length::Fixed(ART))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .class(cosmic::theme::Container::custom(|theme| {
            let cosmic = theme.cosmic();
            widget::container::Style {
                background: Some(cosmic::iced::Color::from(cosmic.button.base).into()),
                icon_color: Some(cosmic.button.on.into()),
                border: cosmic::iced::Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }))
        .into(),
    };
    let artist = row.artist.clone().unwrap_or_else(|| fl!("unknown-artist"));
    let subtitle = match &row.app {
        Some(app) => format!("{artist} · {app}"),
        None => artist,
    };
    let control = |name: &'static str, enabled: bool, request: MprisRequest| {
        button::icon(icon::from_name(name).symbolic(true))
            .extra_small()
            .on_press_maybe(enabled.then(|| Message::MediaRequest(row.id.clone(), request)))
    };
    let play_pause = if row.play_pause.play {
        control(PLAY, row.play_pause.enabled, MprisRequest::Play)
    } else {
        control(PAUSE, row.play_pause.enabled, MprisRequest::Pause)
    };
    let content = widget::Row::new()
        .push(art)
        .push(
            widget::Column::new()
                .push(text::body(row.title.clone()))
                .push(text::caption(subtitle))
                .width(Length::Fill),
        )
        .push(control(GO_BACK, row.previous, MprisRequest::Previous))
        .push(play_pause)
        .push(control(GO_NEXT, row.next, MprisRequest::Next))
        .spacing(10)
        .align_y(Alignment::Center);
    cosmic::applet::menu_button(content)
        .on_press(Message::MediaRequest(row.id, MprisRequest::Raise))
        .into()
}
