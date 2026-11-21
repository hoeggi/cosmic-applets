# Licensing

This repository holds several programs under two licenses. The license applies per crate (and
per file), not to the repository as a whole.

| What | License | Where it's declared |
|---|---|---|
| Workspace default, `common/` and every applet not listed below | MPL-2.0 | `license` in the root `Cargo.toml`, inherited with `license.workspace = true` |
| Applets built on System76's `cosmic-applets` code (planned: `applets/system`) | GPL-3.0-only | `license = "GPL-3.0-only"` in the applet's own `Cargo.toml` |
| AppStream metadata (`resources/app.metainfo.xml`) | CC0-1.0 | `<metadata_license>` |

The MPL-2.0 text is in [`LICENSE`](../LICENSE). A GPL applet ships the GPL-3.0 text as
`applets/<name>/LICENSE`.

## Why two licenses

- The applets started from System76's `cosmic-applet-template`, which is MPL-2.0. MPL is
  weak, per-file copyleft: changes to an MPL file stay MPL, but the code may be combined with
  code under any other license, including proprietary code.
- System76's built-in applets ([pop-os/cosmic-applets](https://github.com/pop-os/cosmic-applets))
  are GPL-3.0-only, except `cosmic-applet-network`, which is GPL-3.0-or-later. Code copied or
  adapted from there makes the whole program a derivative work, so that applet has to be
  distributed as GPL-3.0. Use GPL-3.0-only: it covers both, since or-later code can be used
  under 3.0-only.
- libcosmic, which every applet builds on, is MPL-2.0 and places no restriction on either.

## Rules

1. **No GPL code in `common/`.** Nothing copied or adapted from GPL sources (System76's
   applets, cosmic-settings, cosmic-panel, …) may go into `common/`, not even a small helper.
   `common/` is linked into every applet; GPL code there would turn all of them into GPL
   programs. If a GPL applet has something other applets want, it stays in the GPL applet,
   or someone writes it from scratch for `common/` without copying (see rule 5).
2. **MPL into GPL is fine, GPL into MPL is not.** A GPL applet may depend on `common/` and may
   copy code from the MPL applets. An MPL applet must never copy code from a GPL applet or
   from GPL sources.
3. **A GPL applet declares itself.** In that crate: `license = "GPL-3.0-only"` in
   `Cargo.toml` (overriding the workspace default), `// SPDX-License-Identifier: GPL-3.0-only`
   on every source file, `<project_license>GPL-3.0-only</project_license>` in its
   metainfo, and the GPL-3.0 text in `applets/<name>/LICENSE`, packaged into the `.deb`.
4. **Keep upstream notices.** A file copied or adapted from System76 keeps its
   `Copyright … System76` line and SPDX header. Add your own copyright line below it; don't
   remove theirs.
5. **New code is yours to license.** Code written from scratch, without copying, is MPL-2.0
   by default even when written for a GPL applet, and can go into `common/`. Keep such code in
   its own files and don't paste upstream snippets into them. Reading upstream to learn which
   D-Bus interface or libcosmic widget to use is fine; copying its code is not.
6. **Dependencies follow the same rule.** `common/` may only depend on crates whose license
   lets MPL code use them (MIT, Apache-2.0, BSD, MPL-2.0, …). A GPL dependency belongs in the
   GPL applet only.

## Dependencies of the system applet

| Crate | License | Usable in `common/` |
|---|---|---|
| `dbus-settings-bindings`: `bluez-zbus`, `cosmic-dbus-networkmanager`, `cosmic-settings-daemon` | MPL-2.0 (verified) | yes |
| `dbus-settings-bindings`: `upower_dbus` | MIT (verified) | yes |
| `cosmic-settings-audio-client` / `-audio-core` (from `cosmic-settings-daemon`) | MPL-2.0 (verified) | yes |
| `cosmic-settings-daemon` itself, its `audio-server` and `varlink-server` | GPL-3.0 (verified) | no; not a dependency, the applet only talks to the running daemon |
| `cosmic-settings-upower-subscription` (battery, as upstream's battery applet) | MPL-2.0 (verified: `license` field and SPDX headers) | yes |
| `cosmic-settings-daemon-subscription` (display brightness, as upstream's battery applet) | MPL-2.0 by its SPDX headers; no `license` field, and the cosmic-settings repository as a whole is GPL-3.0 | only in the GPL applet, to be safe |
| `nmrs` 3.5 (NetworkManager, as upstream's network applet) | MIT (verified) | yes |
| `secure-string` (passwords in upstream's network model) | Unlicense (verified) | yes |
| `indexmap` | Apache-2.0 OR MIT (verified) | yes |
| `bluer` 0.17 (BlueZ, as upstream's Bluetooth applet; its `bluetoothd` feature needs `libdbus-1-dev` to build) | BSD-2-Clause (verified) | yes |
| `anyhow`, `fastrand`, `rustc-hash`, `futures` | MIT OR Apache-2.0 (verified) | yes |

Check a crate's `license` field before adding it to `common/`, and update this table.

## Data and assets

- Weather data from [Open-Meteo](https://open-meteo.com/) is CC BY 4.0. The weather applet
  shows the attribution in its popup and README.
- Icons or fonts bundled into a binary carry their own license, which must ship in the
  `.deb` (for example under `usr/share/doc/<package>/`).

This is a summary of the licenses as they apply here, not legal advice.
