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

## Releasing

Follow [docs/RELEASING.md](docs/RELEASING.md). It drafts the changelog from the commits since
the applet's last tag and proposes a version. Get the user's explicit approval before bumping,
committing, tagging or pushing. Claude Code exposes the same procedure as `/release <applet>... | all`.
