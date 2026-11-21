// SPDX-License-Identifier: GPL-3.0-only

//! The applet's own settings: popup sections and their order, panel icons, alerts, and
//! shutdown confirmation.

use super::{AppModel, Message, Page};
use crate::config::{Config, PanelIcons, Section, SectionEntry};
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
    Move(usize, bool),
    Enable(usize, bool),
    Icon(PanelIcon, bool),
    BatteryPercentage(bool),
    ShowAlerts(bool),
    ConfirmShutdown(bool),
}

/// Moves the entry at `index` one up (or down); no-op at the ends.
pub fn move_section(sections: &mut [SectionEntry], index: usize, up: bool) {
    let other = if up {
        index.checked_sub(1)
    } else {
        Some(index + 1)
    };
    if let Some(other) = other.filter(|o| *o < sections.len())
        && index < sections.len()
    {
        sections.swap(index, other);
    }
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
            SettingsMessage::Move(index, up) => {
                let mut sections = self.normalized_sections();
                move_section(&mut sections, index, up);
                self.save(|c, ctx| c.set_sections(ctx, sections));
            }
            SettingsMessage::Enable(index, enabled) => {
                let mut sections = self.normalized_sections();
                if let Some(entry) = sections.get_mut(index) {
                    entry.enabled = enabled;
                }
                self.save(|c, ctx| c.set_sections(ctx, sections));
            }
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

    /// Every section exactly once, in the configured order, so moving works on old configs too.
    fn normalized_sections(&self) -> Vec<SectionEntry> {
        let mut out: Vec<SectionEntry> = Vec::new();
        for entry in &self.config.sections {
            if !out.iter().any(|e| e.section == entry.section) {
                out.push(*entry);
            }
        }
        for section in Section::ALL {
            if !out.iter().any(|e| e.section == section) {
                out.push(SectionEntry {
                    section,
                    enabled: true,
                });
            }
        }
        out
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
            .push(ui::group_label(fl!("sections-in-order")));
        let sections = self.normalized_sections();
        let last = sections.len().saturating_sub(1);
        for (index, entry) in sections.iter().enumerate() {
            let arrow = |name: &'static str, enabled: bool, up: bool| {
                button::icon(icon::from_name(name))
                    .extra_small()
                    .on_press_maybe(
                        enabled.then_some(Message::Settings(SettingsMessage::Move(index, up))),
                    )
            };
            column = column.push(
                padded_control(
                    widget::Row::new()
                        .push(
                            icon::from_name(section_icon(entry.section))
                                .size(16)
                                .symbolic(true)
                                .icon(),
                        )
                        .push(text::body(section_name(entry.section)).width(Length::Fill))
                        .push(arrow("go-up-symbolic", index > 0, true))
                        .push(arrow("go-down-symbolic", index < last, false))
                        .push(widget::toggler(entry.enabled).on_toggle(move |on| {
                            Message::Settings(SettingsMessage::Enable(index, on))
                        }))
                        .spacing(8)
                        .align_y(Alignment::Center),
                )
                .padding([4, 16]),
            );
        }
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

fn section_name(section: Section) -> String {
    match section {
        Section::Battery => fl!("section-battery"),
        Section::Volume => fl!("volume"),
        Section::Microphone => fl!("microphone"),
        Section::Brightness => fl!("brightness"),
        Section::Network => fl!("network"),
        Section::Bluetooth => fl!("bluetooth"),
        Section::Media => fl!("media-controls"),
        Section::PowerButtons => fl!("power-buttons"),
    }
}

fn section_icon(section: Section) -> &'static str {
    match section {
        Section::Battery => "battery-level-80-symbolic",
        Section::Volume => "audio-volume-high-symbolic",
        Section::Microphone => "microphone-sensitivity-high-symbolic",
        Section::Brightness => "display-brightness-symbolic",
        Section::Network => "network-wireless-signal-excellent-symbolic",
        Section::Bluetooth => "bluetooth-active-symbolic",
        Section::Media => "media-playback-start-symbolic",
        Section::PowerButtons => "system-shutdown-symbolic",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<SectionEntry> {
        Section::ALL
            .iter()
            .map(|&section| SectionEntry {
                section,
                enabled: true,
            })
            .collect()
    }

    #[test]
    fn moving_sections() {
        let mut sections = entries();
        move_section(&mut sections, 1, true);
        assert_eq!(sections[0].section, Section::Volume);
        move_section(&mut sections, 0, true);
        assert_eq!(sections[0].section, Section::Volume, "top stays put");
        let last = sections.len() - 1;
        move_section(&mut sections, last, false);
        assert_eq!(
            sections[last].section,
            Section::PowerButtons,
            "bottom stays put"
        );
    }
}
