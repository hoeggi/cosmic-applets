// SPDX-License-Identifier: MPL-2.0

//! The applet's popup window.

use cosmic::iced::platform_specific::shell::wayland::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::{Limits, window::Id};
use cosmic::prelude::*;

/// Opens the popup, or closes it when it is open. `popup` tracks the open
/// popup's window ID; check `popup.is_some()` afterwards to see which happened.
pub fn toggle<M: 'static>(core: &cosmic::Core, popup: &mut Option<Id>) -> Task<cosmic::Action<M>> {
    if let Some(id) = popup.take() {
        return destroy_popup(id);
    }
    let Some(parent) = core.main_window_id() else {
        return Task::none();
    };
    let id = Id::unique();
    *popup = Some(id);
    let mut settings = core.applet.get_popup_settings(parent, id, None, None, None);
    settings.positioner.size_limits = Limits::NONE
        .max_width(400.0)
        .min_width(320.0)
        .min_height(1.0)
        .max_height(1080.0);
    get_popup(settings)
}

/// Forget the popup once the compositor closed it (e.g. click outside).
pub fn closed(popup: &mut Option<Id>, id: Id) {
    if *popup == Some(id) {
        *popup = None;
    }
}
