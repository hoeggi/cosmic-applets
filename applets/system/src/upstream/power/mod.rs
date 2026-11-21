// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from pop-os/cosmic-applets cosmic-applet-power/src/lib.rs at ecb6b61: the "System
//! helpers" (restart, shutdown, suspend, lock, log out), with `cosmic_session.rs` and
//! `session_manager.rs` beside it verbatim. The applet struct and its UI are left out.
//!
//! system-applet: the helpers are `pub` so our power row can call them; upstream's applet asks
//! cosmic-osd to confirm restart and shut down, our popup asks itself (see `crate::power`).

use cosmic_login1_zbus::{
    manager::ManagerProxy,
    session::{SessionClass, SessionProxy, SessionType},
    user::UserProxy,
};
use rustix::process::getuid;
use zbus::Connection;

pub mod cosmic_session;
pub mod session_manager;

use self::{cosmic_session::CosmicSessionProxy, session_manager::SessionManagerProxy};

// ### System helpers

pub async fn restart() -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let manager_proxy = ManagerProxy::new(&connection).await?;
    manager_proxy.reboot(true).await
}

pub async fn shutdown() -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let manager_proxy = ManagerProxy::new(&connection).await?;
    manager_proxy.power_off(true).await
}

pub async fn suspend() -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let manager_proxy = ManagerProxy::new(&connection).await?;
    manager_proxy.suspend(true).await
}

pub async fn lock() -> zbus::Result<()> {
    let connection = Connection::system().await?;
    let manager_proxy = ManagerProxy::new(&connection).await?;
    // Get the session this current process is running in
    let our_uid = getuid().as_raw() as u32;
    let user_path = manager_proxy.get_user(our_uid).await?;
    let user = UserProxy::builder(&connection)
        .path(user_path)
        .build()
        .await?;
    // Lock all non-TTY sessions of this user
    let sessions = user.sessions().await?;
    let mut locked_successfully = false;
    for (_, session_path) in sessions {
        let Ok(session) = SessionProxy::builder(&connection)
            .path(session_path)
            .build()
            .await
        else {
            continue;
        };

        if session.class().await == Ok(SessionClass::User)
            && session.type_().await? != SessionType::TTY
            && session.lock().await.is_ok()
        {
            locked_successfully = true;
        }
    }

    if locked_successfully {
        Ok(())
    } else {
        Err(zbus::Error::Failure("locking session failed".to_string()))
    }
}

pub async fn log_out() -> zbus::Result<()> {
    let session_type = std::env::var("XDG_CURRENT_DESKTOP").ok();
    let connection = Connection::session().await?;
    if let Some("pop:GNOME") = session_type.as_ref().map(|s| s.trim()) {
        let manager_proxy = SessionManagerProxy::new(&connection).await?;
        manager_proxy.logout(0).await?;
    } else {
        // By default assume COSMIC
        let cosmic_session = CosmicSessionProxy::new(&connection).await?;
        cosmic_session.exit().await?;
    }
    Ok(())
}
