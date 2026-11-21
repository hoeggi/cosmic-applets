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
| 4 | Battery header and page, power profiles, charge limit, peripherals, brightness | done | see git log |
| 5 | Bluetooth | next | |
| 6 | Network, airplane mode | | |
| 7 | Panel icons, alerts, settings page | | |

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

## Known gaps

- Charge limit: uses UPower's charge-threshold API, which this machine's UPower 1.90.3 doesn't
  expose, so the toggle stays hidden here. system76-power's `SetChargeThresholds` could back
  it on Pop!_OS; not done.

## Lockfile

The audio client needs tokio 1.54 and zcheapstr ≥ 1.2, so `Cargo.lock` moves tokio
1.53.2 → 1.54.1 and zcheapstr 1.1.0 → 1.4.0 for the whole workspace (semver-compatible).

## Not tested

Nothing has been seen in the real panel: everything is built and unit-tested in the container
only. Run it with `make run APPLET=system`, or install the `.deb`, to check the UI.

## Notes for common/

Nothing so far.
