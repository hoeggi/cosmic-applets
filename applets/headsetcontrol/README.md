# COSMIC HeadsetControl applet

Panel applet for [HeadsetControl](https://github.com/Sapd/HeadsetControl): battery level as the panel
icon, chatmix and headset settings in the popup. It drives the installed `headsetcontrol` CLI
(`-o json`, API 1.x), so install that first (e.g. `ppa:sapd/headsetcontrol`).

- Settings the headset can report back (e.g. sidetone on devices with `CAP_SIDETONE_STATUS`) are always shown.
- Write-only settings appear when **Show write-only settings** is on; the applet remembers the last
  value set per device (`vid:pid`).
- **Re-apply on reconnect** resends those remembered values when the headset comes back online
  (also once when the applet starts with the headset already on).
- A notification fires once when the battery drops to `low_battery_percent` (default 20, 0 = off).

Config lives in `~/.config/cosmic/at.hoeggi.CosmicAppletHeadsetControl/v1/`
(`poll_secs`, `low_battery_percent`, `test_device`, …).

## Development

Build, run and package from the repository root (see the [top-level README](../../README.md)):

    HSC_TEST_DEVICE=1 make run APPLET=headsetcontrol   # against headsetcontrol's mock device
