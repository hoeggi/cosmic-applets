# System applet: build status

Branch `system` (local, not pushed), worktree `.worktrees/system`, crate `applets/system`
(`cosmic-applet-system`, APP_ID `at.hoeggi.CosmicAppletSystem`), **GPL-3.0-only**.
Design: `docs/system-mockups`. Licensing rules: `docs/LICENSING.md`.

## Milestones

| # | Milestone | State | Commit |
|---|---|---|---|
| 1 | Scaffold: crate, license, packaging, panel button, page navigation, config | done | see git log |
| 2 | Power buttons, confirmation with countdown | next | |
| 3 | Audio: sliders, Output tile, Sound page | | |
| 4 | Battery, power profiles, brightness | | |
| 5 | Bluetooth | | |
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

## Defaults chosen for open questions

- Settings open from the gear on the detail pages; the main popup has no gear. (Scaffold has a
  temporary gear at the bottom of the main popup until the settings page lands.)
- The network tile toggles the Wi-Fi radio but shows the icon and subtitle of the connection
  in use.
- Log out asks first like Restart and Shut down (the design only named the latter two).

## Not tested

Nothing has been seen in the real panel: everything is built and unit-tested in the container
only. Run it with `make run APPLET=system`, or install the `.deb`, to check the UI.

## Notes for common/

Nothing so far.
