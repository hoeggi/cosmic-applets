// SPDX-License-Identifier: GPL-3.0-only

//! The Bluetooth tile and page.

use super::{AppModel, Message, Page};
use crate::adapter::bluetooth::{BtDevice, Command, Confirmation, DeviceState};
use crate::fl;
use crate::ui;
use cosmic::applet::padded_control;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget::{self, button, text};
use cosmic_applet_common::widgets::{icon_button, section_divider};

impl AppModel {
    pub(super) fn bluetooth_command(&mut self, command: Command) {
        self.bluetooth_model.command(command);
        self.bluetooth = self.bluetooth_model.view();
    }

    pub(super) fn bluetooth_tile(&self) -> Option<Element<'_, Message>> {
        let bt = &self.bluetooth;
        if !bt.available {
            return None;
        }
        let connected: Vec<_> = bt.connected().collect();
        let status = match (bt.powered, connected.as_slice()) {
            (false, _) => fl!("off"),
            (true, []) => fl!("on"),
            (true, [one]) => one.name.clone(),
            (true, many) => fl!("devices-connected", count = many.len()),
        };
        let icon = if bt.powered {
            "bluetooth-active-symbolic"
        } else {
            "bluetooth-disabled-symbolic"
        };
        Some(ui::tile(
            icon,
            fl!("bluetooth"),
            status,
            bt.powered,
            Some(Message::Bluetooth(Command::SetPowered(!bt.powered))),
            Some(Message::Open(Page::Bluetooth)),
        ))
    }

    pub(super) fn view_bluetooth(&self) -> Element<'_, Message> {
        let bt = &self.bluetooth;
        let refresh = icon_button("view-refresh-symbolic")
            .on_press_maybe(
                bt.powered
                    .then_some(Message::Bluetooth(Command::Scan(true))),
            )
            .into();
        let mut column = widget::Column::new().padding([8, 0]).push(ui::page_header(
            fl!("bluetooth"),
            None,
            Message::Open(Page::Main),
            vec![refresh],
        ));
        if !bt.available {
            column = column
                .push(section_divider())
                .push(ui::note(fl!("bluetooth-unavailable")));
        } else {
            column = column
                .push(ui::section_title(
                    fl!("bluetooth"),
                    Some(
                        widget::toggler(bt.powered)
                            .on_toggle(|on| Message::Bluetooth(Command::SetPowered(on)))
                            .into(),
                    ),
                ))
                .push(section_divider());
            if let Some(confirmation) = &bt.confirmation {
                column = column.push(confirmation_view(confirmation));
            }
            if bt.powered {
                if !bt.known.is_empty() {
                    column = column.push(ui::group_label(fl!("my-devices")));
                    for device in &bt.known {
                        column = column.push(device_row(device));
                    }
                }
                let spinner: Element<'_, Message> = if bt.discovering {
                    widget::progress_bar::indeterminate_circular()
                        .size(14.0)
                        .into()
                } else {
                    widget::Space::new().into()
                };
                column = column.push(
                    padded_control(
                        widget::Row::new()
                            .push(text::caption_heading(fl!("nearby")).width(Length::Fill))
                            .push(spinner)
                            .align_y(Alignment::Center),
                    )
                    .padding([6, 16, 0, 16]),
                );
                if bt.nearby.is_empty() {
                    column = column.push(ui::note(fl!("searching")));
                }
                for device in &bt.nearby {
                    column = column.push(device_row(device));
                }
            }
        }
        column
            .push(section_divider())
            .push(ui::settings_link(
                fl!("bluetooth-settings"),
                Message::OpenSettings("bluetooth"),
            ))
            .into()
    }
}

fn device_row(device: &BtDevice) -> Element<'_, Message> {
    let address = device.address;
    let (subtitle, action) = match device.state {
        DeviceState::Connected => (
            Some(match device.battery {
                Some(battery) => fl!("connected-battery", percent = battery),
                None => fl!("connected"),
            }),
            Some((fl!("disconnect"), Command::Disconnect(address))),
        ),
        DeviceState::Paired => (
            Some(fl!("not-connected")),
            Some((fl!("connect"), Command::Connect(address))),
        ),
        DeviceState::New => (None, Some((fl!("pair"), Command::Pair(address)))),
        // In progress: no button until BlueZ reports the result, like upstream.
        DeviceState::Connecting => (Some(fl!("connecting")), None),
        DeviceState::Disconnecting => (Some(fl!("disconnecting")), None),
        DeviceState::Pairing => (Some(fl!("pairing")), None),
    };
    let trailing = action
        .map(|(label, command)| {
            button::text(label)
                .on_press(Message::Bluetooth(command))
                .into()
        })
        .into_iter()
        .collect();
    ui::choice_row(
        device.icon,
        device.name.clone(),
        subtitle,
        device.connected(),
        trailing,
        None,
    )
}

/// Upstream's PIN confirmation: the device, the PIN to compare, Cancel and Confirm.
fn confirmation_view(confirmation: &Confirmation) -> Element<'_, Message> {
    padded_control(
        widget::Column::new()
            .push(
                widget::Row::new()
                    .push(
                        widget::icon::from_name(confirmation.icon)
                            .size(16)
                            .symbolic(true),
                    )
                    .push(text::body(confirmation.device.clone()))
                    .spacing(12)
                    .align_y(Alignment::Center),
            )
            .push(text::body(fl!(
                "confirm-pin",
                device = confirmation.device.clone()
            )))
            .push(
                text::title3(confirmation.pin.clone())
                    .width(Length::Fill)
                    .align_x(Alignment::Center),
            )
            .push(
                widget::Row::new()
                    .push(widget::space::horizontal())
                    .push(
                        button::standard(fl!("cancel"))
                            .on_press(Message::Bluetooth(Command::Confirm(false))),
                    )
                    .push(
                        button::suggested(fl!("confirm"))
                            .on_press(Message::Bluetooth(Command::Confirm(true))),
                    )
                    .spacing(8),
            )
            .spacing(8),
    )
    .padding([6, 16])
    .into()
}
