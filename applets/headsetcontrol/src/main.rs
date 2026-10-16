// SPDX-License-Identifier: MPL-2.0

mod app;
mod config;
mod hsc;
mod i18n;
mod icon;
mod settings;

fn main() -> cosmic::iced::Result {
    cosmic_applet_common::init_logging("cosmic_applet_headsetcontrol");

    // Get the system's preferred languages.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    // Enable localizations to be applied.
    i18n::init(&requested_languages);

    // Starts the applet's event loop with `()` as the application's flags.
    cosmic::applet::run::<app::AppModel>(())
}
