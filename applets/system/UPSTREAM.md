# Upstream code in the system applet

The backends of this applet are ported from System76's
[pop-os/cosmic-applets](https://github.com/pop-os/cosmic-applets) at commit
**`ecb6b61`** (2026-11-06), the version the installed COSMIC desktop is built from. The
workspace's libcosmic (`dd108aa`) matches that commit's `Cargo.lock`.

## Layout

- `src/upstream/` holds upstream code, as close to verbatim as possible. Each file names its
  source at the top. Every change is marked with a `// system-applet:` comment.
- `src/adapter/` is ours: it turns upstream's state and events into what our pages show, and our
  UI's commands into upstream calls. The UI only talks to the adapters.

## File mapping

| Upstream (at `ecb6b61`) | Here | How |
|---|---|---|
| `cosmic-applet-battery/src/backend/mod.rs`, `power_daemon.rs`, `power_profiles.rs` | `src/upstream/battery/` (same names) | Verbatim: power profiles (system76-power, then power-profiles-daemon) and the system76-power charge limit. |
| `cosmic-applet-battery/src/app.rs` | `src/upstream/battery/app.rs` | `update_battery` (icon), `screen_brightness_percent`, `update_display` (brightness icon) and the `SetScreenBrightness` snapping, as free functions. |
| `cosmic-applet-bluetooth/src/bluetooth.rs` | `src/upstream/bluetooth.rs` | Whole file, verbatim: the `bluer` subscription, requests and pairing agent. |
| `cosmic-applet-network/src/app.rs` | `src/upstream/network.rs` | Backend part only: the state types, `snapshot_to_applet`, `network_type`, `wifi_security`, `wifi_icon`, `HwAddress`, `active_conn_hw_address`. The applet struct, UI, task wrappers and secret agent are left out. |

Adapters:
- `src/adapter/battery.rs`: upstream's battery handlers (UPower display device through
  `cosmic-settings-upower-subscription`, power profiles, charge limit, brightness through
  `cosmic-settings-daemon-subscription` with upstream's 200 ms send loop while dragging), and
  upstream's icons (`cosmic-applet-battery-…`, installed with the cosmic-applets package). Ours
  in it: the batteries of connected devices and the charging state text, which upstream's
  battery applet doesn't have.
- `src/adapter/bluetooth.rs`: keeps upstream's state, request sender and PIN confirmation, does
  upstream's optimistic status changes on requests, and splits devices into known and nearby
  like upstream's popup. Scanning on the Bluetooth page uses upstream's `set_discovery` and
  `set_tick` (3 s while open, 10 s otherwise), as upstream's popup does.
- `src/adapter/network.rs`: view model, `nmrs` subscription and commands, the same `nmrs` calls
  as upstream's message handlers.

## Updating

1. Check out the new upstream commit (the installed `cosmic-applets` package's version is in
   `dpkg-query -W cosmic-applets`; the part after the last `~` is the commit).
2. Raise libcosmic in the workspace to that commit's `Cargo.lock` revision (all applets, full
   gate).
3. For each row above, copy the new upstream code over the file in `src/upstream/` (for
   extracted parts, the same items), then re-apply the `system-applet:` changes listed below.
   `git diff` against the previous port shows what upstream changed.
4. Fix the adapters until the quick check passes; update the base commit in this file.

## `system-applet:` changes

`src/upstream/battery/mod.rs`:
- `#![allow(dead_code, clippy::collapsible_if)]`: our workspace denies warnings.
- `pub mod app;` for the functions extracted from upstream's `app.rs`.

`src/upstream/battery/app.rs`:
- Upstream's methods and handler code as free functions taking the fields they read; bodies
  unchanged.

`src/upstream/bluetooth.rs`:
- `#![allow(dead_code, unreachable_patterns, clippy::collapsible_if, clippy::while_let_loop)]`:
  our workspace denies warnings; the code is unchanged.

`src/upstream/network.rs`:
- `#![allow(dead_code, clippy::unnecessary_sort_by)]`: unused upstream items and a newer clippy
  lint, allowed instead of changing upstream code.
- `AppletSnapshot::wired_devices` (new field) and its value in `snapshot_to_applet`: every
  Ethernet device, also unplugged ones, for our Wired section.
- `snapshot_to_applet`, `wifi_icon`, `wifi_security` made `pub` for the adapter.
