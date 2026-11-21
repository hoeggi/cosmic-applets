// SPDX-License-Identifier: GPL-3.0-only

mod adapter;
mod app;
mod audio;
mod config;
mod i18n;
mod panel;
mod power;
mod ui;
mod upstream;

fn main() -> cosmic::iced::Result {
    cosmic_applet_common::init_logging("cosmic_applet_system");

    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);

    cosmic::applet::run::<app::AppModel>(())
}
