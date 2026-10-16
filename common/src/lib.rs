// SPDX-License-Identifier: MPL-2.0

//! Helpers shared by the COSMIC applets in this workspace: logging, cosmic-config
//! plumbing, the popup window, desktop notifications and a few popup widgets.

pub mod config;
pub mod notify;
pub mod popup;
pub mod widgets;

/// Logs to stderr, which cosmic-panel forwards to the user journal
/// (`journalctl --user -t cosmic-panel`, lines prefixed with the app ID).
///
/// `crate_target` is the applet's crate name in snake case; it logs at `info`,
/// everything else at `warn`, unless `RUST_LOG` says otherwise.
pub fn init_logging(crate_target: &str) {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("warn,{crate_target}=info").into()),
        )
        .with_writer(std::io::stderr)
        .init();
}
