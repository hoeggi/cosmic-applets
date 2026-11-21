# System applet: build status

Branch `system` (local, not pushed), checked out in the main checkout, crate `applets/system`
(`cosmic-applet-system`, APP_ID `at.hoeggi.CosmicAppletSystem`), **GPL-3.0-only**.
Design: `docs/system-mockups`. Licensing rules: `docs/LICENSING.md`. Upstream ports and how to
update them: `applets/system/UPSTREAM.md`.

## Round 1: first build (milestones)

| # | Milestone | Commit |
|---|---|---|
| 1 | Scaffold: crate, license, packaging, panel button, page navigation, config | d27c5fb |
| 2 | Power buttons, confirmation with countdown, settings entry points | 6c29c50 |
| 3 | Audio: sliders, Output tile, Sound page, scroll on the panel icon | 2ce2cee |
| 4 | Battery header and page, power profiles, charge limit, peripherals, brightness | 2379add |
| 5 | Bluetooth tile and page | eb02e54 |
| 6 | Network tile and page, airplane mode | 8785a2c |
| 7 | Panel icons, alerts, settings page | e857c2e |
| | Charge limit through system76-power | 5b0a323 |

## Round 2: upstream ports (base: cosmic-applets `ecb6b61`, the installed version)

| Commit | What |
|---|---|
| 00d99c3 | **Workspace:** libcosmic 5a8bd94 → dd108aa (as in ecb6b61's lockfile); commands and transmission use `cosmic_launch::process::spawn`. Touches shared code: merge or cherry-pick to `main` only with the user's approval. |
| 7b64fea | Network from upstream's NetworkManager model (`nmrs`); every Ethernet device listed |
| 384e461 | Build: `libdbus-1-dev` in the builder image and CI (needed by `bluer`). Touches shared files, like 00d99c3. |
| 23bf1b6 | Bluetooth from upstream's BlueZ backend (`bluer`), with upstream's PIN confirmation |
| aa1a991 | Battery, power profiles, charge limit and brightness from upstream; upstream's icons; zbus 6 |
| 0b14bc7 | Power actions from upstream's helpers (`cosmic-login1-zbus`) |
| e9307aa | Audio through upstream's model (route-based names, sorting, upstream's client calls) |
| 9667b89 | Tiles as one rounded piece |
| 5afbbd3 | Media controls for every MPRIS player (new section "Media controls") |

### What is upstream and what is ours

Ported from upstream (`src/upstream/`, behind adapters in `src/adapter/`): network state and
commands, the whole Bluetooth backend, battery backend (profiles, charge limit), battery and
brightness icons and brightness logic, the battery and brightness subscriptions, power actions,
the audio model, volume icons and client calls, the MPRIS subscription.

Still ours (upstream has no equivalent): the UI and design, the panel icon picker and alerts,
the applet settings, the shutdown confirmation page, connected devices' batteries and the
charging text on the Battery page, the device icons in the Sound page's lists, and listing
every media player (a marked addition to upstream's subscription).

## Gates

- Quick check (fmt, clippy `-p cosmic-applet-system`, its tests) passed before every commit;
  29 tests at 5afbbd3.
- Full gate after the libcosmic update (00d99c3): fmt, workspace clippy `-D warnings`, all
  tests (77 passed, 3 ignored) and release builds of every applet passed.
- Final full gate at 5afbbd3 (2026-11-21): fmt, workspace clippy `-D warnings`, all tests
  (84 passed, 3 ignored) and release builds of every applet passed.

## Try in the panel

Install with `make install APPLET=system`, then `make reload-panel`.
Things to check:
- Network page with the USB Ethernet adapter: both wired ports listed, the USB one "Connected"
  and "In use", the built-in one "Cable unplugged"; Network tile shows "Ethernet".
- Tiles: one rounded piece each, divider before the chevron, in light and dark themes.
- Battery page: the "Limit charge to 80%" switch is visible (system76-power answers here).
  Switching it really changes the charge thresholds.
- Volumes: sliders show the real output/input levels; device names like the built-in applet.
- Media controls: one row per player (e.g. Spotify and two VLC windows), buttons per player.
- Brightness slider and its icon; power modes; Bluetooth page and pairing.

## Decisions

Decided by the user (2026-11-21):
- Detail pages end with a link to the matching COSMIC Settings page; no gear in their headers.
- The main popup's button row has a sixth button that opens COSMIC Settings.
- The applet's own settings open from "Applet settings…" at the bottom of the main popup, below
  a divider, and from a right click on the panel icon.
- The Network tile is neutral ("Network", the connection in use; it only opens the page).
- Output tile body toggles mute; Bluetooth tile body switches Bluetooth; Log out asks first;
  section order with up/down arrows.
- Media controls: every player listed, all three buttons always shown (disabled when not
  supported), no media controls in the panel.

**Changed by the port, for the user to confirm** (upstream's behaviour now, unlike before):
- Airplane mode is NetworkManager's (`nmrs::set_airplane_mode`): it switches Wi-Fi, mobile
  broadband and Bluetooth in one call, using rfkill as upstream does (previously: software only,
  no rfkill). It shows as on when every present radio is off.
- Bluetooth: upstream's backend registers its own pairing agent (not the default one). Numeric
  PIN confirmations appear on the Bluetooth page with Cancel / Confirm, as in the built-in
  applet; other PIN or passkey requests are ignored, as upstream does (previously: no agent).

Chosen here, not discussed:
- Closing the popup cancels a pending countdown.
- "In use" for wired: nmrs doesn't report the default route, so wired counts as in use whenever
  an Ethernet device is connected (NetworkManager prefers wired by route metric by default).

## Known gaps

- Microphone panel icon and the "mic muted while an app records" alert are not built: the
  audio service reports playback streams but not recording streams. The other five alerts work.
- nmrs reports no Wi-Fi bitrate, so the connected network says "Connected" without a speed.
- Wi-Fi: joining a new secured network asks for a WPA/WPA2 personal password inline;
  enterprise (802.1X) networks need Network settings….
- Power modes have no icons (neither the COSMIC nor the Pop theme has power-profile icons).
- The shared builder image now includes `libdbus-1-dev`. A `make` in another checkout whose
  Containerfile lacks it rebuilds the image without it (the image is shared by hash label), and
  the next system build here rebuilds it again, until 384e461 reaches `main`.

## Notes for common/

Nothing: `common/` is unchanged on this branch.
