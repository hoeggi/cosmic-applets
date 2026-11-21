// SPDX-License-Identifier: GPL-3.0-only

//! The power row's actions: lock, log out, suspend, restart and shut down. Which ones ask first
//! is ours (upstream's applet hands restart and shut down to cosmic-osd); performing them is
//! upstream's code in `crate::upstream::power`.

use crate::upstream::power as up;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Lock,
    LogOut,
    Suspend,
    Restart,
    Shutdown,
}

impl PowerAction {
    pub const ALL: [PowerAction; 5] = [
        PowerAction::Lock,
        PowerAction::LogOut,
        PowerAction::Suspend,
        PowerAction::Restart,
        PowerAction::Shutdown,
    ];

    pub fn icon(self) -> &'static str {
        match self {
            PowerAction::Lock => "system-lock-screen-symbolic",
            PowerAction::LogOut => "system-log-out-symbolic",
            PowerAction::Suspend => "system-suspend-symbolic",
            PowerAction::Restart => "system-reboot-symbolic",
            PowerAction::Shutdown => "system-shutdown-symbolic",
        }
    }

    /// Whether the action ends the session, so it asks first when confirmation is on.
    /// Lock and Suspend are harmless and act at once.
    pub fn needs_confirmation(self) -> bool {
        matches!(
            self,
            PowerAction::LogOut | PowerAction::Restart | PowerAction::Shutdown
        )
    }

    /// Upstream's helper for the action (`crate::upstream::power`).
    pub async fn perform(self) -> Result<(), String> {
        let result = match self {
            PowerAction::Lock => up::lock().await,
            PowerAction::LogOut => up::log_out().await,
            PowerAction::Suspend => up::suspend().await,
            PowerAction::Restart => up::restart().await,
            PowerAction::Shutdown => up::shutdown().await,
        };
        result.map_err(|err| err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_session_ending_actions_ask() {
        let asking: Vec<_> = PowerAction::ALL
            .into_iter()
            .filter(|a| a.needs_confirmation())
            .collect();
        assert_eq!(
            asking,
            [
                PowerAction::LogOut,
                PowerAction::Restart,
                PowerAction::Shutdown
            ]
        );
    }
}
