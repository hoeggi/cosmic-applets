# Changelog

All notable changes to `cosmic-applet-transmission` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/). Releases are tagged `transmission-v<version>`.

## [0.1.0] - 2026-10-16

### Added
- Panel icon: a ring filled to the overall progress of the running downloads, with their count
  inside; shows `!` when a torrent failed and a dashed ring when the server is unreachable or
  the login failed. Follows the panel's light/dark colours.
- Popup with a read-only torrent list, filtered by Active, Downloading, Seeding or All, plus
  the current download and upload rates. Clicking the list opens your Transmission client
  (default `flatpak run io.github.TransmissionRemoteGtk`, configurable).
- Connection settings in the popup (host, port, RPC path, username, password). The password is
  stored in the desktop keyring (Secret Service), not in the config.
- Notifications when a download finishes or a torrent fails; each can be turned off.
- Works with Transmission 4.1 or newer (JSON-RPC 2.0), over plain HTTP on the local network.
