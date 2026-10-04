// SPDX-License-Identifier: MPL-2.0

mod app;
mod config;
mod i18n;
mod runner;

fn main() -> cosmic::iced::Result {
    // stderr is forwarded by cosmic-panel to the user journal:
    // `journalctl --user -f -t cosmic-applet-commands`
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,cosmic_applet_commands=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    // Get the system's preferred languages.
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    // Enable localizations to be applied.
    i18n::init(&requested_languages);

    // Starts the applet's event loop with `()` as the application's flags.
    cosmic::applet::run::<app::AppModel>(())
}
