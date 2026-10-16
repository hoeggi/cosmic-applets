// SPDX-License-Identifier: MPL-2.0

//! The RPC password, kept in the desktop keyring (Secret Service, e.g.
//! gnome-keyring). The login keyring is normally unlocked at login; if it is
//! locked, the keyring daemon shows its own unlock prompt.

use crate::app::APP_ID;

fn attributes<'a>(server: &'a str, username: &'a str) -> [(&'static str, &'a str); 3] {
    [
        ("application", APP_ID),
        ("server", server),
        ("username", username),
    ]
}

/// The stored password for `username` on `server` (`host:port`), if any.
pub async fn load(server: String, username: String) -> Result<Option<String>, String> {
    let keyring = open().await?;
    let items = keyring
        .search_items(&attributes(&server, &username))
        .await
        .map_err(|err| err.to_string())?;
    let Some(item) = items.first() else {
        return Ok(None);
    };
    let secret = item.secret().await.map_err(|err| err.to_string())?;
    Ok(Some(
        String::from_utf8_lossy(secret.as_bytes()).into_owned(),
    ))
}

/// Stores (or replaces) the password for `username` on `server`.
pub async fn store(server: String, username: String, password: String) -> Result<(), String> {
    let keyring = open().await?;
    let label = format!("Transmission RPC ({username}@{server})");
    keyring
        .create_item(&label, &attributes(&server, &username), password, true)
        .await
        .map_err(|err| err.to_string())
}

async fn open() -> Result<oo7::Keyring, String> {
    let keyring = oo7::Keyring::new().await.map_err(|err| err.to_string())?;
    // No-op when already unlocked; otherwise the keyring daemon prompts.
    keyring.unlock().await.map_err(|err| err.to_string())?;
    Ok(keyring)
}
