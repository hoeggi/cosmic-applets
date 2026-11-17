// SPDX-License-Identifier: MPL-2.0

mod api;
mod app;
mod chart;
mod config;
mod i18n;
mod model;

fn main() -> cosmic::iced::Result {
    cosmic_applet_common::init_logging("cosmic_applet_weather");

    // Get the system's preferred languages.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    // Enable localizations to be applied.
    i18n::init(&requested_languages);

    // Starts the applet's event loop with `()` as the application's flags.
    cosmic::applet::run::<app::AppModel>(())
}
