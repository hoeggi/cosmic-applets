# COSMIC applets

Panel applets for the [COSMIC](https://system76.com/cosmic) desktop (Pop!_OS 24.04).

| Applet | Package | What it does |
|---|---|---|
| [commands](applets/commands) | `cosmic-applet-commands` | Run configurable shell commands from the panel, with status, output and history |
| [headsetcontrol](applets/headsetcontrol) | `cosmic-applet-headsetcontrol` | Headset battery as a ring gauge, chatmix and settings via [HeadsetControl](https://github.com/Sapd/HeadsetControl) |

## Install

Download the `.deb` from the [Releases](../../releases) page and install it with apt:

    sudo apt install ./cosmic-applet-headsetcontrol_0.1.0-1_amd64.deb

Then add the applet in **Settings → Desktop → Panel → Applets**.

## Layout

    Cargo.toml              workspace: shared dependencies, one Cargo.lock / target/
    common/                 cosmic-applet-common: logging, config, popup, notifications, widgets
    applets/<name>/         one crate per applet (binary + i18n + resources + deb metadata)
    container/Containerfile builder image (Ubuntu 24.04 + Rust + cargo-deb)
    .github/workflows/      ci.yml (fmt, clippy, tests), release.yml (tag → .deb → GitHub release)

## Build

Everything runs in a Podman container (`ENGINE=docker` works too), so no Rust toolchain is
needed on the host. The `cosmic-applet-builder` image is reused across checkouts and only
rebuilt when `container/Containerfile` changes.

    make build                          # all applets (APPLET=<name> for one)
    make check test                     # fmt + clippy, unit tests
    make run APPLET=headsetcontrol      # standalone window
    make deb                            # target/debian/*.deb
    make install APPLET=headsetcontrol  # builds the .deb and installs it via apt
    make reload-panel                   # restart cosmic-panel to pick up a new binary
    make logs                           # applet log lines from the panel's journal

## Release

Each applet is versioned and released on its own. Bump `version` in
`applets/<name>/Cargo.toml`, commit, then tag `<name>-v<version>`:

    git tag headsetcontrol-v0.2.0
    git push origin headsetcontrol-v0.2.0

The release workflow checks that the tag matches the crate version, builds the `.deb`, and
publishes a GitHub release with notes since that applet's previous tag.

## Adding an applet

Copy an existing `applets/<name>` directory, then rename the crate, `APP_ID`, i18n file,
and the `[package.metadata.deb]` asset paths. The workspace, Makefile and workflows pick
up new applets automatically.
