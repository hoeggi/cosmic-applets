# Changelog

All notable changes to `cosmic-applet-system` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/). Releases are tagged `system-v<version>`.

## [0.2.0] - 2026-11-22

### Added
- Media rows show covers that players send as a web address, such as Spotify's album art,
  instead of the note icon. Covers are downloaded once and kept in the user's cache.
- When a player sends no cover at all, its row shows the application's icon.

## [0.1.0] - 2026-11-21

### Added
- One panel applet for sound, battery, Bluetooth, network and power, in place of COSMIC's five
  separate applets and built on their code. Licensed GPL-3.0-only for that reason.
- Panel icons for network, VPN, Bluetooth, volume and battery, each optional, with the battery
  percentage. With alerts on, hidden icons appear in red while something needs attention: no
  network, low battery, muted output, a Bluetooth device running low, or a VPN that dropped.
  Scrolling on the icons changes the volume.
- The popup shows the battery with time left and power mode, sliders for volume, microphone and
  brightness, tiles for network, Bluetooth, output, airplane mode and VPN, and one row per
  playing or paused media player with previous, play/pause and next.
- Pages for battery (power modes, an 80% charge limit where the hardware supports it, batteries
  of connected devices), sound (outputs and inputs), network (wired, Wi-Fi with a password
  prompt, VPN) and Bluetooth (paired devices, scanning, pairing with PIN confirmation).
- Buttons for lock, log out, suspend, restart, shut down and COSMIC Settings. Log out, restart
  and shut down ask first, with a 60-second countdown.
- Applet settings, from the popup or by right-clicking the panel icon: the battery header,
  the order of sliders and tiles, media controls, power buttons, panel icons, alerts and the
  shutdown confirmation.
