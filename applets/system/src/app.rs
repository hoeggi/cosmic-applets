// SPDX-License-Identifier: GPL-3.0-only

use crate::config::{Config, Section};
use crate::fl;
use crate::power::PowerAction;
use crate::ui;
use cosmic::applet::padded_control;
use cosmic::cosmic_config;
use cosmic::iced::{Alignment, Length, Subscription, window::Id};
use cosmic::prelude::*;
use cosmic::widget::{self, button, icon, text};
use cosmic_applet_common::widgets::section_divider;
use cosmic_applet_common::{config as cfg, popup};
use std::time::{Duration, Instant};

pub const APP_ID: &str = "at.hoeggi.CosmicAppletSystem";

/// Popup height limit; the compositor slides and flips popups but doesn't shrink them.
const POPUP_MAX_HEIGHT: f32 = 900.0;
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
        (app, Task::none())
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
            Page::Battery => self.view_placeholder(fl!("battery"), fl!("power-settings"), "power"),
            Page::Sound => self.view_placeholder(fl!("sound"), fl!("sound-settings"), "sound"),
            Page::Network => {
                self.view_placeholder(fl!("network"), fl!("network-settings"), "network")
            }
            Page::Bluetooth => {
                self.view_placeholder(fl!("bluetooth"), fl!("bluetooth-settings"), "bluetooth")
            }
            Page::Settings => {
                self.view_placeholder(fl!("applet-settings-title"), String::new(), "")
            }
            Page::Confirm(action) => self.view_confirm(action),
        };
        let body = widget::container(widget::scrollable(content)).max_height(POPUP_MAX_HEIGHT);
        self.core.applet.popup_container(body).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subs =
            vec![cfg::watch::<Config>(self.core(), Self::APP_ID).map(Message::UpdateConfig)];
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
                self.page = Page::Main;
                self.confirm_deadline = None;
                return task;
            }
            Message::PopupClosed(id) => {
                popup::closed(&mut self.popup, id);
                if self.popup.is_none() {
                    // Closing the popup cancels a pending restart or shutdown.
                    self.confirm_deadline = None;
                    self.page = Page::Main;
                }
            }
            Message::UpdateConfig(config) => self.config = config,
            Message::Open(page) => self.page = page,
            Message::OpenAppletSettings => {
                self.page = Page::Settings;
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
                self.page = Page::Main;
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
    /// The config context, for writing settings.
    #[allow(dead_code)]
    pub fn config_ctx(&self) -> Option<&cosmic_config::Config> {
        self.config_ctx.as_ref()
    }

    fn close_popup(&mut self) -> Task<cosmic::Action<Message>> {
        if self.popup.is_some() {
            return popup::toggle_with_max_height(&self.core, &mut self.popup, POPUP_MAX_HEIGHT);
        }
        Task::none()
    }

    fn perform(&mut self, action: PowerAction) -> Task<cosmic::Action<Message>> {
        self.confirm_deadline = None;
        self.page = Page::Main;
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
        let content = widget::Row::new()
            .push(
                icon::from_name("preferences-system-symbolic")
                    .size(16)
                    .symbolic(true)
                    .icon(),
            )
            .spacing(4)
            .align_y(Alignment::Center);
        let (major, minor) = self.core.applet.suggested_padding(true);
        let padding = if self.core.applet.is_horizontal() {
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
            .autosize_window(widget::mouse_area(button).on_right_press(Message::OpenAppletSettings))
            .into()
    }

    fn view_main(&self) -> Element<'_, Message> {
        let mut column = widget::Column::new().padding([8, 0]);
        let mut tiles = Vec::new();
        let mut power_buttons = false;
        for section in self.config.visible_sections() {
            match section {
                Section::Network => tiles.push(ui::tile(
                    "network-wireless-symbolic",
                    fl!("network"),
                    String::new(),
                    false,
                    Some(Message::Open(Page::Network)),
                    Some(Message::Open(Page::Network)),
                )),
                Section::Bluetooth => tiles.push(ui::tile(
                    "bluetooth-active-symbolic",
                    fl!("bluetooth"),
                    String::new(),
                    false,
                    None,
                    Some(Message::Open(Page::Bluetooth)),
                )),
                Section::Volume => tiles.push(ui::tile(
                    "audio-speakers-symbolic",
                    fl!("output"),
                    String::new(),
                    false,
                    None,
                    Some(Message::Open(Page::Sound)),
                )),
                Section::Battery => {
                    column = column.push(
                        cosmic::applet::menu_button(
                            widget::Row::new()
                                .push(text::body(fl!("battery")).width(Length::Fill))
                                .push(icon::from_name("go-next-symbolic").size(16).icon()),
                        )
                        .on_press(Message::Open(Page::Battery)),
                    );
                }
                Section::PowerButtons => power_buttons = true,
                Section::Microphone | Section::Brightness => {}
            }
        }
        if !tiles.is_empty() {
            column = column.push(ui::tile_grid(tiles));
        }
        column = column.push(section_divider());
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

    /// Detail page skeleton until its milestone lands.
    fn view_placeholder(
        &self,
        title: String,
        settings_label: String,
        settings_page: &'static str,
    ) -> Element<'_, Message> {
        let mut column = widget::Column::new()
            .padding([8, 0])
            .push(ui::page_header(
                title,
                None,
                Message::Open(Page::Main),
                Vec::new(),
            ))
            .push(section_divider())
            .push(ui::note(fl!("not-available")));
        if !settings_page.is_empty() {
            column = column.push(section_divider()).push(ui::settings_link(
                settings_label,
                Message::OpenSettings(settings_page),
            ));
        }
        column.into()
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
