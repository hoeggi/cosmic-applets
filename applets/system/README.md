# COSMIC System applet

One panel applet for audio, battery, Bluetooth, network and power, replacing five separate
applets. The design is in [docs/system-mockups](../../docs/system-mockups).

- **Main popup:** battery charge and power mode, sliders for volume, microphone and
  brightness, tiles for Wi-Fi, Bluetooth, the audio output and airplane mode, and buttons to
  lock, log out, suspend, restart and shut down. A tile's body toggles it; its chevron opens
  the detail page.
- **Detail pages:** Battery, Sound, Network (wired, Wi-Fi and VPN) and Bluetooth.
- **Panel:** pick which icons show, and whether problems with hidden icons still show up.

Sections without hardware (no battery, no backlight, no Wi-Fi card) are left out.

## License

GPL-3.0-only, unlike the rest of this repository: it builds on code from System76's
[cosmic-applets](https://github.com/pop-os/cosmic-applets). See [LICENSE](LICENSE) and
[docs/LICENSING.md](../../docs/LICENSING.md).
