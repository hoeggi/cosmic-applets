// SPDX-License-Identifier: GPL-3.0-only

use crate::config::{Config, Section};
use crate::fl;
use crate::ui;
use cosmic::cosmic_config;
use cosmic::iced::{Alignment, Length, Subscription, window::Id};
use cosmic::prelude::*;
use cosmic::widget::{self, button, icon};
use cosmic_applet_common::widgets::{icon_button, section_divider};
use cosmic_applet_common::{config as cfg, popup};

pub const APP_ID: &str = "at.hoeggi.CosmicAppletSystem";

/// Popup height limit; the compositor slides and flips popups but doesn't shrink them.
const POPUP_MAX_HEIGHT: f32 = 900.0;

/// The page the popup shows. Detail pages are opened by the chevrons on the main page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Main,
    Battery,
    Sound,
    Network,
    Bluetooth,
    Settings,
}

#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    page: Page,
    config: Config,
    config_ctx: Option<cosmic_config::Config>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    UpdateConfig(Config),
    Open(Page),
    /// Opens a page of COSMIC Settings, e.g. "sound" or "wireless".
    OpenSettings(&'static str),
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
            Page::Battery => self.view_placeholder(fl!("battery"), "power"),
            Page::Sound => self.view_placeholder(fl!("sound"), "sound"),
            Page::Network => self.view_placeholder(fl!("network"), "network"),
            Page::Bluetooth => self.view_placeholder(fl!("bluetooth"), "bluetooth"),
            Page::Settings => self.view_placeholder(fl!("settings"), ""),
        };
        let body = widget::container(widget::scrollable(content)).max_height(POPUP_MAX_HEIGHT);
        self.core.applet.popup_container(body).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        cfg::watch::<Config>(self.core(), Self::APP_ID).map(Message::UpdateConfig)
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::TogglePopup => {
                let task =
                    popup::toggle_with_max_height(&self.core, &mut self.popup, POPUP_MAX_HEIGHT);
                if self.popup.is_some() {
                    self.page = Page::Main;
                }
                return task;
            }
            Message::PopupClosed(id) => popup::closed(&mut self.popup, id),
            Message::UpdateConfig(config) => self.config = config,
            Message::Open(page) => self.page = page,
            Message::OpenSettings(page) => {
                let mut cmd = std::process::Command::new("cosmic-settings");
                if !page.is_empty() {
                    cmd.arg(page);
                }
                if let Err(err) = cmd.spawn() {
                    tracing::warn!(%err, "cannot start cosmic-settings");
                }
            }
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl AppModel {
    /// The config context, for writing settings.
    #[allow(dead_code)]
    pub fn config_ctx(&self) -> Option<&cosmic_config::Config> {
        self.config_ctx.as_ref()
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
        self.core
            .applet
            .autosize_window(
                button::custom(content)
                    .padding(padding)
                    .class(cosmic::theme::Button::AppletIcon)
                    .on_press_down(Message::TogglePopup),
            )
            .into()
    }

    fn view_main(&self) -> Element<'_, Message> {
        let mut column = widget::Column::new().padding([8, 0]);
        let mut tiles = Vec::new();
        for section in self.config.visible_sections() {
            match section {
                Section::Network => tiles.push(ui::tile(
                    "network-wireless-symbolic",
                    fl!("wifi"),
                    String::new(),
                    false,
                    None,
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
                                .push(widget::text::body(fl!("battery")).width(Length::Fill))
                                .push(icon::from_name("go-next-symbolic").size(16).icon()),
                        )
                        .on_press(Message::Open(Page::Battery)),
                    );
                }
                Section::Microphone | Section::Brightness | Section::PowerButtons => {}
            }
        }
        if !tiles.is_empty() {
            column = column.push(ui::tile_grid(tiles));
        }
        column = column.push(section_divider()).push(
            widget::Row::new()
                .push(widget::space::horizontal())
                .push(icon_button("emblem-system-symbolic").on_press(Message::Open(Page::Settings)))
                .padding([0, 12]),
        );
        column.into()
    }

    /// Detail page skeleton until its milestone lands.
    fn view_placeholder(&self, title: String, settings_page: &'static str) -> Element<'_, Message> {
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
                fl!("open-settings"),
                Message::OpenSettings(settings_page),
            ));
        }
        column.into()
    }
}
