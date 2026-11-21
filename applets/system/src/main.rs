// SPDX-License-Identifier: GPL-3.0-only

mod app;
mod config;
mod i18n;
// Some helpers are only used by later milestones.
#[allow(dead_code)]
mod ui;

fn main() -> cosmic::iced::Result {
    cosmic_applet_common::init_logging("cosmic_applet_system");

    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);

    cosmic::applet::run::<app::AppModel>(())
}
