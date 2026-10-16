// SPDX-License-Identifier: MPL-2.0

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

/// A user-defined command shown in the popup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEntry {
    pub id: u64,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub in_terminal: bool,
}

/// Stored per field as RON under `~/.config/cosmic/<APP_ID>/v1/<field>`.
#[derive(Debug, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    pub commands: Vec<CommandEntry>,
    /// Terminal launcher prefix; the command is appended as `sh -c ...`.
    pub terminal: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            terminal: vec!["cosmic-term".into(), "--".into()],
        }
    }
}

impl Config {
    pub fn next_id(&self) -> u64 {
        self.commands
            .iter()
            .map(|c| c.id)
            .max()
            .map_or(1, |id| id + 1)
    }

    pub fn get(&self, id: u64) -> Option<&CommandEntry> {
        self.commands.iter().find(|c| c.id == id)
    }
}
