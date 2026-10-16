// SPDX-License-Identifier: MPL-2.0

//! The panel ring: overall download progress drawn around the active count.
//!
//! The ring is a single-colour SVG marked symbolic, so the panel repaints it in
//! its own foreground colour; the count is a text widget stacked on top (SVG
//! text would need fonts at rasterisation time).

use crate::model::Panel;
use cosmic::widget::icon;
use std::f32::consts::TAU;

const RADIUS: f32 = 7.0;
const STROKE: &str = r#"fill="none" stroke="currentColor" stroke-width="1.3""#;

pub fn handle(panel: Panel) -> icon::Handle {
    let mut handle = icon::from_svg_bytes(svg(panel).into_bytes());
    handle.symbolic = true;
    handle
}

/// The text inside the ring, and whether it is drawn dimmed.
pub fn label(panel: Panel) -> (String, bool) {
    match panel {
        Panel::Downloading { count, .. } if count > 99 => ("99+".into(), false),
        Panel::Downloading { count, .. } => (count.to_string(), false),
        Panel::Idle => ("0".into(), true),
        Panel::Error => ("!".into(), false),
        Panel::Offline => ("–".into(), true),
    }
}

fn svg(panel: Panel) -> String {
    let track = |opacity: &str, dash: &str| {
        format!(r#"<circle cx="8" cy="8" r="{RADIUS}" {STROKE} opacity="{opacity}"{dash}/>"#)
    };
    let body = match panel {
        Panel::Downloading { percent, .. } => format!("{}{}", track(".3", ""), arc(percent)),
        Panel::Idle => track("1", ""),
        Panel::Error => track(".3", ""),
        Panel::Offline => track(".5", r#" stroke-dasharray="1.6 1.6""#),
    };
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">{body}</svg>"#)
}

/// Clockwise arc from 12 o'clock covering `percent` of the ring.
fn arc(percent: u8) -> String {
    let percent = f32::from(percent.min(100));
    if percent >= 99.9 {
        return format!(r#"<circle cx="8" cy="8" r="{RADIUS}" {STROKE}/>"#);
    }
    // Keep a visible dot for tiny values.
    let angle = TAU * percent.max(1.0) / 100.0;
    let (x, y) = (8.0 + RADIUS * angle.sin(), 8.0 - RADIUS * angle.cos());
    let large = u8::from(percent > 50.0);
    format!(
        r#"<path d="M8 1A{RADIUS} {RADIUS} 0 {large} 1 {x:.2} {y:.2}" {STROKE} stroke-linecap="round"/>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svgs_are_well_formed() {
        for panel in [
            Panel::Offline,
            Panel::Error,
            Panel::Idle,
            Panel::Downloading {
                count: 1,
                percent: 0,
            },
            Panel::Downloading {
                count: 3,
                percent: 37,
            },
            Panel::Downloading {
                count: 12,
                percent: 100,
            },
        ] {
            let svg = svg(panel);
            assert!(
                svg.starts_with("<svg") && svg.ends_with("</svg>"),
                "{panel:?}"
            );
            assert_eq!(
                svg.matches('<').count(),
                svg.matches('>').count(),
                "{panel:?}"
            );
        }
    }

    #[test]
    fn labels() {
        let dl = |count| Panel::Downloading { count, percent: 5 };
        assert_eq!(label(dl(2)), ("2".into(), false));
        assert_eq!(label(dl(140)), ("99+".into(), false));
        assert_eq!(label(Panel::Offline), ("–".into(), true));
    }

    #[test]
    fn arc_uses_large_flag_past_half() {
        assert!(arc(25).contains(" 0 1 "));
        assert!(arc(75).contains(" 1 1 "));
    }
}
