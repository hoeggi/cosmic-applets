// SPDX-License-Identifier: GPL-3.0-only

use crate::adapter::audio;
use crate::adapter::battery as battery_backend;
use crate::adapter::bluetooth as bt;
use crate::adapter::media as mpris;
use crate::adapter::network as net;
use crate::config::{Config, Slider, Tile};
use crate::fl;
use crate::panel;
use crate::power::PowerAction;
use crate::ui;
use cosmic::applet::padded_control;
use cosmic::cosmic_config;
use cosmic::iced::mouse::ScrollDelta;
use cosmic::iced::{Alignment, Length, Subscription, window::Id};
use cosmic::prelude::*;
use cosmic::widget::{self, button, icon, text};
use cosmic_applet_common::widgets::{section_divider, status_color};
use cosmic_applet_common::{config as cfg, popup};
use std::time::{Duration, Instant};

mod battery;
mod bluetooth;
mod media;
mod network;
mod settings;
mod sound;

pub const APP_ID: &str = "at.hoeggi.CosmicAppletSystem";

/// Popup height limit; the compositor slides and flips popups but doesn't shrink them.
const POPUP_MAX_HEIGHT: f32 = 900.0;
/// Gap between the panel icons in a horizontal panel.
const PANEL_SPACING_HORIZONTAL: u16 = 8;
/// Gap between the panel icons in a vertical panel.
const PANEL_SPACING_VERTICAL: u16 = 4;
/// Restart, shut down and log out happen by themselves after this long.
const CONFIRM_SECONDS: u64 = 60;

/// The page the popup shows. Detail pages are opened by the chevrons on the main page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Main,
    Battery,
    Sound,
    Network,
    Bluetooth,
    /// The applet's own settings.
    Settings,
    /// Asks before a session-ending power action, with a countdown.
    Confirm(PowerAction),
}

#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    page: Page,
    config: Config,
    config_ctx: Option<cosmic_config::Config>,
    /// When the action on the confirmation page runs by itself.
    confirm_deadline: Option<Instant>,
    /// Redraw tick for the countdown.
    now: Option<Instant>,
    audio: audio::Model,
    sink_volume: audio::Delayed,
    source_volume: audio::Delayed,
    /// Every running media player.
    media: mpris::Model,
    /// Upstream's battery, profile and brightness state; `battery` is our view of it.
    battery_model: battery_backend::Model,
    battery: battery_backend::Battery,
    /// Upstream's Bluetooth state and request sender; `bluetooth` is our view of it.
    bluetooth_model: bt::Model,
    bluetooth: bt::Bluetooth,
    network: net::Network,
    network_tx: Option<tokio::sync::mpsc::UnboundedSender<net::Command>>,
    password_prompt: Option<network::PasswordPrompt>,
    /// A VPN went down without being switched off here; cleared when one connects.
    vpn_dropped: bool,
    /// VPNs switched off here, so their going down isn't an alert.
    vpn_switched_off: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    UpdateConfig(Config),
    Open(Page),
    /// Right click on the panel icon, or the link at the bottom of the main popup.
    OpenAppletSettings,
    /// Opens COSMIC Settings, on a page such as "sound" or "network" (empty: its start page).
    OpenSettings(&'static str),
    Power(PowerAction),
    ConfirmPower,
    CancelPower,
    Tick(Instant),
    PowerDone(Result<(), String>),
    Audio(audio::Update),
    SinkVolume(u32),
    SourceVolume(u32),
    /// Scroll on the panel icon: positive is up.
    ScrollVolume(f32),
    ToggleSinkMute,
    ToggleSourceMute,
    SetDefault(u32),
    BatteryEvent(battery_backend::Event),
    Media(crate::upstream::audio::mpris_subscription::MprisUpdate),
    /// A player's web cover and app icon, looked up by the adapter.
    MediaExtras(String, mpris::Extras),
    /// A request for one player, by its bus name.
    MediaRequest(
        String,
        crate::upstream::audio::mpris_subscription::MprisRequest,
    ),
    Brightness(u32),
    BrightnessDebounced,
    BrightnessReleased,
    SetProfile(crate::upstream::battery::Power),
    SetChargeLimit(bool),
    BluetoothUpdate(crate::upstream::bluetooth::BluerEvent),
    Bluetooth(bt::Command),
    NetworkUpdate(net::Update),
    Network(net::Command),
    SelectNetwork(String),
    PasswordInput(String),
    PasswordSubmit,
    PasswordCancel,
    Airplane(bool),
    Settings(settings::SettingsMessage),
    /// A background task finished; nothing to do.
    Done,
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
        let (config, config_ctx) = cfg::load::<Config>(Self::APP_ID);
        tracing::info!("applet started");
        let app = AppModel {
            core,
            config,
            config_ctx,
            ..Default::default()
        };
        // As upstream's battery applet: the session bus (for brightness) and the charge limit.
        let tasks = Task::batch([
            Task::perform(battery_backend::connect_session(), |event| {
                cosmic::Action::App(event.map_or(Message::Done, Message::BatteryEvent))
            }),
            Task::perform(battery_backend::read_charge_limit(), |event| {
                cosmic::Action::App(Message::BatteryEvent(event))
            }),
        ]);
        (app, tasks)
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&self) -> Element<'_, Self::Message> {
        self.view_panel()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let content = match self.page {
            Page::Main => self.view_main(),
            Page::Battery => self.view_battery(),
            Page::Sound => self.view_sound(),
            Page::Network => self.view_network(),
            Page::Bluetooth => self.view_bluetooth(),
            Page::Settings => self.view_settings(),
            Page::Confirm(action) => self.view_confirm(action),
        };
        let body = widget::container(widget::scrollable(content)).max_height(POPUP_MAX_HEIGHT);
        self.core.applet.popup_container(body).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subs = vec![
            cfg::watch::<Config>(self.core(), Self::APP_ID).map(Message::UpdateConfig),
            audio::subscription().map(Message::Audio),
            battery_backend::subscription(self.battery_model.session.as_ref())
                .map(Message::BatteryEvent),
            mpris::subscription().map(Message::Media),
            bt::subscription().map(Message::BluetoothUpdate),
            net::subscription().map(Message::NetworkUpdate),
        ];
        if self.confirm_deadline.is_some() {
            subs.push(cosmic::iced::time::every(Duration::from_secs(1)).map(Message::Tick));
        }
        Subscription::batch(subs)
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::TogglePopup => {
                let task =
                    popup::toggle_with_max_height(&self.core, &mut self.popup, POPUP_MAX_HEIGHT);
                self.set_page(Page::Main);
                self.confirm_deadline = None;
                return task;
            }
            Message::PopupClosed(id) => {
                popup::closed(&mut self.popup, id);
                if self.popup.is_none() {
                    // Closing the popup cancels a pending restart or shutdown.
                    self.confirm_deadline = None;
                    self.set_page(Page::Main);
                }
            }
            Message::UpdateConfig(config) => self.config = config,
            Message::Open(page) => self.set_page(page),
            Message::OpenAppletSettings => {
                self.set_page(Page::Settings);
                if self.popup.is_none() {
                    return popup::toggle_with_max_height(
                        &self.core,
                        &mut self.popup,
                        POPUP_MAX_HEIGHT,
                    );
                }
            }
            Message::OpenSettings(page) => {
                open_cosmic_settings(page);
                return self.close_popup();
            }
            Message::Power(action) => {
                if action.needs_confirmation() && self.config.confirm_shutdown {
                    self.page = Page::Confirm(action);
                    let now = Instant::now();
                    self.now = Some(now);
                    self.confirm_deadline = Some(now + Duration::from_secs(CONFIRM_SECONDS));
                } else {
                    return self.perform(action);
                }
            }
            Message::ConfirmPower => {
                if let Page::Confirm(action) = self.page {
                    return self.perform(action);
                }
            }
            Message::CancelPower => {
                self.confirm_deadline = None;
                self.set_page(Page::Main);
            }
            Message::Tick(now) => {
                self.now = Some(now);
                if let (Page::Confirm(action), Some(deadline)) = (self.page, self.confirm_deadline)
                    && now >= deadline
                {
                    return self.perform(action);
                }
            }
            Message::PowerDone(result) => {
                if let Err(err) = result {
                    tracing::error!(%err, "power action failed");
                }
            }
            Message::Audio(update) => self.audio.update(update),
            Message::SinkVolume(volume) => return self.set_sink_volume(volume),
            Message::SourceVolume(volume) => return self.set_source_volume(volume),
            Message::ScrollVolume(delta) => {
                // Upstream: 5 % per wheel event, at most one every 192 ms.
                let step = if delta > 0.0 { 5.0 } else { -5.0 };
                if self.audio.sink().is_some()
                    && let Some(volume) = self.audio.wheel(step, Instant::now())
                {
                    return audio_task(audio::Command::SinkVolumeNow(volume));
                }
            }
            Message::ToggleSinkMute => return audio_task(audio::Command::ToggleSinkMute),
            Message::ToggleSourceMute => return audio_task(audio::Command::ToggleSourceMute),
            Message::SetDefault(id) => return audio_task(audio::Command::SetDefault(id)),
            Message::Media(update) => {
                self.media.update(update);
                let lookups = self.media.lookups().into_iter().map(|l| {
                    let id = l.id;
                    Task::perform(mpris::lookup(l.player, l.previous), move |extras| {
                        cosmic::Action::App(Message::MediaExtras(id.clone(), extras))
                    })
                });
                return Task::batch(lookups);
            }
            Message::MediaExtras(id, extras) => self.media.set_extras(id, extras),
            Message::MediaRequest(id, request) => self.media.request(&id, request),
            Message::BatteryEvent(event) => {
                self.battery_model.event(event);
                self.battery = self.battery_model.view();
            }
            Message::Brightness(percent) => return self.set_brightness(percent),
            Message::BrightnessDebounced => return self.brightness_debounced(),
            Message::BrightnessReleased => self.battery_model.release_brightness(),
            Message::SetProfile(profile) => self.battery_model.set_profile(profile),
            Message::SetChargeLimit(enabled) => {
                self.battery_model.set_charge_limit(enabled);
                self.battery = self.battery_model.view();
                return Task::perform(battery_backend::write_charge_limit(enabled), |()| {
                    cosmic::Action::App(Message::Done)
                });
            }
            Message::BluetoothUpdate(event) => {
                self.bluetooth_model.event(event);
                self.bluetooth = self.bluetooth_model.view();
            }
            Message::Bluetooth(command) => self.bluetooth_command(command),
            Message::NetworkUpdate(net::Update::Ready(tx)) => self.network_tx = Some(tx),
            Message::NetworkUpdate(net::Update::State(state)) => self.network_changed(*state),
            Message::Network(command) => {
                if let net::Command::SetVpn(uuid, false) = &command {
                    self.vpn_switched_off.push(uuid.clone());
                }
                self.vpn_dropped = false;
                self.network_command(command);
            }
            Message::Settings(message) => self.update_settings(message),
            Message::SelectNetwork(ssid) => {
                if let Some(ap) = self
                    .network
                    .access_points
                    .iter()
                    .find(|a| a.ssid == ssid)
                    .cloned()
                {
                    self.select_network(&ap);
                }
            }
            Message::PasswordInput(password) => {
                if let Some(prompt) = &mut self.password_prompt {
                    prompt.password = password;
                }
            }
            Message::PasswordSubmit => self.submit_password(),
            Message::PasswordCancel => self.password_prompt = None,
            Message::Airplane(on) => self.set_airplane(on),
            Message::Done => {}
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

/// Starts COSMIC Settings, on `page` if given.
fn open_cosmic_settings(page: &str) {
    let mut cmd = std::process::Command::new("cosmic-settings");
    if !page.is_empty() {
        cmd.arg(page);
    }
    if let Err(err) = cmd.spawn() {
        tracing::warn!(%err, "cannot start cosmic-settings");
    }
}

impl AppModel {
    /// Switches pages; the Bluetooth page scans while it is open.
    fn set_page(&mut self, page: Page) {
        if self.page == Page::Bluetooth && page != Page::Bluetooth {
            self.bluetooth_command(bt::Command::Scan(false));
        } else if page == Page::Bluetooth && self.page != Page::Bluetooth && self.bluetooth.powered
        {
            self.bluetooth_command(bt::Command::Scan(true));
        }
        if page == Page::Network && self.page != Page::Network {
            self.network_command(net::Command::Scan);
        }
        if page != Page::Network {
            self.password_prompt = None;
        }
        self.page = page;
    }

    fn network_changed(&mut self, state: net::Network) {
        for before in self.network.vpns.iter().filter(|v| v.connected) {
            let still_up = state
                .vpns
                .iter()
                .any(|v| v.uuid == before.uuid && v.connected);
            if !still_up && !self.vpn_switched_off.contains(&before.uuid) {
                tracing::warn!(vpn = before.name, "VPN dropped");
                self.vpn_dropped = true;
            }
        }
        if state.vpn_connected() {
            self.vpn_dropped = false;
        }
        self.vpn_switched_off
            .retain(|uuid| state.vpns.iter().any(|v| &v.uuid == uuid && v.connected));
        self.network = state;
    }

    fn panel_state(&self) -> panel::State {
        let battery = &self.battery;
        panel::State {
            network: self.network.available.then(|| self.network.icon()),
            network_connected: self.network.connected(),
            vpn_connected: self.network.vpn_connected(),
            vpn_dropped: self.vpn_dropped,
            bluetooth: self.bluetooth.available.then_some(self.bluetooth.powered),
            bluetooth_lowest: self.bluetooth.connected().filter_map(|d| d.battery).min(),
            volume: self
                .audio
                .sink()
                .map(|s| (audio::volume_icon(s.volume, s.mute), s.mute)),
            battery: battery
                .present
                .then(|| (battery.icon.clone(), battery.percentage, battery.is_low())),
        }
    }

    fn close_popup(&mut self) -> Task<cosmic::Action<Message>> {
        if self.popup.is_some() {
            return popup::toggle_with_max_height(&self.core, &mut self.popup, POPUP_MAX_HEIGHT);
        }
        Task::none()
    }

    fn perform(&mut self, action: PowerAction) -> Task<cosmic::Action<Message>> {
        self.confirm_deadline = None;
        self.set_page(Page::Main);
        let close = self.close_popup();
        let run = Task::perform(action.perform(), |r| {
            cosmic::Action::App(Message::PowerDone(r))
        });
        Task::batch([close, run])
    }

    fn seconds_left(&self) -> u64 {
        match (self.confirm_deadline, self.now) {
            (Some(deadline), Some(now)) => deadline.saturating_duration_since(now).as_secs(),
            _ => CONFIRM_SECONDS,
        }
    }

    fn view_panel(&self) -> Element<'_, Message> {
        let horizontal = self.core.applet.is_horizontal();
        let size = f32::from(self.core.applet.suggested_size(true).0);
        let mut items = panel::items(
            &self.panel_state(),
            &self.config.panel_icons,
            self.config.show_alerts,
            self.config.battery_percentage,
        );
        if items.is_empty() {
            items.push(panel::Item {
                icon: "preferences-system-symbolic".into(),
                alert: false,
                label: None,
            });
        }
        let elements: Vec<Element<'_, Message>> = items
            .into_iter()
            .map(|item| {
                let icon = panel_icon(item.icon, size, item.alert);
                match item.label {
                    Some(label) => {
                        let mut text = self.core.applet.text(label);
                        if item.alert {
                            text = text.class(cosmic::theme::Text::Color(status_color(false)));
                        }
                        widget::Row::new()
                            .push(icon)
                            .push(text)
                            .spacing(2)
                            .align_y(Alignment::Center)
                            .into()
                    }
                    None => icon,
                }
            })
            .collect();
        let content: Element<'_, Message> = if horizontal {
            widget::Row::with_children(elements)
                .spacing(PANEL_SPACING_HORIZONTAL)
                .align_y(Alignment::Center)
                .into()
        } else {
            widget::Column::with_children(elements)
                .spacing(PANEL_SPACING_VERTICAL)
                .align_x(Alignment::Center)
                .into()
        };
        let (major, minor) = self.core.applet.suggested_padding(true);
        let padding = if horizontal {
            [minor, major]
        } else {
            [major, minor]
        };
        let button = button::custom(content)
            .padding(padding)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press_down(Message::TogglePopup);
        self.core
            .applet
            .autosize_window(
                widget::mouse_area(button)
                    .on_right_press(Message::OpenAppletSettings)
                    .on_scroll(|delta| {
                        let y = match delta {
                            ScrollDelta::Lines { y, .. } | ScrollDelta::Pixels { y, .. } => y,
                        };
                        Message::ScrollVolume(y)
                    }),
            )
            .into()
    }

    fn view_main(&self) -> Element<'_, Message> {
        // Fixed group order: battery header, sliders, tiles, media rows, power buttons. Only the
        // order inside the slider and tile groups is configurable.
        let mut column = widget::Column::new().padding([8, 0]);
        if self.config.battery_header
            && let Some(header) = self.battery_header()
        {
            column = column.push(header).push(section_divider());
        }
        for slider in self.config.visible_sliders() {
            let row = match slider {
                Slider::Volume => self.sink_slider(),
                Slider::Microphone => self.source_slider(),
                Slider::Brightness => self.brightness_slider(),
            };
            if let Some(row) = row {
                column = column.push(row);
            }
        }
        // Tiles fill the grid top left, top right, bottom left, bottom right; unavailable ones
        // (no Wi-Fi, no VPN profile, no audio) leave no gap.
        let tiles: Vec<_> = self
            .config
            .visible_tiles()
            .into_iter()
            .filter_map(|tile| match tile {
                Tile::Network => self.network_tile(),
                Tile::Bluetooth => self.bluetooth_tile(),
                Tile::Output => self.audio.sink().is_some().then(|| self.output_tile()),
                Tile::Airplane => self.airplane_tile(),
                Tile::Vpn => self.vpn_tile(),
            })
            .collect();
        if !tiles.is_empty() {
            column = column.push(ui::tile_grid(tiles));
        }
        let media = self.config.media_controls;
        let power_buttons = self.config.power_buttons;
        column = column.push(section_divider());
        // One row per player; no rows and no extra divider without players.
        let rows = if media { self.media_rows() } else { Vec::new() };
        if !rows.is_empty() {
            for row in rows {
                column = column.push(row);
            }
            column = column.push(section_divider());
        }
        if power_buttons {
            column = column
                .push(self.view_power_row(None))
                .push(section_divider());
        }
        column
            .push(
                cosmic::applet::menu_button(text::body(fl!("applet-settings")))
                    .on_press(Message::OpenAppletSettings),
            )
            .into()
    }

    /// Lock, log out, suspend, restart, shut down, and COSMIC Settings. `active` marks the
    /// action being confirmed.
    fn view_power_row(&self, active: Option<PowerAction>) -> Element<'_, Message> {
        let mut row = widget::Row::new().spacing(8).align_y(Alignment::Center);
        for action in PowerAction::ALL {
            let class = if Some(action) == active {
                cosmic::theme::Button::Destructive
            } else {
                cosmic::theme::Button::Standard
            };
            row = row.push(widget::tooltip(
                button::custom(
                    icon::from_name(action.icon())
                        .size(16)
                        .symbolic(true)
                        .icon(),
                )
                .class(class)
                .padding([10, 14])
                .on_press(Message::Power(action)),
                text::caption(power_label(action)),
                widget::tooltip::Position::Top,
            ));
        }
        row = row.push(widget::tooltip(
            button::custom(
                icon::from_name("emblem-system-symbolic")
                    .size(16)
                    .symbolic(true)
                    .icon(),
            )
            .class(cosmic::theme::Button::Standard)
            .padding([10, 14])
            .on_press(Message::OpenSettings("")),
            text::caption(fl!("cosmic-settings")),
            widget::tooltip::Position::Top,
        ));
        padded_control(
            widget::container(row)
                .width(Length::Fill)
                .align_x(Alignment::Center),
        )
        .padding([6, 12])
        .into()
    }

    fn view_confirm(&self, action: PowerAction) -> Element<'_, Message> {
        let seconds = self.seconds_left();
        let (title, body, confirm) = match action {
            PowerAction::Restart => (
                fl!("confirm-restart"),
                fl!("confirm-restart-body", seconds = seconds),
                fl!("restart-now"),
            ),
            PowerAction::LogOut => (
                fl!("confirm-log-out"),
                fl!("confirm-log-out-body", seconds = seconds),
                fl!("log-out-now"),
            ),
            _ => (
                fl!("confirm-shutdown"),
                fl!("confirm-shutdown-body", seconds = seconds),
                fl!("shutdown-now"),
            ),
        };
        let content = widget::Column::new()
            .push(
                icon::from_name(action.icon())
                    .size(32)
                    .symbolic(true)
                    .icon(),
            )
            .push(text::heading(title))
            .push(text::caption(body))
            .push(
                widget::Row::new()
                    .push(button::standard(fl!("cancel")).on_press(Message::CancelPower))
                    .push(button::destructive(confirm).on_press(Message::ConfirmPower))
                    .spacing(8),
            )
            .spacing(8)
            .align_x(Alignment::Center)
            .width(Length::Fill);
        widget::Column::new()
            .padding([8, 0])
            .push(padded_control(content).padding([24, 16]))
            .push(section_divider())
            .push(self.view_power_row(Some(action)))
            .into()
    }
}

fn power_label(action: PowerAction) -> String {
    match action {
        PowerAction::Lock => fl!("lock"),
        PowerAction::LogOut => fl!("log-out"),
        PowerAction::Suspend => fl!("suspend"),
        PowerAction::Restart => fl!("restart"),
        PowerAction::Shutdown => fl!("shutdown"),
    }
}

/// Runs an audio command in the background.
fn audio_task(command: audio::Command) -> Task<cosmic::Action<Message>> {
    Task::perform(audio::run(command), |()| cosmic::Action::App(Message::Done))
}

/// A symbolic icon in the panel's foreground colour, or the alert colour.
fn panel_icon(name: String, size: f32, alert: bool) -> Element<'static, Message> {
    widget::icon(icon::from_name(name).symbolic(true).handle())
        .class(cosmic::theme::Svg::Custom(std::rc::Rc::new(move |theme| {
            let color = if alert {
                status_color(false)
            } else {
                theme.cosmic().background(theme.transparent).on.into()
            };
            cosmic::iced::widget::svg::Style { color: Some(color) }
        })))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}
