// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from pop-os/cosmic-applets cosmic-applet-battery/src/app.rs at ecb6b61: the battery
//! icon, the screen brightness percent and display icon, and the brightness snapping.
//!
//! system-applet: upstream has these as methods of its applet struct (`update_battery`,
//! `screen_brightness_percent`, `update_display`) and inside its `SetScreenBrightness` handler.
//! Here they are free functions taking the fields they read as arguments; their bodies are
//! upstream's, unchanged apart from reading those arguments instead of `self`.

/// Upstream's `update_battery`: the panel icon for a battery level.
pub fn battery_icon_name(
    mut percent: f64,
    on_battery: bool,
    charging_limit: Option<bool>,
) -> String {
    percent = percent.clamp(0.0, 100.0);
    let battery_percent = if percent > 95.0 && !charging_limit.unwrap_or_default() {
        100
    } else if percent > 80.0 && !charging_limit.unwrap_or_default() {
        90
    } else if percent > 65.0 {
        80
    } else if percent > 35.0 {
        50
    } else if percent > 20.0 {
        35
    } else if percent > 14.0 {
        20
    } else if percent > 9.0 {
        10
    } else if percent > 5.0 {
        5
    } else {
        0
    };
    let limited = if charging_limit.unwrap_or_default() {
        "limited-"
    } else {
        ""
    };
    let charging = if on_battery { "" } else { "charging-" };
    format!("cosmic-applet-battery-level-{battery_percent}-{limited}{charging}symbolic",)
}

/// Upstream's `screen_brightness_percent`, as a fraction (0.01 to 1.0).
pub fn screen_brightness_percent(
    screen_brightness: Option<i32>,
    max_screen_brightness: Option<i32>,
) -> Option<f64> {
    let raw = screen_brightness? as i64;
    let max = max_screen_brightness?.max(1) as i64;
    if max <= 20 {
        // Coarse panels (<=20 brightness levels)
        let rung = (raw.saturating_add(1)).min(20);
        Some((5 * rung) as f64 / 100.0)
    } else {
        let p = ((raw * 100 + max / 2) / max).clamp(1, 100) as f64;
        Some(p / 100.0)
    }
}

/// Upstream's `update_display`: the brightness slider's icon.
pub fn display_icon_name(screen_brightness_percent: Option<f64>) -> String {
    let screen_brightness = if let Some(percent) = screen_brightness_percent {
        if percent < 0.011 {
            "off"
        } else if percent < 0.333 {
            "low"
        } else if percent < 0.666 {
            "medium"
        } else {
            "high"
        }
    } else {
        "off"
    };

    format!("cosmic-applet-battery-display-brightness-{screen_brightness}-symbolic",)
}

/// Upstream's `SetScreenBrightness` snapping, "matching brightness calculation logic from
/// cosmic-osd and cosmic-settings-daemon".
pub fn snap_screen_brightness(brightness: i32, max_screen_brightness: Option<i32>) -> i32 {
    if let Some(max) = max_screen_brightness {
        if max > 0 && max <= 20 {
            // Coarse: map raw→k by round, then back to raw setpoint round(k*max/20)
            let k = ((brightness as i64 * 20 + (max as i64) / 2) / (max as i64)).clamp(0, 20);
            (((k * (max as i64)) + 10) / 20) as i32
        } else {
            brightness
        }
    } else {
        brightness
    }
}
