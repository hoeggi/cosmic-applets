// SPDX-License-Identifier: MPL-2.0

use crate::config::Config;
use crate::fl;
use crate::hsc::{self, BatteryState, Device, HscError};
use crate::icon::{self, PanelState};
use crate::settings::{self, Kind, SETTINGS, Setting};
use cosmic::applet::padded_control;
use cosmic::cosmic_config;
use cosmic::iced::{Alignment, Length, Subscription, window::Id};
use cosmic::prelude::*;
use cosmic::widget::{self, button, text};
use cosmic_applet_common::widgets::{error_text, section_divider};
use cosmic_applet_common::{config as cfg, notify, popup};
use std::collections::HashMap;
use std::time::Duration;

pub const APP_ID: &str = "at.hoeggi.CosmicAppletHeadsetControl";

/// Poll interval while the popup is open.
const POPUP_POLL: Duration = Duration::from_secs(5);

#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    config: Config,
    config_ctx: Option<cosmic_config::Config>,
    /// Last successful status read; `None` when no headset (dongle) is present.
    device: Option<Device>,
    /// The CLI could not be run or its output not understood.
    cli_error: Option<HscError>,
    /// Online state from the previous poll; starts `false` so a headset that is
    /// already on when the applet starts counts as (re)connected.
    was_online: bool,
    polling: bool,
    /// Slider values while dragging, committed on release.
    dragging: HashMap<&'static str, i32>,
    /// Last error from applying a setting.
    apply_error: Option<String>,
    low_battery_notified: bool,
    panel_icon: Option<(PanelState, cosmic::widget::icon::Handle)>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    UpdateConfig(Config),
    Poll,
    Polled(Result<Option<Box<Device>>, HscError>),
    Drag(&'static str, i32),
    Commit(&'static str),
    Set(&'static str, i32),
    Applied {
        device_key: String,
        values: Vec<(&'static str, i32)>,
        result: Result<(), String>,
    },
    ShowWriteOnly(bool),
    ReapplyOnReconnect(bool),
    Noop,
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let (mut config, config_ctx) = cfg::load::<Config>(Self::APP_ID);
        if std::env::var_os("HSC_TEST_DEVICE").is_some_and(|v| v != "0") {
            config.test_device = true;
        }
        tracing::info!(test_device = config.test_device, "applet started");

        let mut app = AppModel {
            core,
            config,
            config_ctx,
            ..Default::default()
        };
        app.update_panel_icon();
        let task = app.poll();
        (app, task)
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    /// The button shown in the panel.
    fn view(&self) -> Element<'_, Self::Message> {
        let handle = self.panel_icon.as_ref().map_or_else(
            || icon::handle(PanelState::Offline),
            |(_, handle)| handle.clone(),
        );
        self.core
            .applet
            .icon_button_from_handle(handle)
            .on_press_down(Message::TogglePopup)
            .into()
    }

    /// The popup.
    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        self.core.applet.popup_container(self.view_popup()).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let interval = if self.popup.is_some() {
            POPUP_POLL
        } else {
            Duration::from_secs(self.config.poll_secs.max(5))
        };
        Subscription::batch([
            cfg::watch::<Config>(self.core(), Self::APP_ID).map(Message::UpdateConfig),
            cosmic::iced::time::every(interval).map(|_| Message::Poll),
        ])
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::TogglePopup => {
                let task = popup::toggle(&self.core, &mut self.popup);
                if self.popup.is_none() {
                    return task;
                }
                self.apply_error = None;
                return Task::batch([task, self.poll()]);
            }
            Message::PopupClosed(id) => popup::closed(&mut self.popup, id),
            Message::UpdateConfig(config) => {
                let test_device = self.config.test_device;
                self.config = config;
                self.config.test_device |= test_device;
            }
            Message::Poll => return self.poll(),
            Message::Polled(result) => return self.polled(result),
            Message::Drag(key, value) => {
                self.dragging.insert(key, value);
            }
            Message::Commit(key) => {
                if let Some(value) = self.dragging.get(key).copied() {
                    return self.apply(vec![(key, value)]);
                }
            }
            Message::Set(key, value) => return self.apply(vec![(key, value)]),
            Message::Applied {
                device_key,
                values,
                result,
            } => self.applied(&device_key, values, result),
            Message::ShowWriteOnly(on) => {
                self.save(|config, ctx| config.set_show_write_only(ctx, on));
            }
            Message::ReapplyOnReconnect(on) => {
                self.save(|config, ctx| config.set_reapply_on_reconnect(ctx, on));
            }
            Message::Noop => {}
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl AppModel {
    fn poll(&mut self) -> Task<cosmic::Action<Message>> {
        if self.polling {
            return Task::none();
        }
        self.polling = true;
        let extra: Vec<String> = self
            .device
            .as_ref()
            .map(|device| {
                SETTINGS
                    .iter()
                    .filter(|s| s.readable(device))
                    .filter_map(|s| s.read_arg.map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let test_device = self.config.test_device;
        cosmic::task::future(async move {
            Message::Polled(
                hsc::query(extra, test_device)
                    .await
                    .map(|d| d.map(Box::new)),
            )
        })
    }

    fn polled(
        &mut self,
        result: Result<Option<Box<Device>>, HscError>,
    ) -> Task<cosmic::Action<Message>> {
        self.polling = false;
        let mut tasks = Vec::new();
        match result {
            Ok(device) => {
                let device = device.map(|d| *d);
                self.cli_error = None;
                let online = device.as_ref().is_some_and(Device::is_online);
                if online && !self.was_online {
                    tracing::info!(device = ?device.as_ref().map(|d| &d.device), "headset online");
                    // A newly seen device may have readable settings we did not query yet.
                    if self.device.as_ref().map(Device::key) != device.as_ref().map(Device::key) {
                        tasks.push(cosmic::task::message(cosmic::Action::App(Message::Poll)));
                    }
                }
                if !online && self.was_online {
                    tracing::info!("headset offline");
                }
                let reconnected = online && !self.was_online;
                self.was_online = online;
                self.device = device;
                if reconnected && self.config.reapply_on_reconnect {
                    tasks.push(self.reapply());
                }
                tasks.push(self.check_low_battery());
            }
            Err(err) => {
                if self.cli_error.as_ref() != Some(&err) {
                    tracing::warn!(%err, "status query failed");
                }
                self.cli_error = Some(err);
            }
        }
        self.update_panel_icon();
        Task::batch(tasks)
    }

    /// Resends remembered values of write-only settings in one call.
    fn reapply(&self) -> Task<cosmic::Action<Message>> {
        let Some(device) = &self.device else {
            return Task::none();
        };
        let Some(stored) = self.config.values(&device.key()) else {
            return Task::none();
        };
        let values: Vec<_> = SETTINGS
            .iter()
            .filter(|s| s.supported(device) && !s.readable(device))
            .filter_map(|s| stored.get(s.key).map(|v| (s.key, *v)))
            .collect();
        if values.is_empty() {
            return Task::none();
        }
        tracing::info!(?values, "re-applying settings");
        self.spawn_apply(device.key(), values)
    }

    fn apply(&mut self, values: Vec<(&'static str, i32)>) -> Task<cosmic::Action<Message>> {
        let Some(device) = &self.device else {
            return Task::none();
        };
        tracing::info!(?values, "apply");
        self.spawn_apply(device.key(), values)
    }

    fn spawn_apply(
        &self,
        device_key: String,
        values: Vec<(&'static str, i32)>,
    ) -> Task<cosmic::Action<Message>> {
        let args: Vec<String> = values
            .iter()
            .filter_map(|(key, value)| settings::get(key).map(|s| s.args(*value)))
            .flatten()
            .collect();
        let test_device = self.config.test_device;
        cosmic::task::future(async move {
            let result = hsc::apply(args, test_device).await;
            Message::Applied {
                device_key,
                values,
                result,
            }
        })
    }

    fn applied(
        &mut self,
        device_key: &str,
        values: Vec<(&'static str, i32)>,
        result: Result<(), String>,
    ) {
        for (key, _) in &values {
            self.dragging.remove(key);
        }
        match result {
            Ok(()) => {
                self.apply_error = None;
                let mut stored = self.config.stored.clone();
                let entry = stored.entry(device_key.to_owned()).or_default();
                for (key, value) in values {
                    entry.insert(key.to_owned(), value);
                }
                // Readable values show the new state right away instead of after the next poll.
                if let Some(device) = &mut self.device
                    && let Some(&level) = entry.get("sidetone")
                    && let Some(sidetone) = &mut device.sidetone
                {
                    sidetone.level = level;
                }
                self.save(|config, ctx| config.set_stored(ctx, stored));
            }
            Err(err) => {
                tracing::warn!(%err, ?values, "applying settings failed");
                self.apply_error = Some(err);
            }
        }
    }

    fn check_low_battery(&mut self) -> Task<cosmic::Action<Message>> {
        let threshold = self.config.low_battery_percent;
        match self.device.as_ref().and_then(Device::battery_state) {
            Some(BatteryState::Available(level)) if threshold > 0 && level <= threshold => {
                if !self.low_battery_notified {
                    self.low_battery_notified = true;
                    let name = self
                        .device
                        .as_ref()
                        .map(|d| d.device.clone())
                        .unwrap_or_default();
                    return cosmic::task::future(async move {
                        notify_low_battery(name, level).await;
                        Message::Noop
                    });
                }
            }
            Some(BatteryState::Available(_) | BatteryState::Charging(_)) => {
                self.low_battery_notified = false;
            }
            _ => {}
        }
        Task::none()
    }

    /// Writes one config field through cosmic-config (also updates `self.config`).
    fn save(
        &mut self,
        set: impl FnOnce(&mut Config, &cosmic_config::Config) -> Result<bool, cosmic_config::Error>,
    ) {
        cfg::save(&mut self.config, self.config_ctx.as_ref(), set);
    }

    /// Rebuilds the panel icon only when what it shows changes (avoids re-rasterising).
    fn update_panel_icon(&mut self) {
        let state = if self.cli_error.is_some() {
            PanelState::Error
        } else {
            match self.device.as_ref().and_then(Device::battery_state) {
                Some(BatteryState::Available(level)) => PanelState::Level(level),
                Some(BatteryState::Charging(level)) => PanelState::Charging(level),
                _ => PanelState::Offline,
            }
        };
        if self.panel_icon.as_ref().map(|(s, _)| *s) != Some(state) {
            self.panel_icon = Some((state, icon::handle(state)));
        }
    }

    fn view_popup(&self) -> Element<'_, Message> {
        let mut col = widget::Column::new().padding([8, 0]);

        let Some(device) = &self.device else {
            let msg = match &self.cli_error {
                Some(HscError::NotInstalled) => fl!("not-installed"),
                Some(err) => err.to_string(),
                None if self.polling => fl!("searching"),
                None => fl!("no-headset"),
            };
            return col.push(padded_control(text::body(msg))).into();
        };

        col = col.push(padded_control(self.view_status(device)));
        if let Some(err) = &self.cli_error {
            col = col.push(padded_control(error_text(err.to_string())));
        }

        let online = device.is_online();
        let supported: Vec<&Setting> = SETTINGS.iter().filter(|s| s.supported(device)).collect();
        let readable: Vec<_> = supported.iter().filter(|s| s.readable(device)).collect();
        let write_only: Vec<_> = supported.iter().filter(|s| !s.readable(device)).collect();
        let stored = self.config.values(&device.key());

        if !readable.is_empty() {
            col = col.push(section_divider());
            for setting in readable {
                let value = setting.read(device);
                col = col.push(padded_control(
                    self.view_setting(setting, value, device, online),
                ));
            }
        }

        if self.config.show_write_only && !write_only.is_empty() {
            col = col.push(section_divider()).push(padded_control(
                widget::Column::new()
                    .push(text::heading(fl!("write-only")))
                    .push(text::caption(fl!("write-only-hint"))),
            ));
            for setting in write_only {
                let value = stored.and_then(|s| s.get(setting.key)).copied();
                col = col.push(padded_control(
                    self.view_setting(setting, value, device, online),
                ));
            }
        }

        if let Some(err) = &self.apply_error {
            col = col.push(padded_control(error_text(err.clone())));
        }

        col.push(section_divider())
            .push(padded_control(toggle_row(
                fl!("show-write-only"),
                self.config.show_write_only,
                Message::ShowWriteOnly,
            )))
            .push(padded_control(toggle_row(
                fl!("reapply-on-reconnect"),
                self.config.reapply_on_reconnect,
                Message::ReapplyOnReconnect,
            )))
            .into()
    }

    fn view_status<'a>(&'a self, device: &'a Device) -> Element<'a, Message> {
        let battery = match device.battery_state() {
            Some(BatteryState::Available(level)) => fl!("battery", level = level),
            Some(BatteryState::Charging(Some(level))) => fl!("charging", level = level),
            Some(BatteryState::Charging(None)) => fl!("charging-unknown"),
            Some(BatteryState::Unavailable) => fl!("offline"),
            None => String::new(),
        };
        let mut col = widget::Column::new()
            .push(
                widget::Row::new()
                    .push(text::heading(&device.device).width(Length::Fill))
                    .push(text::body(battery))
                    .spacing(8)
                    .align_y(Alignment::Center),
            )
            .spacing(4);

        if device.has(hsc::CAP_CHATMIX)
            && let Some(chatmix) = device.chatmix
        {
            // 0 = all game, 64 = balanced, 128 = all chat.
            col = col.push(
                widget::Row::new()
                    .push(text::caption(fl!("chatmix")).width(Length::Fill))
                    .push(text::caption(fl!("game")))
                    .push(
                        widget::determinate_linear(chatmix as f32 / 128.0)
                            .width(Length::Fixed(120.0))
                            .girth(6.0),
                    )
                    .push(text::caption(fl!("chat")))
                    .spacing(8)
                    .align_y(Alignment::Center),
            );
        }
        col.into()
    }

    fn view_setting(
        &self,
        setting: &'static Setting,
        value: Option<i32>,
        device: &Device,
        enabled: bool,
    ) -> Element<'_, Message> {
        let key = setting.key;
        let label = setting_label(key);
        let unset = || text::caption(fl!("not-set"));

        match setting.kind {
            Kind::Toggle => {
                let on = value.unwrap_or(0) != 0;
                let mut row = widget::Row::new()
                    .push(text::body(label).width(Length::Fill))
                    .spacing(8)
                    .align_y(Alignment::Center);
                if value.is_none() {
                    row = row.push(unset());
                }
                row.push(
                    widget::toggler(on).on_toggle_maybe(
                        enabled.then_some(move |on| Message::Set(key, i32::from(on))),
                    ),
                )
                .into()
            }
            Kind::Slider { min, max } => {
                let current = self.dragging.get(key).copied().or(value);
                let shown = current.map_or_else(|| fl!("not-set"), |v| v.to_string());
                let mut slider = widget::slider(min..=max, current.unwrap_or(min), move |v| {
                    Message::Drag(key, v)
                });
                if enabled {
                    slider = slider.on_release(Message::Commit(key));
                }
                widget::Column::new()
                    .push(
                        widget::Row::new()
                            .push(text::body(label).width(Length::Fill))
                            .push(text::caption(shown)),
                    )
                    .push(slider)
                    .spacing(4)
                    .into()
            }
            Kind::Choice(options) => choice_row(
                label,
                options.iter().map(|o| choice_label(key, o)).collect(),
                value,
                key,
                enabled,
            ),
            Kind::EqPreset => choice_row(label, device.preset_names(), value, key, enabled),
        }
    }
}

fn setting_label(key: &str) -> String {
    crate::i18n::LANGUAGE_LOADER.get(&format!("setting-{key}"))
}

fn choice_label(key: &str, option: &str) -> String {
    let id = format!("choice-{key}-{option}");
    if crate::i18n::LANGUAGE_LOADER.has(&id) {
        crate::i18n::LANGUAGE_LOADER.get(&id)
    } else {
        option.to_owned()
    }
}

fn choice_row(
    label: String,
    options: Vec<String>,
    value: Option<i32>,
    key: &'static str,
    enabled: bool,
) -> Element<'static, Message> {
    let mut buttons = widget::Row::new().spacing(4);
    for (idx, option) in options.into_iter().enumerate() {
        let idx = i32::try_from(idx).unwrap_or(i32::MAX);
        let msg = enabled.then_some(Message::Set(key, idx));
        buttons = buttons.push(if value == Some(idx) {
            button::suggested(option).on_press_maybe(msg)
        } else {
            button::standard(option).on_press_maybe(msg)
        });
    }
    let mut header = widget::Row::new().push(text::body(label).width(Length::Fill));
    if value.is_none() {
        header = header.push(text::caption(fl!("not-set")));
    }
    widget::Column::new()
        .push(header)
        .push(widget::scrollable::horizontal(buttons))
        .spacing(4)
        .into()
}

fn toggle_row(label: String, on: bool, msg: fn(bool) -> Message) -> Element<'static, Message> {
    widget::Row::new()
        .push(text::body(label).width(Length::Fill))
        .push(widget::toggler(on).on_toggle(msg))
        .align_y(Alignment::Center)
        .into()
}

async fn notify_low_battery(name: String, level: u8) {
    let summary = fl!("low-battery", level = level);
    notify::send("HeadsetControl", "battery-level-10-symbolic", summary, name).await;
}
