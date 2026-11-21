# AGENTS.md

Guidance for coding agents (Claude Code, Codex, Cursor, Copilot, …) working in this repository.
See [README.md](README.md) for what the applets are and how the repo is laid out.

## Building

- Do all builds, checks and tests through `make`. These run inside the Podman builder
  container (`container/Containerfile`). Don't install Rust or other build tools on the host,
  and don't run host `cargo`.
- `make check test` is the gate before any commit: `cargo fmt --check`, clippy with
  `-D warnings`, and unit tests. CI (`.github/workflows/ci.yml`) runs the same checks.
- `make build` / `make deb` / `make run APPLET=<name>`. `make install` and `make uninstall`
  use sudo, so leave them to the user.

## Code

- Each applet is its own crate in `applets/<name>` (binary `cosmic-applet-<name>`). Shared
  helpers go in `common/` (`cosmic-applet-common`) once a second applet needs them.
- Match the surrounding style: libcosmic `Application` structure, `fl!()` for user-visible
  strings (add them to `i18n/en/*.ftl`), `tracing` for logs, and SPDX headers.
- Run git non-interactively (`git --no-pager …`).

## Licensing

Read [docs/LICENSING.md](docs/LICENSING.md) before adding a crate, a dependency, or code from
another project. The applets are MPL-2.0 by default; an applet that copies or adapts code from
System76's `cosmic-applets` is GPL-3.0-only. The hard rules:

- **Never put GPL code in `common/`.** Nothing copied or adapted from System76's applets or any
  other GPL source goes into `common/`, however small. `common/` is linked into every applet,
  so GPL code there would make all of them GPL. GPL-derived code stays inside its GPL applet.
- Never copy code from a GPL applet or GPL source into an MPL applet. The other direction
  (MPL code used in a GPL applet) is fine.
- `common/` may only depend on MPL-compatible crates (MIT, Apache-2.0, BSD, MPL-2.0, …).
- A GPL applet sets `license = "GPL-3.0-only"` in its `Cargo.toml`, uses
  `// SPDX-License-Identifier: GPL-3.0-only`, sets `<project_license>` in its metainfo, and
  ships the GPL-3.0 text. Copied files keep System76's copyright line and SPDX header.
- Code written from scratch is MPL-2.0 and may go into `common/`, but only if no upstream
  code was pasted into it. When in doubt, keep it in the GPL applet and ask.

## Releasing

Follow [docs/RELEASING.md](docs/RELEASING.md). It drafts the changelog from the commits since
the applet's last tag and proposes a version. Get the user's explicit approval before bumping,
committing, tagging or pushing. Claude Code exposes the same procedure as `/release <applet>... | all`.
