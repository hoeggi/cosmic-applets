# Changelog

All notable changes to `cosmic-applet-commands` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/). Releases are tagged `commands-v<version>`.

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
