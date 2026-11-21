// SPDX-License-Identifier: GPL-3.0-only

//! The Network and Airplane tiles, and the Network page (wired, Wi-Fi, VPN).

use super::{AppModel, Message, Page};
use crate::bluetooth;
use crate::fl;
use crate::network::{self, AccessPointInfo, Command, Network, Primary};
use crate::ui;
use cosmic::applet::padded_control;
use cosmic::iced::Length;
use cosmic::prelude::*;
use cosmic::widget::{self, button, icon, text};
use cosmic_applet_common::widgets::{icon_button, section_divider};

/// A new secured network waiting for its password.
#[derive(Debug, Clone, Default)]
pub struct PasswordPrompt {
    pub ssid: String,
    pub ap: String,
    pub password: String,
}

impl AppModel {
    pub(super) fn network_command(&self, command: Command) {
        if let Some(tx) = &self.network_tx {
            _ = tx.send(command);
        }
    }

    /// Airplane mode: Wi-Fi off and Bluetooth off (or absent).
    pub(super) fn airplane(&self) -> bool {
        let net = &self.network;
        let bt = &self.bluetooth;
        net.has_wifi && !net.wifi_enabled && (!bt.available || !bt.powered)
    }

    pub(super) fn set_airplane(&self, on: bool) {
        self.network_command(Command::Airplane(on));
        if self.bluetooth.available {
            self.bluetooth_command(bluetooth::Command::SetPowered(!on));
        }
    }

    pub(super) fn select_network(&mut self, ap: &AccessPointInfo) {
        if ap.known || !ap.secured {
            self.password_prompt = None;
            self.network_command(Command::ConnectWifi {
                ssid: ap.ssid.clone(),
                ap: ap.path.clone(),
                password: None,
            });
        } else {
            self.password_prompt = Some(PasswordPrompt {
                ssid: ap.ssid.clone(),
                ap: ap.path.clone(),
                password: String::new(),
            });
        }
    }

    pub(super) fn submit_password(&mut self) {
        if let Some(prompt) = self.password_prompt.take() {
            self.network_command(Command::ConnectWifi {
                ssid: prompt.ssid,
                ap: prompt.ap,
                password: Some(prompt.password),
            });
        }
    }

    /// Neutral tile: shows the connection in use; the whole tile opens the Network page.
    pub(super) fn network_tile(&self) -> Option<Element<'_, Message>> {
        let net = &self.network;
        if !net.available {
            return None;
        }
        Some(ui::tile(
            net.icon(),
            fl!("network"),
            network_status(net),
            net.connected(),
            Some(Message::Open(Page::Network)),
            Some(Message::Open(Page::Network)),
        ))
    }

    /// Without a Wi-Fi card the airplane slot is free; a VPN tile takes it.
    pub(super) fn second_network_tile(&self) -> Option<Element<'_, Message>> {
        let net = &self.network;
        if net.has_wifi {
            let on = self.airplane();
            return Some(ui::tile(
                "airplane-mode-symbolic",
                fl!("airplane"),
                if on { fl!("on") } else { fl!("off") },
                on,
                Some(Message::Airplane(!on)),
                None,
            ));
        }
        let vpn = net.vpns.iter().find(|v| v.connected).or(net.vpns.first())?;
        Some(ui::tile(
            "network-vpn-symbolic",
            fl!("vpn"),
            vpn.name.clone(),
            vpn.connected,
            Some(Message::Network(Command::SetVpn(
                vpn.path.clone(),
                !vpn.connected,
            ))),
            Some(Message::Open(Page::Network)),
        ))
    }

    pub(super) fn view_network(&self) -> Element<'_, Message> {
        let net = &self.network;
        let refresh = icon_button("view-refresh-symbolic")
            .on_press_maybe(
                (net.has_wifi && net.wifi_enabled).then_some(Message::Network(Command::Scan)),
            )
            .into();
        let mut column = widget::Column::new().padding([8, 0]).push(ui::page_header(
            fl!("network"),
            None,
            Message::Open(Page::Main),
            vec![refresh],
        ));
        if !net.available {
            return column
                .push(section_divider())
                .push(ui::note(fl!("network-unavailable")))
                .into();
        }
        let both = net.wired.as_ref().is_some_and(|w| w.connected) && net.wifi.is_some();
        if let Some(wired) = &net.wired {
            let subtitle = match (wired.connected, wired.speed) {
                (true, Some(speed)) => fl!("connected-speed", speed = speed_text(speed)),
                (true, None) => fl!("connected"),
                (false, _) => fl!("cable-unplugged"),
            };
            column = column
                .push(section_divider())
                .push(ui::section_title(fl!("wired"), None))
                .push(ui::choice_row(
                    "network-wired-symbolic",
                    fl!("ethernet"),
                    Some(subtitle),
                    wired.connected,
                    in_use_pill(both && wired.in_use),
                    None,
                ));
        }
        if net.has_wifi {
            column = column.push(section_divider()).push(ui::section_title(
                fl!("wifi"),
                Some(
                    widget::toggler(net.wifi_enabled)
                        .on_toggle(|on| Message::Network(Command::SetWifi(on)))
                        .into(),
                ),
            ));
            if net.wifi_enabled {
                if let Some(wifi) = &net.wifi {
                    let subtitle = if both && !wifi.in_use {
                        fl!("connected-standby")
                    } else {
                        fl!("connected-speed", speed = speed_text(wifi.bitrate))
                    };
                    let mut trailing = in_use_pill(both && wifi.in_use);
                    if wifi.secured {
                        trailing.push(lock_icon());
                    }
                    trailing.push(
                        button::text(fl!("disconnect"))
                            .on_press(Message::Network(Command::DisconnectWifi))
                            .into(),
                    );
                    column = column.push(ui::choice_row(
                        network::wifi_icon(wifi.strength),
                        wifi.ssid.clone(),
                        Some(subtitle),
                        true,
                        trailing,
                        None,
                    ));
                }
                if !net.access_points.is_empty() {
                    column = column.push(ui::group_label(fl!("other-networks")));
                }
                for ap in &net.access_points {
                    column = column.push(self.access_point_row(ap));
                }
            }
        }
        if !net.vpns.is_empty() {
            column = column
                .push(section_divider())
                .push(ui::section_title(fl!("vpn"), None));
            for vpn in &net.vpns {
                let subtitle = match (vpn.connected, vpn.active.is_some()) {
                    (true, _) => match net.primary {
                        Primary::Wired => fl!("connected-via", link = fl!("ethernet")),
                        Primary::Wifi => fl!("connected-via", link = fl!("wifi")),
                        _ => fl!("connected"),
                    },
                    (false, true) => fl!("connecting"),
                    (false, false) => fl!("off"),
                };
                let toggle = widget::toggler(vpn.connected || vpn.active.is_some())
                    .on_toggle(|on| Message::Network(Command::SetVpn(vpn.path.clone(), on)))
                    .into();
                column = column.push(ui::choice_row(
                    "network-vpn-symbolic",
                    vpn.name.clone(),
                    Some(subtitle),
                    vpn.connected,
                    vec![toggle],
                    None,
                ));
            }
        }
        column
            .push(section_divider())
            .push(ui::settings_link(
                fl!("network-settings"),
                Message::OpenSettings("network"),
            ))
            .into()
    }

    fn access_point_row<'a>(&'a self, ap: &'a AccessPointInfo) -> Element<'a, Message> {
        if let Some(prompt) = self.password_prompt.as_ref().filter(|p| p.ssid == ap.ssid) {
            let input = widget::secure_input(fl!("password"), prompt.password.clone(), None, true)
                .on_input(Message::PasswordInput)
                .on_submit(|_| Message::PasswordSubmit)
                .width(Length::Fill);
            return padded_control(
                widget::Column::new()
                    .push(text::body(ap.ssid.clone()))
                    .push(input)
                    .push(
                        widget::Row::new()
                            .push(widget::space::horizontal())
                            .push(button::standard(fl!("cancel")).on_press(Message::PasswordCancel))
                            .push(button::suggested(fl!("connect")).on_press_maybe(
                                (prompt.password.len() >= 8).then_some(Message::PasswordSubmit),
                            ))
                            .spacing(8),
                    )
                    .spacing(8),
            )
            .padding([6, 16])
            .into();
        }
        let subtitle = (!ap.secured).then(|| fl!("open-network"));
        let trailing = if ap.secured {
            vec![lock_icon()]
        } else {
            Vec::new()
        };
        ui::choice_row(
            network::wifi_icon(ap.strength),
            ap.ssid.clone(),
            subtitle,
            false,
            trailing,
            Some(Message::SelectNetwork(ap.ssid.clone())),
        )
    }
}

/// "Home-5G", "Ethernet", "Disconnected", "Off", plus " · VPN".
fn network_status(net: &Network) -> String {
    let base = match (net.primary, &net.wifi) {
        (Primary::Wired, _) => fl!("ethernet"),
        (Primary::Wifi, Some(wifi)) => wifi.ssid.clone(),
        (Primary::Other, _) => fl!("connected"),
        _ if net.has_wifi && !net.wifi_enabled && net.wired.is_none() => fl!("off"),
        _ => fl!("disconnected"),
    };
    if net.vpn_connected() {
        format!("{base} · {}", fl!("vpn"))
    } else {
        base
    }
}

fn speed_text(mbps: u32) -> String {
    if mbps >= 1000 && mbps.is_multiple_of(1000) {
        format!("{} Gb/s", mbps / 1000)
    } else {
        format!("{mbps} Mb/s")
    }
}

fn in_use_pill<'a>(show: bool) -> Vec<Element<'a, Message>> {
    if show {
        vec![
            text::caption_heading(fl!("in-use"))
                .class(cosmic::theme::Text::Accent)
                .into(),
        ]
    } else {
        Vec::new()
    }
}

fn lock_icon<'a>() -> Element<'a, Message> {
    icon::from_name("network-wireless-encrypted-symbolic")
        .size(16)
        .symbolic(true)
        .icon()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{ActiveWifi, Vpn};

    #[test]
    fn status_names_the_link_in_use_and_vpn() {
        let mut net = Network {
            available: true,
            has_wifi: true,
            wifi_enabled: true,
            primary: Primary::Wifi,
            wifi: Some(ActiveWifi {
                ssid: "Home-5G".into(),
                strength: 80,
                secured: true,
                bitrate: 866,
                in_use: true,
            }),
            ..Network::default()
        };
        assert_eq!(network_status(&net), "Home-5G");
        net.primary = Primary::Wired;
        net.vpns.push(Vpn {
            name: "Office".into(),
            path: "/s/1".into(),
            active: Some("/a/1".into()),
            connected: true,
        });
        assert_eq!(network_status(&net), "Ethernet · VPN");
        net.primary = Primary::None;
        net.wifi = None;
        net.vpns.clear();
        net.wifi_enabled = false;
        assert_eq!(network_status(&net), "Off");
    }

    #[test]
    fn speeds() {
        assert_eq!(speed_text(1000), "1 Gb/s");
        assert_eq!(speed_text(866), "866 Mb/s");
    }
}
