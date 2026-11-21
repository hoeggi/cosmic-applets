# System applet: build status

Branch `system` (local, not pushed), worktree `.worktrees/system`, crate `applets/system`
(`cosmic-applet-system`, APP_ID `at.hoeggi.CosmicAppletSystem`), **GPL-3.0-only**.
Design: `docs/system-mockups`. Licensing rules: `docs/LICENSING.md`.

## Milestones

| # | Milestone | State | Commit |
|---|---|---|---|
| 1 | Scaffold: crate, license, packaging, panel button, page navigation, config | done | d27c5fb |
| 2 | Power buttons, confirmation with countdown, settings entry points | done | 6c29c50 |
| 3 | Audio: sliders, Output tile, Sound page, scroll on the panel icon | done | 2ce2cee |
| 4 | Battery header and page, power profiles, charge limit, peripherals, brightness | done | 2379add |
| 5 | Bluetooth tile and page: power, paired devices, scan, pair, connect | done | eb02e54 |
| 6 | Network tile and page (wired, Wi-Fi, VPN, In use), airplane mode, VPN tile on desktops | done | 8785a2c |
| 7 | Panel icons, alerts, settings page | done | e857c2e |

Full gate (`fmt --check`, workspace clippy `-D warnings`, workspace tests) passed on
2026-11-21 at e857c2e: 76 tests, 3 ignored, across all applets.

## Next steps

- Try it in the panel (`make deb` then install, or `make run APPLET=system`) and check every
  page; nothing has been seen on screen yet.
- Charge limit through system76-power where UPower lacks it.
- A Bluetooth pairing agent for PIN and passkey devices.
- Decide whether the Microphone panel icon is worth a PipeWire query for recording streams.

## How to run and check

From the worktree:

    make run APPLET=system        # standalone window (popup pages need the panel)

Quick check (per commit; the full `make check test` gate runs before merging):

    podman run --rm --userns=keep-id -v $PWD:/src -w /src \
      -v /home/hoeggi/repos/cosmic-applets/.cache:/src/.cache \
      -e HOME=/tmp -e USER=$USER -e CARGO_HOME=/src/.cache/cargo cosmic-applet-builder \
      sh -ec "cargo fmt --all --check && cargo clippy --release -p cosmic-applet-system --all-targets -- -D warnings && cargo test --release -p cosmic-applet-system"

## Backends and licenses

- Power: login1, com.system76.CosmicSession (org.gnome.SessionManager under pop:GNOME), with
  zbus 5 proxies in `src/power.rs`, adapted from cosmic-applet-power (GPL).
- Audio: cosmic-settings-daemon's varlink socket through `cosmic-settings-audio-client`
  (MPL-2.0), as upstream's audio applet does.
- Battery, Bluetooth, network, brightness: zbus 5 bindings from `dbus-settings-bindings`
  (MPL-2.0 / MIT), the same revision libcosmic already pulls in.
- Verified licenses are in `docs/LICENSING.md`. The cosmic-settings subscription crates are
  MPL-2.0 but need a newer libcosmic than this workspace pins, so they aren't used.

## Decisions

Decided by the user (2026-11-21):
- Detail pages end with a link to the matching COSMIC Settings page ("Sound settings…",
  "Network settings…", "Bluetooth settings…", "Power settings…"). Their headers have no gear;
  refresh icons stay.
- The main popup's button row has a sixth button after Shut down that opens COSMIC Settings.
- The applet's own settings page opens from an "Applet settings…" row at the very bottom of
  the main popup, and from a right click on the panel icon.
- The network tile is neutral: named "Network", with the icon and subtitle of the connection
  in use (SSID, "Ethernet", plus " · VPN"; "Disconnected" or "Off" otherwise). Clicking it only
  opens the Network page; Wi-Fi on/off lives on that page. On desktops the VPN tile in the free
  slot keeps its toggle and chevron.

Chosen here (tell me if you want it different):
- The Output tile's body toggles mute; its chevron opens the Sound page. Scrolling on the
  panel icon changes the volume in 5 % steps.
- Log out asks first, like Restart and Shut down. Lock and Suspend act at once.
- Closing the popup cancels a pending countdown.
- The Bluetooth tile's body switches Bluetooth on and off. The Bluetooth page scans while it is
  open and stops when it is left or the popup closes.

## Known gaps

- Microphone panel icon and the "mic muted while an app records" alert are not built: the
  audio service reports playback streams but not recording streams, so the applet can't tell
  when an app records. The config keeps the `microphone` flag; the settings page doesn't offer
  it.
- The other five alerts work: no network, low battery (< 10 % discharging), muted output, a
  connected Bluetooth device below 20 %, and a VPN that went down without being switched off
  in the applet.
- Section order uses up/down arrows instead of the mockup's drag handles.

- Charge limit: uses UPower's charge-threshold API, which this machine's UPower 1.90.3 doesn't
  expose, so the toggle stays hidden here. system76-power's `SetChargeThresholds` could back
  it on Pop!_OS; not done.

- Bluetooth pairing registers no agent of its own. Devices that need a PIN or a passkey
  confirmation only pair if another agent runs (e.g. COSMIC Settings' Bluetooth page); "Just
  works" devices pair directly.

- Wi-Fi: joining a new secured network asks for a WPA/WPA2 personal password inline.
  Enterprise (802.1X) and WEP networks aren't handled; use Network settings… for those.
- Airplane mode switches Wi-Fi and mobile broadband off through NetworkManager and Bluetooth
  through BlueZ. It's shown as on while Wi-Fi and Bluetooth are both off. It doesn't use rfkill
  directly, so a hardware airplane key isn't reflected separately.
- Power profiles have no icons: neither the COSMIC nor the Pop icon theme has
  power-profile-* icons, and libcosmic doesn't fall back to Adwaita.

## Lockfile

The audio client needs tokio 1.54 and zcheapstr ≥ 1.2, so `Cargo.lock` moves tokio
1.53.2 → 1.54.1 and zcheapstr 1.1.0 → 1.4.0 for the whole workspace (semver-compatible).

## Not tested

Nothing has been seen in the real panel: everything is built and unit-tested in the container
only. Run it with `make run APPLET=system`, or install the `.deb`, to check the UI.

## Notes for common/

Nothing so far.
