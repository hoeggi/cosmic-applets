# Changelog

All notable changes to `cosmic-applet-headsetcontrol` are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/). Releases are tagged `headsetcontrol-v<version>`.

## [0.1.0] - 2026-10-16

### Added
- Panel icon: headset inside a ring gauge filled to the exact battery level, with charging,
  headset-off and "headsetcontrol missing" states; follows the panel's light/dark colours.
- Popup with device name, battery level and chatmix balance (game ↔ chat).
- Settings driven by the headset's capabilities: sidetone, equalizer preset, microphone volume,
  mute LED brightness, noise filter, rotate to mute, volume limiter, lights, voice prompts,
  auto-off time and Bluetooth options.
- "Show write-only settings": settings the headset cannot report back are shown with the last
  value set from the applet, remembered per device.
- "Re-apply on reconnect": resends remembered settings when the headset comes back online.
- Low-battery notification (default 20 %, configurable via `low_battery_percent`).
- Works with the `headsetcontrol` CLI 4.1+ (`-o json`, API 1.x).
