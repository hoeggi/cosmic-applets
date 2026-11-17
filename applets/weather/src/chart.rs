// SPDX-License-Identifier: MPL-2.0

//! The 24-hour chart: temperature curve, hourly rain bars and night shading, drawn as an SVG.
//!
//! The chart shares its grid with the table below it: [`COLUMNS`] equal columns, column `j`
//! centred on hour `j * STEP`. The popup draws it at the exact width of the table (see
//! `responsive` in app.rs), so dots, guides and table columns line up at any popup width without
//! stretching. Colours come from the theme and are baked in (the panel's single-colour symbolic
//! trick can't do several).

use crate::model::{COLUMNS, STEP, Timeline};
use cosmic::iced::Color;
use std::fmt::Write;

pub const HEIGHT: f64 = 84.0;
const TOP: f64 = 10.0;
const BASELINE: f64 = HEIGHT - 4.0;
/// The curve stays above this, leaving the bottom for the rain bars.
const CURVE_BOTTOM: f64 = BASELINE - 16.0;
/// Bar height for 1 mm of rain in an hour, and the cap.
const BAR_PER_MM: f64 = 22.0;
const BAR_MAX: f64 = 34.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colors {
    pub accent: Color,
    pub foreground: Color,
    pub background: Color,
}

impl Colors {
    pub fn from_theme(theme: &cosmic::Theme) -> Self {
        let cosmic = theme.cosmic();
        Self {
            accent: cosmic.accent_color().into(),
            foreground: cosmic.background(theme.transparent).on.into(),
            background: cosmic.background(theme.transparent).base.into(),
        }
    }
}

/// x of a position given in hours from now, for columns `column` wide.
fn x_at(column: f64, hours: f64) -> f64 {
    column * (hours / STEP as f64 + 0.5)
}

/// The chart `width` pixels wide. `mm_per_unit` converts the precipitation unit to mm (1, or
/// 25.4 for inch).
pub fn svg(timeline: &Timeline, colors: &Colors, mm_per_unit: f64, width: f64) -> String {
    let column = width / COLUMNS as f64;
    let x = |hours: f64| x_at(column, hours);
    let (accent, fg, bg) = (
        hex(colors.accent),
        hex(colors.foreground),
        hex(colors.background),
    );
    let mut out = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {HEIGHT}" width="{width}" height="{HEIGHT}">"#
    );

    for (from, to) in &timeline.nights {
        let (a, b) = (x(*from).max(0.0), x(*to).min(width));
        let _ = write!(
            out,
            r#"<rect x="{a:.1}" y="0" width="{:.1}" height="{BASELINE}" fill="{fg}" fill-opacity=".06"/>"#,
            b - a
        );
    }
    let _ = write!(
        out,
        r#"<line x1="0" x2="{width}" y1="{BASELINE}" y2="{BASELINE}" stroke="{fg}" stroke-opacity=".2"/>"#
    );

    let temps: Vec<(f64, f64)> = timeline
        .temperatures
        .iter()
        .enumerate()
        .filter_map(|(k, t)| t.map(|t| (k as f64, t)))
        .collect();
    let (lo, hi) = temps.iter().fold((f64::MAX, f64::MIN), |(lo, hi), (_, t)| {
        (lo.min(*t), hi.max(*t))
    });
    let pad = ((hi - lo) * 0.1).max(1.0);
    let y = |t: f64| CURVE_BOTTOM - (t - (lo - pad)) / (hi - lo + 2.0 * pad) * (CURVE_BOTTOM - TOP);

    // Guides from each column's point down to the baseline.
    for j in 0..COLUMNS {
        if let Some(Some(t)) = timeline.temperatures.get(j * STEP) {
            let cx = x((j * STEP) as f64);
            let _ = write!(
                out,
                r#"<line x1="{cx:.1}" x2="{cx:.1}" y1="{:.1}" y2="{BASELINE}" stroke="{fg}" stroke-opacity=".25" stroke-dasharray="2 2"/>"#,
                y(*t)
            );
        }
    }

    let bar_width = column / STEP as f64 * 0.6;
    for (centre, amount) in &timeline.precipitation {
        let height = (amount * mm_per_unit * BAR_PER_MM).clamp(1.5, BAR_MAX);
        let _ = write!(
            out,
            r#"<rect x="{:.1}" y="{:.1}" width="{bar_width:.1}" height="{height:.1}" rx="1.5" fill="{fg}" fill-opacity=".45"/>"#,
            x(*centre) - bar_width / 2.0,
            BASELINE - height
        );
    }

    // The curve, split where values are missing.
    for segment in segments(&temps) {
        let points: Vec<String> = segment
            .iter()
            .map(|(k, t)| format!("{:.1},{:.1}", x(*k), y(*t)))
            .collect();
        if let (Some((k0, _)), Some((k1, _))) = (segment.first(), segment.last()) {
            let _ = write!(
                out,
                r#"<polygon points="{:.1},{BASELINE} {} {:.1},{BASELINE}" fill="{accent}" fill-opacity=".14"/>"#,
                x(*k0),
                points.join(" "),
                x(*k1)
            );
        }
        let _ = write!(
            out,
            r#"<polyline points="{}" fill="none" stroke="{accent}" stroke-width="2" stroke-linejoin="round"/>"#,
            points.join(" ")
        );
    }

    for j in 0..COLUMNS {
        if let Some(Some(t)) = timeline.temperatures.get(j * STEP) {
            let (cx, cy) = (x((j * STEP) as f64), y(*t));
            let _ = if j == 0 {
                write!(
                    out,
                    r#"<circle cx="{cx:.1}" cy="{cy:.1}" r="3.5" fill="{accent}"/>"#
                )
            } else {
                write!(
                    out,
                    r#"<circle cx="{cx:.1}" cy="{cy:.1}" r="2.6" fill="{bg}" stroke="{accent}" stroke-width="2"/>"#
                )
            };
        }
    }
    out.push_str("</svg>");
    out
}

/// Runs of consecutive hours.
fn segments(points: &[(f64, f64)]) -> Vec<Vec<(f64, f64)>> {
    let mut out: Vec<Vec<(f64, f64)>> = Vec::new();
    for &point in points {
        match out.last_mut() {
            Some(run) if run.last().is_some_and(|last| point.0 - last.0 == 1.0) => run.push(point),
            _ => out.push(vec![point]),
        }
    }
    out
}

fn hex(color: Color) -> String {
    let [r, g, b, _] = color.into_rgba8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SPAN, timeline};

    fn colors() -> Colors {
        Colors {
            accent: Color::from_rgb8(0x63, 0xd0, 0xdf),
            foreground: Color::WHITE,
            background: Color::BLACK,
        }
    }

    fn sample() -> Timeline {
        let forecast = crate::api::tests::forecast_fixture();
        timeline(
            &forecast,
            crate::model::parse_time("2026-11-17T18:40").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn columns_are_centred_on_their_hours() {
        let (width, column) = (320.0, 40.0);
        assert_eq!(x_at(column, 0.0), column / 2.0);
        assert_eq!(x_at(column, STEP as f64), column * 1.5);
        assert_eq!(x_at(column, (SPAN - STEP) as f64), width - column / 2.0);
        // Drawn at the requested width.
        assert!(svg(&sample(), &colors(), 1.0, 351.0).contains(r#"viewBox="0 0 351 84""#));
    }

    #[test]
    fn draws_every_part() {
        let svg = svg(&sample(), &colors(), 1.0, 320.0);
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
        assert_eq!(svg.matches("<circle").count(), COLUMNS);
        assert_eq!(svg.matches("<polyline").count(), 1);
        assert!(svg.contains("#63d0df"));
        assert!(svg.matches("stroke-dasharray").count() == COLUMNS);
        // Night shading and rain bars (the fixture rains from 19:30).
        assert!(svg.matches("fill-opacity=\".06\"").count() >= 1);
        assert!(svg.matches("fill-opacity=\".45\"").count() >= 1);
    }

    #[test]
    fn gaps_split_the_curve() {
        let mut timeline = sample();
        timeline.temperatures[5] = None;
        let svg = svg(&timeline, &colors(), 1.0, 320.0);
        assert_eq!(svg.matches("<polyline").count(), 2);
        assert_eq!(segments(&[(0.0, 1.0), (1.0, 1.0), (3.0, 1.0)]).len(), 2);
    }
}
