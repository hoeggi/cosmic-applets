// SPDX-License-Identifier: GPL-3.0-only

//! The Bluetooth tile and page.

use super::{AppModel, Message, Page};
use crate::bluetooth::{BtDevice, Command};
use crate::fl;
use crate::ui;
use cosmic::applet::padded_control;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget::{self, button, text};
use cosmic_applet_common::widgets::{icon_button, section_divider};

impl AppModel {
    pub(super) fn bluetooth_command(&self, command: Command) {
        if let Some(tx) = &self.bluetooth_tx {
            _ = tx.send(command);
        }
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
            if bt.powered {
                let paired: Vec<_> = bt.paired().collect();
                if !paired.is_empty() {
                    column = column.push(ui::group_label(fl!("my-devices")));
                    for device in paired {
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
                let nearby: Vec<_> = bt.nearby().collect();
                if nearby.is_empty() {
                    column = column.push(ui::note(fl!("searching")));
                }
                for device in nearby {
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
    let mut subtitle = None;
    let (label, command) = if !device.paired {
        (fl!("pair"), Command::Pair(device.path.clone()))
    } else if device.connected {
        subtitle = Some(match device.battery {
            Some(battery) => fl!("connected-battery", percent = battery),
            None => fl!("connected"),
        });
        (fl!("disconnect"), Command::Disconnect(device.path.clone()))
    } else {
        subtitle = Some(fl!("not-connected"));
        (fl!("connect"), Command::Connect(device.path.clone()))
    };
    let action = button::text(label)
        .on_press(Message::Bluetooth(command))
        .into();
    ui::choice_row(
        device.icon_name(),
        device.name.clone(),
        subtitle,
        device.connected,
        vec![action],
        None,
    )
}
