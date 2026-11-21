// SPDX-License-Identifier: GPL-3.0-only

//! View building blocks shared by the main popup and the detail pages.

use cosmic::applet::padded_control;
use cosmic::iced::{Alignment, Length};
use cosmic::prelude::*;
use cosmic::widget::{self, button, icon, text};
use cosmic_applet_common::widgets::icon_button;

/// Header of a detail page: back arrow, title, optional subtitle and trailing actions.
pub fn page_header<'a, M: Clone + 'static>(
    title: String,
    subtitle: Option<String>,
    back: M,
    actions: Vec<Element<'a, M>>,
) -> Element<'a, M> {
    let mut info = widget::Column::new().push(text::heading(title));
    if let Some(subtitle) = subtitle {
        info = info.push(text::caption(subtitle));
    }
    let mut row = widget::Row::new()
        .push(icon_button("go-previous-symbolic").on_press(back))
        .push(info.width(Length::Fill))
        .spacing(8)
        .align_y(Alignment::Center);
    for action in actions {
        row = row.push(action);
    }
    padded_control(row).padding([4, 12]).into()
}

/// Bold section title inside a page, with an optional control on the right (e.g. a toggler).
pub fn section_title<'a, M: 'static>(
    title: String,
    right: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let mut row = widget::Row::new()
        .push(text::heading(title).width(Length::Fill))
        .align_y(Alignment::Center);
    if let Some(right) = right {
        row = row.push(right);
    }
    padded_control(row).padding([6, 16, 2, 16]).into()
}

/// Small muted group label, e.g. "Other networks".
pub fn group_label<M: 'static>(label: String) -> Element<'static, M> {
    padded_control(text::caption_heading(label))
        .padding([6, 16, 0, 16])
        .into()
}

/// Tile corner radius, as in the mockups.
const TILE_RADIUS: f32 = 12.0;

/// One quick-settings tile: one rounded background (the accent button colours when on, the
/// standard button colours when off) holding two click areas: the body toggles, the chevron (if
/// any) opens a page, with a thin divider between them.
/// One line, cut with "…" at the end: tiles keep one height whatever the text.
fn one_line<'a>(
    t: cosmic::widget::Text<'a, cosmic::Theme>,
) -> cosmic::widget::Text<'a, cosmic::Theme> {
    use cosmic::iced::advanced::text::{Ellipsize, EllipsizeHeightLimit, Wrapping};
    t.wrapping(Wrapping::None)
        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
}

pub fn tile<'a, M: Clone + 'static>(
    icon_name: &'a str,
    name: String,
    status: String,
    on: bool,
    toggle: Option<M>,
    open: Option<M>,
) -> Element<'a, M> {
    let body = widget::Row::new()
        .push(icon::from_name(icon_name).size(16).symbolic(true).icon())
        .push(
            widget::Column::new()
                .push(one_line(text::body(name).font(cosmic::font::semibold())))
                .push(one_line(text::caption(status)))
                .width(Length::Fill),
        )
        .spacing(10)
        .align_y(Alignment::Center);
    let has_chevron = open.is_some();
    let body_corners = if has_chevron {
        [TILE_RADIUS, 0.0, 0.0, TILE_RADIUS]
    } else {
        [TILE_RADIUS; 4]
    };
    let mut row = widget::Row::new().push(
        button::custom(body)
            .class(tile_area(on, body_corners))
            .padding([8, 10])
            .width(Length::Fill)
            .on_press_maybe(toggle),
    );
    if let Some(open) = open {
        row = row
            .push(
                widget::container(widget::Space::new())
                    .width(Length::Fixed(1.0))
                    .height(Length::Fixed(28.0))
                    .class(cosmic::theme::Container::custom(move |theme| {
                        let mut color: cosmic::iced::Color = tile_colors(theme, on).1.into();
                        color.a = 0.35;
                        widget::container::Style {
                            background: Some(color.into()),
                            ..Default::default()
                        }
                    })),
            )
            .push(
                button::custom(
                    icon::from_name("go-next-symbolic")
                        .size(16)
                        .symbolic(true)
                        .icon(),
                )
                .class(tile_area(on, [0.0, TILE_RADIUS, TILE_RADIUS, 0.0]))
                .padding([16, 8])
                .on_press(open),
            );
    }
    widget::container(row.align_y(Alignment::Center))
        .width(Length::Fill)
        .class(cosmic::theme::Container::custom(move |theme| {
            let (base, on_color) = tile_colors(theme, on);
            widget::container::Style {
                background: Some(cosmic::iced::Color::from(base).into()),
                text_color: Some(on_color.into()),
                icon_color: Some(on_color.into()),
                border: cosmic::iced::Border {
                    radius: TILE_RADIUS.into(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }))
        .into()
}

/// The tile's background and foreground: the accent button colours when on, the standard
/// button colours when off, so both themes work.
fn tile_colors(
    theme: &cosmic::Theme,
    on: bool,
) -> (
    cosmic::cosmic_theme::palette::Srgba,
    cosmic::cosmic_theme::palette::Srgba,
) {
    let cosmic = theme.cosmic();
    let component = if on {
        &cosmic.accent_button
    } else {
        &cosmic.button
    };
    (component.base, component.on)
}

/// A click area inside a tile: transparent over the tile's background, with the component's
/// hover and pressed colours.
fn tile_area(on: bool, corners: [f32; 4]) -> cosmic::theme::Button {
    let style = move |theme: &cosmic::Theme,
                      state: fn(
        &cosmic::cosmic_theme::Component,
    ) -> Option<cosmic::cosmic_theme::palette::Srgba>| {
        let cosmic = theme.cosmic();
        let component = if on {
            &cosmic.accent_button
        } else {
            &cosmic.button
        };
        button::Style {
            background: state(component).map(|c| cosmic::iced::Color::from(c).into()),
            border_radius: corners.into(),
            text_color: Some(component.on.into()),
            icon_color: Some(component.on.into()),
            ..button::Style::new()
        }
    };
    cosmic::theme::Button::Custom {
        active: Box::new(move |_, theme| style(theme, |_| None)),
        disabled: Box::new(move |theme| style(theme, |_| None)),
        hovered: Box::new(move |_, theme| style(theme, |c| Some(c.hover))),
        pressed: Box::new(move |_, theme| style(theme, |c| Some(c.pressed))),
    }
}

/// Two tiles per row.
pub fn tile_grid<'a, M: 'static>(tiles: Vec<Element<'a, M>>) -> Element<'a, M> {
    let mut column = widget::Column::new().spacing(8);
    let mut iter = tiles.into_iter();
    while let Some(first) = iter.next() {
        let mut row = widget::Row::new().push(first).spacing(8);
        row = match iter.next() {
            Some(second) => row.push(second),
            None => row.push(widget::space::horizontal().width(Length::Fill)),
        };
        column = column.push(row);
    }
    padded_control(column).padding([8, 12]).into()
}

/// Icon button (e.g. mute), slider, and the value on the right.
pub fn slider_row<'a, M: Clone + 'static>(
    icon_name: &'static str,
    on_icon: Option<M>,
    value: u32,
    label: String,
    on_change: impl Fn(u32) -> M + 'a,
    on_release: Option<M>,
) -> Element<'a, M> {
    let mut slider = widget::slider(0..=100, value, on_change).width(Length::Fill);
    if let Some(release) = on_release {
        slider = slider.on_release(release);
    }
    let row = widget::Row::new()
        .push(icon_button(icon_name).on_press_maybe(on_icon))
        .push(slider)
        .push(
            widget::container(text::body(label))
                .width(Length::Fixed(44.0))
                .align_x(Alignment::End),
        )
        .spacing(8)
        .align_y(Alignment::Center);
    padded_control(row).padding([2, 16, 2, 10]).into()
}

/// A selectable row: icon (none if `icon_name` is empty), name, subtitle, trailing widgets and a check mark when selected.
pub fn choice_row<'a, M: Clone + 'static>(
    icon_name: &'a str,
    name: String,
    subtitle: Option<String>,
    selected: bool,
    trailing: Vec<Element<'a, M>>,
    on_press: Option<M>,
) -> Element<'a, M> {
    let mut info = widget::Column::new().push(text::body(name));
    if let Some(subtitle) = subtitle {
        info = info.push(text::caption(subtitle));
    }
    let mut row = widget::Row::new().spacing(12).align_y(Alignment::Center);
    // An empty name leaves the icon out (e.g. power profiles, which have no themed icons).
    if !icon_name.is_empty() {
        row = row.push(icon::from_name(icon_name).size(16).symbolic(true).icon());
    }
    row = row.push(info.width(Length::Fill));
    for item in trailing {
        row = row.push(item);
    }
    row = row.push(
        widget::container(if selected {
            Element::from(
                icon::from_name("object-select-symbolic")
                    .size(16)
                    .symbolic(true)
                    .icon(),
            )
        } else {
            Element::from(
                widget::Space::new()
                    .width(Length::Fixed(16.0))
                    .height(Length::Fixed(16.0)),
            )
        })
        .width(Length::Fixed(16.0)),
    );
    cosmic::applet::menu_button(row)
        .on_press_maybe(on_press)
        .into()
}

/// The last row of a detail page: opens a COSMIC Settings page.
pub fn settings_link<'a, M: Clone + 'static>(label: String, on_press: M) -> Element<'a, M> {
    cosmic::applet::menu_button(text::body(label))
        .on_press(on_press)
        .into()
}

/// Muted explanation, e.g. when a backend is unavailable.
pub fn note<M: 'static>(message: String) -> Element<'static, M> {
    padded_control(text::caption(message))
        .padding([4, 16])
        .into()
}
