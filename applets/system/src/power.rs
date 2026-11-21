// Copyright 2023 System76 <info@system76.com>
// Copyright 2026 Daniel Hoeggerl
// SPDX-License-Identifier: GPL-3.0-only

//! Lock, log out, suspend, restart and shut down. Adapted from cosmic-applet-power, with the
//! login1 proxies written for the zbus version this workspace uses.

use zbus::{Connection, proxy, zvariant::OwnedObjectPath};

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

    pub async fn perform(self) -> Result<(), String> {
        let result = async {
            match self {
                PowerAction::Lock => lock().await,
                PowerAction::LogOut => log_out().await,
                PowerAction::Suspend => manager().await?.suspend(true).await,
                PowerAction::Restart => manager().await?.reboot(true).await,
                PowerAction::Shutdown => manager().await?.power_off(true).await,
            }
        }
        .await;
        result.map_err(|err| err.to_string())
    }
}

#[proxy(
    interface = "org.freedesktop.login1.Manager",
    default_service = "org.freedesktop.login1",
    default_path = "/org/freedesktop/login1"
)]
trait Manager {
    fn get_user(&self, uid: u32) -> zbus::Result<OwnedObjectPath>;
    fn power_off(&self, interactive: bool) -> zbus::Result<()>;
    fn reboot(&self, interactive: bool) -> zbus::Result<()>;
    fn suspend(&self, interactive: bool) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.login1.User",
    default_service = "org.freedesktop.login1"
)]
trait User {
    #[zbus(property)]
    fn sessions(&self) -> zbus::Result<Vec<(String, OwnedObjectPath)>>;
}

#[proxy(
    interface = "org.freedesktop.login1.Session",
    default_service = "org.freedesktop.login1"
)]
trait Session {
    fn lock(&self) -> zbus::Result<()>;
    #[zbus(property)]
    fn class(&self) -> zbus::Result<String>;
    #[zbus(property, name = "Type")]
    fn kind(&self) -> zbus::Result<String>;
}

#[proxy(
    interface = "com.system76.CosmicSession",
    default_service = "com.system76.CosmicSession",
    default_path = "/com/system76/CosmicSession"
)]
trait CosmicSession {
    fn exit(&self) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.gnome.SessionManager",
    default_service = "org.gnome.SessionManager",
    default_path = "/org/gnome/SessionManager"
)]
trait GnomeSessionManager {
    fn logout(&self, mode: u32) -> zbus::Result<()>;
}

async fn manager() -> Result<ManagerProxy<'static>, zbus::Error> {
    let connection = Connection::system().await?;
    ManagerProxy::new(&connection).await
}

/// Locks every graphical session of this user.
async fn lock() -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let manager = ManagerProxy::new(&connection).await?;
    let uid = rustix::process::getuid().as_raw();
    let user = UserProxy::builder(&connection)
        .path(manager.get_user(uid).await?)?
        .build()
        .await?;
    let mut locked = false;
    for (_, path) in user.sessions().await? {
        let Ok(builder) = SessionProxy::builder(&connection).path(path) else {
            continue;
        };
        let Ok(session) = builder.build().await else {
            continue;
        };
        if session.class().await.is_ok_and(|c| c == "user")
            && session.kind().await.is_ok_and(|t| t != "tty")
            && session.lock().await.is_ok()
        {
            locked = true;
        }
    }
    if locked {
        Ok(())
    } else {
        Err(zbus::Error::Failure("locking the session failed".into()))
    }
}

async fn log_out() -> zbus::Result<()> {
    let connection = Connection::session().await?;
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if desktop.trim() == "pop:GNOME" {
        GnomeSessionManagerProxy::new(&connection)
            .await?
            .logout(0)
            .await
    } else {
        CosmicSessionProxy::new(&connection).await?.exit().await
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
