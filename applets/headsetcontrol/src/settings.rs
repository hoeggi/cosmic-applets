// SPDX-License-Identifier: MPL-2.0

//! Data-driven table of the settings the popup can change.

use crate::hsc::Device;

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Slider {
        min: i32,
        max: i32,
    },
    Toggle,
    /// Fixed option labels; the value is the option index.
    Choice(&'static [&'static str]),
    /// Option labels come from the device's `equalizer_presets`.
    EqPreset,
}

#[derive(Debug)]
pub struct Setting {
    /// Stable key, used in the stored config and in i18n (`setting-<key>`).
    pub key: &'static str,
    pub cap: &'static str,
    /// Capability that makes the current value readable, if any.
    pub read_cap: Option<&'static str>,
    /// Query argument for reading the value back.
    pub read_arg: Option<&'static str>,
    pub flag: &'static str,
    pub kind: Kind,
}

pub const SETTINGS: &[Setting] = &[
    Setting {
        key: "sidetone",
        cap: "CAP_SIDETONE",
        read_cap: Some("CAP_SIDETONE_STATUS"),
        read_arg: Some("-s"),
        flag: "-s",
        kind: Kind::Slider { min: 0, max: 128 },
    },
    Setting {
        key: "eq-preset",
        cap: "CAP_EQUALIZER_PRESET",
        read_cap: None,
        read_arg: None,
        flag: "-p",
        kind: Kind::EqPreset,
    },
    Setting {
        key: "mic-volume",
        cap: "CAP_MICROPHONE_VOLUME",
        read_cap: None,
        read_arg: None,
        flag: "--microphone-volume",
        kind: Kind::Slider { min: 0, max: 128 },
    },
    Setting {
        key: "mute-led",
        cap: "CAP_MICROPHONE_MUTE_LED_BRIGHTNESS",
        read_cap: None,
        read_arg: None,
        flag: "--microphone-mute-led-brightness",
        kind: Kind::Choice(&["0", "1", "2", "3"]),
    },
    Setting {
        key: "noise-filter",
        cap: "CAP_NOISE_FILTER",
        read_cap: None,
        read_arg: None,
        flag: "--noise-filter",
        kind: Kind::Choice(&["off", "low", "high"]),
    },
    Setting {
        key: "rotate-to-mute",
        cap: "CAP_ROTATE_TO_MUTE",
        read_cap: None,
        read_arg: None,
        flag: "-r",
        kind: Kind::Toggle,
    },
    Setting {
        key: "volume-limiter",
        cap: "CAP_VOLUME_LIMITER",
        read_cap: None,
        read_arg: None,
        flag: "--volume-limiter",
        kind: Kind::Toggle,
    },
    Setting {
        key: "lights",
        cap: "CAP_LIGHTS",
        read_cap: None,
        read_arg: None,
        flag: "-l",
        kind: Kind::Toggle,
    },
    Setting {
        key: "voice-prompts",
        cap: "CAP_VOICE_PROMPTS",
        read_cap: None,
        read_arg: None,
        flag: "-v",
        kind: Kind::Toggle,
    },
    Setting {
        key: "inactive-time",
        cap: "CAP_INACTIVE_TIME",
        read_cap: None,
        read_arg: None,
        flag: "-i",
        kind: Kind::Slider { min: 0, max: 90 },
    },
    Setting {
        key: "bt-when-powered-on",
        cap: "CAP_BT_WHEN_POWERED_ON",
        read_cap: None,
        read_arg: None,
        flag: "--bt-when-powered-on",
        kind: Kind::Toggle,
    },
    Setting {
        key: "bt-call-volume",
        cap: "CAP_BT_CALL_VOLUME",
        read_cap: None,
        read_arg: None,
        flag: "--bt-call-volume",
        kind: Kind::Slider { min: 0, max: 100 },
    },
];

pub fn get(key: &str) -> Option<&'static Setting> {
    SETTINGS.iter().find(|s| s.key == key)
}

impl Setting {
    pub fn supported(&self, device: &Device) -> bool {
        device.has(self.cap)
    }

    pub fn readable(&self, device: &Device) -> bool {
        self.read_cap.is_some_and(|cap| device.has(cap))
    }

    /// Current value as reported by the device (readable settings only).
    pub fn read(&self, device: &Device) -> Option<i32> {
        match self.key {
            "sidetone" => device.sidetone.as_ref().map(|s| s.level),
            _ => None,
        }
    }

    pub fn args(&self, value: i32) -> [String; 2] {
        [self.flag.to_owned(), value.to_string()]
    }
}
