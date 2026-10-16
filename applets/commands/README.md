# Commands applet

Run configurable shell commands from the COSMIC panel. Clicking a command runs it and shows its
status and output; long or failed runs send a notification; recent runs are kept in a history.
Commands are added, edited and reordered in the popup and stored via cosmic-config under
`~/.config/cosmic/at.hoeggi.CosmicAppletCommands/v1/`.

See [PLAN.md](PLAN.md) for the design and [docs/](docs) for the UI mockup.

## Development

Build, run and package from the repository root (see the [top-level README](../../README.md)):

    make run APPLET=commands

## Translators

[Fluent](https://projectfluent.org/) is used for localization; translations live in [i18n](i18n).
Copy the `en` directory to a new ISO 639-1 code and translate the message identifiers.
