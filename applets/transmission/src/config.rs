// SPDX-License-Identifier: MPL-2.0

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

/// Stored per field as RON under `~/.config/cosmic/<APP_ID>/v1/<field>`.
/// The password is not here; it lives in the keyring (see `keyring.rs`).
#[derive(Debug, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    /// Host name or IP of the Transmission daemon; empty = not set up yet.
    pub host: String,
    pub port: u16,
    pub rpc_path: String,
    /// Empty when the daemon does not require a login.
    pub username: String,
    /// Started when the list is clicked.
    pub client_command: Vec<String>,
    /// Poll interval while the popup is closed.
    pub poll_secs: u64,
    pub notify_finished: bool,
    pub notify_errors: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 9091,
            rpc_path: "/transmission/rpc".into(),
            username: String::new(),
            client_command: vec![
                "flatpak".into(),
                "run".into(),
                "io.github.TransmissionRemoteGtk".into(),
            ],
            poll_secs: 15,
            notify_finished: true,
            notify_errors: true,
        }
    }
}

impl Config {
    pub fn is_configured(&self) -> bool {
        !self.host.trim().is_empty()
    }

    /// `host:port`, also the keyring key for the password.
    pub fn server(&self) -> String {
        format!("{}:{}", self.host.trim(), self.port)
    }

    pub fn url(&self) -> String {
        let path = self.rpc_path.trim();
        let slash = if path.starts_with('/') { "" } else { "/" };
        format!("http://{}{slash}{path}", self.server())
    }

    /// The fields that require a new connection when they change.
    pub fn connection(&self) -> (String, String, String) {
        (self.url(), self.username.clone(), self.server())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url() {
        let config = Config {
            host: " 192.168.0.24 ".into(),
            rpc_path: "transmission/rpc".into(),
            ..Default::default()
        };
        assert_eq!(config.url(), "http://192.168.0.24:9091/transmission/rpc");
        assert_eq!(config.server(), "192.168.0.24:9091");
        assert!(!Config::default().is_configured());
    }
}
