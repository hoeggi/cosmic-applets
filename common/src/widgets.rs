// SPDX-License-Identifier: MPL-2.0

//! Small building blocks for applet popups.

use cosmic::applet::padded_control;
use cosmic::iced::Color;
use cosmic::prelude::*;
use cosmic::widget::{button, divider, icon, text};

/// Horizontal divider between popup sections.
pub fn section_divider<M: 'static>() -> Element<'static, M> {
    padded_control(divider::horizontal::default())
        .padding([4, 12])
        .into()
}

/// Theme colour for a good (`true`) or bad (`false`) state.
pub fn status_color(ok: bool) -> Color {
    let theme = cosmic::theme::active();
    let cosmic = theme.cosmic();
    if ok {
        cosmic.success_color()
    } else {
        cosmic.destructive_color()
    }
    .into()
}

/// Caption text in the destructive colour.
pub fn error_text<M: 'static>(msg: impl Into<String>) -> Element<'static, M> {
    text::caption(msg.into())
        .class(cosmic::theme::Text::Color(status_color(false)))
        .into()
}

/// Extra-small icon button, e.g. for row actions.
pub fn icon_button<M: Clone + 'static>(name: &'static str) -> button::IconButton<'static, M> {
    button::icon(icon::from_name(name)).extra_small()
}
