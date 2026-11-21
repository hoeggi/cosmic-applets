// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

//! Ported from pop-os/cosmic-applets cosmic-applet-audio/src/lib.rs at ecb6b61.
//!
//! system-applet: upstream has these as methods of its applet struct (`output_icon_name`,
//! `input_icon_name`) reading `self.model.active_sink` / `active_source`. Here they are free
//! functions taking the volume and mute they read; their bodies are upstream's.

pub fn output_icon_name(volume: u32, mute: bool) -> &'static str {
    if mute || volume == 0 {
        "audio-volume-muted-symbolic"
    } else if volume < 33 {
        "audio-volume-low-symbolic"
    } else if volume < 66 {
        "audio-volume-medium-symbolic"
    } else if volume <= 100 {
        "audio-volume-high-symbolic"
    } else {
        "audio-volume-overamplified-symbolic"
    }
}

pub fn input_icon_name(volume: u32, mute: bool) -> &'static str {
    if mute || volume == 0 {
        "microphone-sensitivity-muted-symbolic"
    } else if volume < 33 {
        "microphone-sensitivity-low-symbolic"
    } else if volume < 66 {
        "microphone-sensitivity-medium-symbolic"
    } else {
        "microphone-sensitivity-high-symbolic"
    }
}
