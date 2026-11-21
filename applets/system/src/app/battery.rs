// SPDX-License-Identifier: GPL-3.0-only

//! Battery header, brightness slider and the Battery page.

use super::{AppModel, Message, Page};
use crate::battery::{self, Charge, LOW_PERCENT};
use crate::fl;
use crate::ui;
use cosmic::applet::padded_control;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget::{self, icon, text};
use cosmic_applet_common::widgets::{section_divider, status_color};

impl AppModel {
    pub(super) fn set_brightness(&mut self, percent: u32) -> Task<cosmic::Action<Message>> {
        self.brightness = Some(percent);
        match self.brightness_debounce.set(percent) {
            Some(send) => Task::perform(send, |()| cosmic::Action::App(Message::Done)),
            None => Task::none(),
        }
    }

    pub(super) fn brightness_slider(&self) -> Option<Element<'_, Message>> {
        let percent = self.brightness?;
        Some(ui::slider_row(
            "display-brightness-symbolic",
            None,
            percent,
            format!("{percent}%"),
            Message::Brightness,
            None,
        ))
    }

    /// "78% · 3 h 40 min left" plus a meter, the power mode and a chevron to the Battery page.
    pub(super) fn battery_header(&self) -> Option<Element<'_, Message>> {
        let battery = &self.battery;
        if !battery.present {
            return None;
        }
        let low = battery.is_low();
        let mut percent = text::title4(format!("{:.0}%", battery.percentage));
        if low {
            percent = percent.class(cosmic::theme::Text::Color(status_color(false)));
        }
        let summary = widget::Row::new()
            .push(percent)
            .push(text::caption(format!("· {}", charge_text(battery))))
            .spacing(6)
            .align_y(Alignment::Center);
        let info = widget::Column::new()
            .push(summary)
            .push(meter(battery.percentage, low))
            .spacing(4)
            .width(Length::Fill);
        let mut row = widget::Row::new()
            .push(
                icon::from_name(battery.icon())
                    .size(24)
                    .symbolic(true)
                    .icon(),
            )
            .push(info)
            .spacing(12)
            .align_y(Alignment::Center);
        if let Some(profile) = &battery.profile {
            row = row.push(text::caption(profile_name(profile)));
        }
        row = row.push(
            icon::from_name("go-next-symbolic")
                .size(16)
                .symbolic(true)
                .icon(),
        );
        Some(
            cosmic::applet::menu_button(row)
                .on_press(Message::Open(Page::Battery))
                .into(),
        )
    }

    pub(super) fn view_battery(&self) -> Element<'_, Message> {
        let battery = &self.battery;
        let mut column = widget::Column::new()
            .padding([8, 0])
            .push(ui::page_header(
                fl!("battery"),
                None,
                Message::Open(Page::Main),
                Vec::new(),
            ))
            .push(section_divider());
        if battery.present {
            let mut details = charge_text(battery);
            if let Some(watts) = battery.watts {
                details.push_str(&format!(" · {watts:.1} W"));
            }
            column = column.push(
                padded_control(
                    widget::Column::new()
                        .push(
                            widget::Row::new()
                                .push(text::title3(format!("{:.0}%", battery.percentage)))
                                .push(text::caption(details))
                                .spacing(10)
                                .align_y(Alignment::Center),
                        )
                        .push(meter(battery.percentage, battery.is_low()))
                        .spacing(6),
                )
                .padding([4, 16]),
            );
        }
        if !battery.profiles.is_empty() {
            column = column
                .push(section_divider())
                .push(ui::group_label(fl!("power-mode")));
            for profile in &battery.profiles {
                let selected = battery.profile.as_ref() == Some(profile);
                column = column.push(ui::choice_row(
                    profile_icon(profile),
                    profile_name(profile),
                    Some(profile_description(profile)),
                    selected,
                    Vec::new(),
                    (!selected).then(|| Message::SetProfile(profile.clone())),
                ));
            }
        }
        if let Some((enabled, end)) = battery.charge_limit {
            column = column.push(section_divider()).push(
                padded_control(
                    widget::Row::new()
                        .push(
                            widget::Column::new()
                                .push(text::body(fl!("charge-limit", percent = end)))
                                .push(text::caption(fl!("charge-limit-description")))
                                .width(Length::Fill),
                        )
                        .push(widget::toggler(enabled).on_toggle(Message::SetChargeLimit))
                        .align_y(Alignment::Center),
                )
                .padding([4, 16]),
            );
        }
        if !battery.peripherals.is_empty() {
            column = column.push(ui::group_label(fl!("connected-devices")));
            for device in &battery.peripherals {
                column = column.push(
                    padded_control(
                        widget::Row::new()
                            .push(
                                icon::from_name(device.icon())
                                    .size(16)
                                    .symbolic(true)
                                    .icon(),
                            )
                            .push(text::body(device.name.clone()).width(Length::Fill))
                            .push(text::body(format!("{:.0}%", device.percentage)))
                            .spacing(12)
                            .align_y(Alignment::Center),
                    )
                    .padding([6, 16]),
                );
            }
        }
        column
            .push(section_divider())
            .push(ui::settings_link(
                fl!("power-settings"),
                Message::OpenSettings("power"),
            ))
            .into()
    }
}

fn meter<'a>(percentage: f64, low: bool) -> Element<'a, Message> {
    let mut bar = widget::progress_bar::determinate_linear((percentage / 100.0) as f32)
        .width(Length::Fill)
        .girth(Length::Fixed(4.0));
    if low {
        bar = bar.bar_color(status_color(false));
    }
    bar.into()
}

fn charge_text(battery: &battery::Battery) -> String {
    let time = battery.seconds_left.map(battery::duration_text);
    match (battery.charge, time) {
        (Some(Charge::Full), _) => fl!("fully-charged"),
        (Some(Charge::NotCharging), _) => fl!("not-charging"),
        (Some(Charge::Charging), Some((h, m))) => fl!("until-full", hours = h, minutes = m),
        (Some(Charge::Charging), None) => fl!("charging"),
        (_, Some((h, m))) => fl!("time-left", hours = h, minutes = m),
        _ if battery.percentage < LOW_PERCENT => fl!("battery-low"),
        _ => fl!("on-battery"),
    }
}

fn profile_name(profile: &str) -> String {
    match profile {
        "power-saver" => fl!("profile-power-saver"),
        "balanced" => fl!("profile-balanced"),
        "performance" => fl!("profile-performance"),
        other => other.to_owned(),
    }
}

fn profile_description(profile: &str) -> String {
    match profile {
        "power-saver" => fl!("profile-power-saver-description"),
        "performance" => fl!("profile-performance-description"),
        _ => fl!("profile-balanced-description"),
    }
}

fn profile_icon(profile: &str) -> &'static str {
    match profile {
        "power-saver" => "power-profile-power-saver-symbolic",
        "performance" => "power-profile-performance-symbolic",
        _ => "power-profile-balanced-symbolic",
    }
}
