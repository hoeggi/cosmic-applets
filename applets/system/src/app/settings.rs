// SPDX-License-Identifier: GPL-3.0-only

//! The applet's own settings: what the popup shows (battery header, sliders and tiles in their
//! order, media controls, power buttons), panel icons, alerts, and shutdown confirmation.

use super::{AppModel, Message, Page};
use crate::config::{Config, Entry, PanelIcons, Slider, Tile, move_entry};
use crate::fl;
use crate::ui;
use cosmic::applet::padded_control;
use cosmic::cosmic_config;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget::{self, button, icon, text};
use cosmic_applet_common::config as cfg;
use cosmic_applet_common::widgets::section_divider;

/// A panel icon the user can pick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelIcon {
    Network,
    Vpn,
    Bluetooth,
    Volume,
    Battery,
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    BatteryHeader(bool),
    MoveSlider(usize, bool),
    EnableSlider(usize, bool),
    MoveTile(usize, bool),
    EnableTile(usize, bool),
    MediaControls(bool),
    PowerButtons(bool),
    Icon(PanelIcon, bool),
    BatteryPercentage(bool),
    ShowAlerts(bool),
    ConfirmShutdown(bool),
}

impl AppModel {
    fn save(
        &mut self,
        set: impl FnOnce(&mut Config, &cosmic_config::Config) -> Result<bool, cosmic_config::Error>,
    ) {
        cfg::save(&mut self.config, self.config_ctx.as_ref(), set);
    }

    pub(super) fn update_settings(&mut self, message: SettingsMessage) {
        match message {
            SettingsMessage::BatteryHeader(on) => self.save(|c, ctx| c.set_battery_header(ctx, on)),
            SettingsMessage::MoveSlider(index, up) => {
                let mut sliders = self.config.slider_entries();
                move_entry(&mut sliders, index, up);
                self.save(|c, ctx| c.set_sliders(ctx, sliders));
            }
            SettingsMessage::EnableSlider(index, on) => {
                let mut sliders = self.config.slider_entries();
                if let Some(entry) = sliders.get_mut(index) {
                    entry.enabled = on;
                }
                self.save(|c, ctx| c.set_sliders(ctx, sliders));
            }
            SettingsMessage::MoveTile(index, up) => {
                let mut tiles = self.config.tile_entries();
                move_entry(&mut tiles, index, up);
                self.save(|c, ctx| c.set_tiles(ctx, tiles));
            }
            SettingsMessage::EnableTile(index, on) => {
                let mut tiles = self.config.tile_entries();
                if let Some(entry) = tiles.get_mut(index) {
                    entry.enabled = on;
                }
                self.save(|c, ctx| c.set_tiles(ctx, tiles));
            }
            SettingsMessage::MediaControls(on) => self.save(|c, ctx| c.set_media_controls(ctx, on)),
            SettingsMessage::PowerButtons(on) => self.save(|c, ctx| c.set_power_buttons(ctx, on)),
            SettingsMessage::Icon(which, on) => {
                let mut icons = self.config.panel_icons;
                match which {
                    PanelIcon::Network => icons.network = on,
                    PanelIcon::Vpn => icons.vpn = on,
                    PanelIcon::Bluetooth => icons.bluetooth = on,
                    PanelIcon::Volume => icons.volume = on,
                    PanelIcon::Battery => icons.battery = on,
                }
                self.save(|c, ctx| c.set_panel_icons(ctx, icons));
            }
            SettingsMessage::BatteryPercentage(on) => {
                self.save(|c, ctx| c.set_battery_percentage(ctx, on));
            }
            SettingsMessage::ShowAlerts(on) => self.save(|c, ctx| c.set_show_alerts(ctx, on)),
            SettingsMessage::ConfirmShutdown(on) => {
                self.save(|c, ctx| c.set_confirm_shutdown(ctx, on));
            }
        }
    }

    pub(super) fn view_settings(&self) -> Element<'_, Message> {
        let mut column = widget::Column::new()
            .padding([8, 0])
            .push(ui::page_header(
                fl!("applet-settings-title"),
                None,
                Message::Open(Page::Main),
                Vec::new(),
            ))
            .push(section_divider())
            .push(toggle_row(
                fl!("section-battery"),
                None,
                self.config.battery_header,
                |on| Message::Settings(SettingsMessage::BatteryHeader(on)),
            ))
            .push(ui::group_label(fl!("sliders-in-order")));
        let sliders = self.config.slider_entries();
        for row in ordered_rows(
            &sliders,
            slider_name,
            slider_icon,
            SettingsMessage::MoveSlider,
            SettingsMessage::EnableSlider,
        ) {
            column = column.push(row);
        }
        column = column.push(ui::group_label(fl!("tiles-in-order")));
        let tiles = self.config.tile_entries();
        for row in ordered_rows(
            &tiles,
            tile_name,
            tile_icon,
            SettingsMessage::MoveTile,
            SettingsMessage::EnableTile,
        ) {
            column = column.push(row);
        }
        column = column
            .push(ui::group_label(fl!("more-in-popup")))
            .push(toggle_row(
                fl!("media-controls"),
                None,
                self.config.media_controls,
                |on| Message::Settings(SettingsMessage::MediaControls(on)),
            ))
            .push(toggle_row(
                fl!("power-buttons"),
                None,
                self.config.power_buttons,
                |on| Message::Settings(SettingsMessage::PowerButtons(on)),
            ));
        column = column
            .push(section_divider())
            .push(ui::group_label(fl!("panel-icons")));
        let icons: PanelIcons = self.config.panel_icons;
        for (which, on, label, note) in [
            (PanelIcon::Network, icons.network, fl!("network"), None),
            (
                PanelIcon::Vpn,
                icons.vpn,
                fl!("vpn"),
                Some(fl!("vpn-icon-note")),
            ),
            (
                PanelIcon::Bluetooth,
                icons.bluetooth,
                fl!("bluetooth"),
                None,
            ),
            (PanelIcon::Volume, icons.volume, fl!("volume"), None),
            (PanelIcon::Battery, icons.battery, fl!("battery"), None),
        ] {
            let mut info = widget::Column::new().push(text::body(label));
            if let Some(note) = note {
                info = info.push(text::caption(note));
            }
            column = column.push(
                padded_control(
                    widget::Row::new()
                        .push(widget::checkbox(on).on_toggle(move |on| {
                            Message::Settings(SettingsMessage::Icon(which, on))
                        }))
                        .push(info.width(Length::Fill))
                        .spacing(12)
                        .align_y(Alignment::Center),
                )
                .padding([4, 16]),
            );
        }
        column = column
            .push(toggle_row(
                fl!("battery-percentage"),
                None,
                self.config.battery_percentage,
                |on| Message::Settings(SettingsMessage::BatteryPercentage(on)),
            ))
            .push(toggle_row(
                fl!("show-alerts"),
                Some(fl!("show-alerts-description")),
                self.config.show_alerts,
                |on| Message::Settings(SettingsMessage::ShowAlerts(on)),
            ))
            .push(section_divider())
            .push(ui::group_label(fl!("power")))
            .push(toggle_row(
                fl!("confirm-shutdown-setting"),
                None,
                self.config.confirm_shutdown,
                |on| Message::Settings(SettingsMessage::ConfirmShutdown(on)),
            ));
        column.into()
    }
}

fn toggle_row<'a>(
    label: String,
    note: Option<String>,
    on: bool,
    message: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    let mut info = widget::Column::new().push(text::body(label));
    if let Some(note) = note {
        info = info.push(text::caption(note));
    }
    padded_control(
        widget::Row::new()
            .push(info.width(Length::Fill))
            .push(widget::toggler(on).on_toggle(message))
            .spacing(12)
            .align_y(Alignment::Center),
    )
    .padding([4, 16])
    .into()
}

/// One row per entry: icon, name, up/down arrows (moving only within this list) and a toggle.
fn ordered_rows<'a, T: Copy + 'static>(
    entries: &[Entry<T>],
    name: fn(T) -> String,
    icon_name: fn(T) -> &'static str,
    move_msg: fn(usize, bool) -> SettingsMessage,
    enable_msg: fn(usize, bool) -> SettingsMessage,
) -> Vec<Element<'a, Message>> {
    let last = entries.len().saturating_sub(1);
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let arrow = |name: &'static str, enabled: bool, up: bool| {
                button::icon(icon::from_name(name))
                    .extra_small()
                    .on_press_maybe(enabled.then(|| Message::Settings(move_msg(index, up))))
            };
            padded_control(
                widget::Row::new()
                    .push(
                        icon::from_name(icon_name(entry.item))
                            .size(16)
                            .symbolic(true)
                            .icon(),
                    )
                    .push(text::body(name(entry.item)).width(Length::Fill))
                    .push(arrow("go-up-symbolic", index > 0, true))
                    .push(arrow("go-down-symbolic", index < last, false))
                    .push(
                        widget::toggler(entry.enabled)
                            .on_toggle(move |on| Message::Settings(enable_msg(index, on))),
                    )
                    .spacing(8)
                    .align_y(Alignment::Center),
            )
            .padding([4, 16])
            .into()
        })
        .collect()
}

fn slider_name(slider: Slider) -> String {
    match slider {
        Slider::Volume => fl!("volume"),
        Slider::Microphone => fl!("microphone"),
        Slider::Brightness => fl!("brightness"),
    }
}

fn slider_icon(slider: Slider) -> &'static str {
    match slider {
        Slider::Volume => "audio-volume-high-symbolic",
        Slider::Microphone => "microphone-sensitivity-high-symbolic",
        Slider::Brightness => "display-brightness-symbolic",
    }
}

fn tile_name(tile: Tile) -> String {
    match tile {
        Tile::Network => fl!("network"),
        Tile::Bluetooth => fl!("bluetooth"),
        Tile::Output => fl!("output"),
        Tile::Airplane => fl!("airplane"),
        Tile::Vpn => fl!("vpn"),
    }
}

fn tile_icon(tile: Tile) -> &'static str {
    match tile {
        Tile::Network => "network-wireless-signal-excellent-symbolic",
        Tile::Bluetooth => "bluetooth-active-symbolic",
        Tile::Output => "audio-speakers-symbolic",
        Tile::Airplane => "airplane-mode-symbolic",
        Tile::Vpn => "network-vpn-symbolic",
    }
}
