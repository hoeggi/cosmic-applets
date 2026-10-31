# Changelog

All notable changes to `cosmic-applet-commands` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/). Releases are tagged `commands-v<version>`.

## [0.1.2] - 2026-10-31

### Fixed
- With the panel on several monitors, a running command now shows on all of them, in the panel
  icon and in the popup, instead of only on the monitor it was started from.
- Finished runs appear in "Recent runs" and as the command's status on all monitors right away.
- A command that is already running can no longer be started again from another monitor.

## [0.1.1] - 2026-10-16

### Changed
- No changes to the applet itself. Release notes on GitHub are now taken from this changelog.

## [0.1.0] - 2026-10-16

### Added
- Panel applet listing user-defined shell commands; clicking one runs it and shows its status
  (running time, exit code) and captured output.
- Commands can run in a terminal instead (`cosmic-term` by default).
- Add, edit, reorder and delete commands in the popup; stored via cosmic-config, hand edits
  are picked up live.
- "Recent runs" history with time, duration, result and output, kept across restarts.
- Desktop notifications for runs that finish while the popup is closed or take longer than 2 s;
  the panel icon briefly shows an error state after a failed run.
