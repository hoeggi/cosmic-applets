// SPDX-License-Identifier: MPL-2.0

//! cosmic-config plumbing with error logging.

use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::Subscription;

/// Opens the config for `app_id` and loads it, falling back to defaults (per
/// field) on errors. The returned context is `None` if the config is unavailable.
pub fn load<C: CosmicConfigEntry + Default>(app_id: &str) -> (C, Option<cosmic_config::Config>) {
    let ctx = cosmic_config::Config::new(app_id, C::VERSION)
        .inspect_err(|err| tracing::error!(%err, "cannot open config"))
        .ok();
    let config = ctx
        .as_ref()
        .map(|ctx| {
            C::get_entry(ctx).unwrap_or_else(|(errors, config)| {
                for err in errors.iter().filter(|e| e.is_err()) {
                    tracing::error!(%err, "error loading config");
                }
                config
            })
        })
        .unwrap_or_default();
    (config, ctx)
}

/// Emits the config whenever it changes on disk (also for hand edits).
pub fn watch<C>(core: &cosmic::Core, app_id: &'static str) -> Subscription<C>
where
    C: CosmicConfigEntry + Send + Sync + Default + Clone + PartialEq + 'static,
{
    core.watch_config::<C>(app_id).map(|update| {
        for err in update.errors.iter().filter(|e| e.is_err()) {
            tracing::error!(%err, "config update error");
        }
        update.config
    })
}

/// Writes through a generated `set_<field>` setter (which also updates `config`).
pub fn save<C>(
    config: &mut C,
    ctx: Option<&cosmic_config::Config>,
    set: impl FnOnce(&mut C, &cosmic_config::Config) -> Result<bool, cosmic_config::Error>,
) {
    if let Some(ctx) = ctx
        && let Err(err) = set(config, ctx)
    {
        tracing::error!(%err, "failed to save config");
    }
}
