// SPDX-License-Identifier: MPL-2.0

//! Icons used outside the weather codes. Theme icons are preferred. libcosmic looks them up in
//! the COSMIC and Pop themes but does not reach Adwaita, so sunrise, sunset, wind, humidity and
//! pressure have no theme icon there; those are drawn here as symbolic 16×16 glyphs, repainted in
//! the text colour like any symbolic icon. `theme_icons_resolve` checks the theme ones.

use cosmic::widget::icon;

pub const UV: &str = "weather-clear-symbolic";
pub const VISIBILITY: &str = "image-red-eye-symbolic";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Sunrise,
    Sunset,
    Wind,
    Humidity,
    Pressure,
}

impl Glyph {
    fn path(self) -> &'static str {
        match self {
            Self::Sunrise => {
                "M2 12h12v1.2H2zM4.8 11a3.2 3.2 0 0 1 6.4 0zM7.4 7V4.2H6L8 1.8l2 2.4H8.6V7z"
            }
            Self::Sunset => {
                "M2 12h12v1.2H2zM4.8 11a3.2 3.2 0 0 1 6.4 0zM7.4 1.8v2.8H6L8 7l2-2.4H8.6V1.8z"
            }
            Self::Wind => {
                "M1 7h8.5a1.7 1.7 0 1 0-1.7-1.7H6.5A3 3 0 1 1 9.5 8.3H1zM1 10h10.5a1.4 1.4 0 1 1-1.4 \
                 1.4H8.8a2.7 2.7 0 1 0 2.7-2.7H1z"
            }
            Self::Humidity => {
                "M8 1.5S3.5 6.6 3.5 9.8a4.5 4.5 0 0 0 9 0C12.5 6.6 8 1.5 8 1.5zm0 2.3c1.5 1.9 3 4.3 3 \
                 6a3 3 0 0 1-6 0c0-1.7 1.5-4.1 3-6z"
            }
            Self::Pressure => {
                "M8 2a7 7 0 0 0-5.6 11.2l1-.8A5.7 5.7 0 1 1 12.6 12.4l1 .8A7 7 0 0 0 8 2zm3.5 \
                 3.6L7.3 8.2a1.2 1.2 0 1 0 1.5 1.6z"
            }
        }
    }

    fn svg(self) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path fill="currentColor" d="{}"/></svg>"#,
            self.path()
        )
    }

    pub fn handle(self) -> icon::Handle {
        let mut handle = icon::from_svg_bytes(self.svg().into_bytes());
        handle.symbolic = true;
        handle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{self, Precip};

    #[test]
    fn glyphs_are_well_formed() {
        for glyph in [
            Glyph::Sunrise,
            Glyph::Sunset,
            Glyph::Wind,
            Glyph::Humidity,
            Glyph::Pressure,
        ] {
            let svg = glyph.svg();
            assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
            assert!(
                !svg.contains("  "),
                "{glyph:?} path has a line-continuation gap"
            );
        }
    }

    /// Resolves every theme icon the applet uses against the icon themes installed on this
    /// machine, and fails on a missing one or one that only resolves through libcosmic's
    /// fallback (e.g. `weather-windy-symbolic` turning into the colour `weather` app icon).
    /// Run it on the host (the build container has no icon themes):
    /// `target/release/deps/cosmic_applet_weather-* --ignored --nocapture theme_icons_resolve`
    #[test]
    #[ignore = "needs the desktop's icon themes"]
    fn theme_icons_resolve() {
        let mut names: Vec<&str> = vec![
            UV,
            VISIBILITY,
            "find-location-symbolic",
            "view-refresh-symbolic",
            "emblem-system-symbolic",
            "dialog-warning-symbolic",
            "go-previous-symbolic",
            "go-next-symbolic",
            "pan-down-symbolic",
            "object-select-symbolic",
            "edit-delete-symbolic",
            "list-add-symbolic",
        ];
        for code in 0..=99 {
            names.push(model::icon(code, true));
            names.push(model::icon(code, false));
        }
        for kind in [
            Precip::Rain,
            Precip::Snow,
            Precip::FreezingRain,
            Precip::Thunderstorm,
            Precip::Hail,
        ] {
            names.push(kind.icon());
        }
        names.sort_unstable();
        names.dedup();

        let wrong: Vec<String> = names
            .into_iter()
            .filter_map(|name| {
                let path = icon::from_name(name).size(16).path();
                println!("{name}: {path:?}");
                let exact = path
                    .as_ref()
                    .and_then(|p| p.file_stem())
                    .is_some_and(|stem| stem == name);
                (!exact).then(|| format!("{name} -> {path:?}"))
            })
            .collect();
        assert!(wrong.is_empty(), "not in the icon theme: {wrong:#?}");
    }
}
