// SPDX-License-Identifier: MPL-2.0

//! Desktop notifications.

/// Sends a desktop notification without blocking the UI; failures are logged.
pub async fn send(app_name: &'static str, icon: &'static str, summary: String, body: String) {
    let result = tokio::task::spawn_blocking(move || {
        notify_rust::Notification::new()
            .appname(app_name)
            .icon(icon)
            .summary(&summary)
            .body(&body)
            .show()
            .map(|_| ())
    })
    .await;

    match result {
        Ok(Err(err)) => tracing::warn!(%err, "failed to send notification"),
        Err(err) => tracing::warn!(%err, "notification task panicked"),
        Ok(Ok(())) => {}
    }
}
