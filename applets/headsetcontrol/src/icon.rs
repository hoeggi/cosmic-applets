// SPDX-License-Identifier: MPL-2.0

//! The panel icon: a small headset inside a ring gauge showing the battery level.
//!
//! Drawn as a single-colour SVG and marked symbolic, so the panel repaints it in
//! its own foreground colour (follows light/dark like the system icons).

use cosmic::widget::icon;
use std::f32::consts::TAU;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelState {
    /// Battery level 0–100.
    Level(u8),
    /// Charging, with the level when the device reports it.
    Charging(Option<u8>),
    /// No headset, or the headset is switched off.
    Offline,
    /// headsetcontrol is missing or failing.
    Error,
}

const RADIUS: f32 = 6.8;
const HEADSET: &str = r#"<g transform="translate(4 4) scale(.5)"><path d="M2.5 9V8a5.5 5.5 0 0 1 11 0v1" fill="none" stroke="currentColor" stroke-width="2"/><rect x="1" y="8" width="3.5" height="6" rx="1.2" fill="currentColor"/><rect x="11.5" y="8" width="3.5" height="6" rx="1.2" fill="currentColor"/></g>"#;
const TRACK: &str = r#"<circle cx="8" cy="8" r="6.8" fill="none" stroke="currentColor" stroke-width="1.4" opacity=".3"/>"#;
const BOLT: &str = r#"<path d="M8.9 4 6.3 8.4h1.9l-.9 3.6 2.6-4.4H8z" fill="currentColor"/>"#;

pub fn handle(state: PanelState) -> icon::Handle {
    let mut handle = icon::from_svg_bytes(svg(state).into_bytes());
    handle.symbolic = true;
    handle
}

fn svg(state: PanelState) -> String {
    let body = match state {
        PanelState::Level(level) => format!("{HEADSET}{TRACK}{}", arc(level)),
        PanelState::Charging(level) => {
            format!("{BOLT}{TRACK}{}", level.map(arc).unwrap_or_default())
        }
        PanelState::Offline => masked(
            &format!("{HEADSET}{TRACK}"),
            r#"<path d="M1.5 1.5l13 13" stroke="black" stroke-width="3.6"/>"#,
            r#"<path d="M1.5 1.5l13 13" stroke="currentColor" stroke-width="1.6"/>"#,
        ),
        PanelState::Error => masked(
            &format!("{HEADSET}{TRACK}"),
            r#"<circle cx="12" cy="12" r="4.6" fill="black"/>"#,
            r#"<path d="M10 10l4 4M14 10l-4 4" stroke="currentColor" stroke-width="1.5"/>"#,
        ),
    };
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">{body}</svg>"#)
}

/// Clockwise arc from 12 o'clock covering `level` percent of the ring.
fn arc(level: u8) -> String {
    let level = f32::from(level.min(100));
    if level >= 99.9 {
        return r#"<circle cx="8" cy="8" r="6.8" fill="none" stroke="currentColor" stroke-width="1.4"/>"#.into();
    }
    let angle = TAU * level / 100.0;
    let (x, y) = (8.0 + RADIUS * angle.sin(), 8.0 - RADIUS * angle.cos());
    let large = u8::from(level > 50.0);
    format!(
        r#"<path d="M8 1.2A6.8 6.8 0 {large} 1 {x:.2} {y:.2}" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>"#
    )
}

/// `body` with `cut` punched out of it, then `overlay` drawn on top.
fn masked(body: &str, cut: &str, overlay: &str) -> String {
    format!(
        r#"<defs><mask id="m"><rect width="16" height="16" fill="white"/>{cut}</mask></defs><g mask="url(#m)">{body}</g>{overlay}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svgs_are_well_formed() {
        for state in [
            PanelState::Level(0),
            PanelState::Level(37),
            PanelState::Level(62),
            PanelState::Level(100),
            PanelState::Charging(None),
            PanelState::Charging(Some(45)),
            PanelState::Offline,
            PanelState::Error,
        ] {
            let svg = svg(state);
            assert!(
                svg.starts_with("<svg") && svg.ends_with("</svg>"),
                "{state:?}"
            );
            assert_eq!(
                svg.matches('<').count(),
                svg.matches('>').count(),
                "{state:?}"
            );
        }
    }

    #[test]
    fn arc_uses_large_flag_past_half() {
        assert!(arc(25).contains(" 0 1 "));
        assert!(arc(75).contains(" 1 1 "));
    }
}
