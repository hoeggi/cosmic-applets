// SPDX-License-Identifier: MPL-2.0

//! Thin wrapper around the `headsetcontrol` CLI and its JSON output (API 1.x).

use serde::Deserialize;
use serde_json::{Map, Value};
use std::io::ErrorKind;
use std::time::Duration;

const BIN: &str = "headsetcontrol";
/// The CLI's own HID timeout is 5 s; leave room for that plus startup.
const TIMEOUT: Duration = Duration::from_secs(8);

pub const CAP_BATTERY: &str = "CAP_BATTERY_STATUS";
pub const CAP_CHATMIX: &str = "CAP_CHATMIX_STATUS";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HscError {
    NotInstalled,
    Failed(String),
}

impl std::fmt::Display for HscError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled => write!(f, "{BIN} is not installed"),
            Self::Failed(msg) => f.write_str(msg),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Report {
    api_version: String,
    #[serde(default)]
    devices: Vec<Device>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Device {
    #[serde(default)]
    pub device: String,
    #[serde(default)]
    pub id_vendor: String,
    #[serde(default)]
    pub id_product: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub battery: Option<Battery>,
    pub chatmix: Option<i32>,
    pub sidetone: Option<Sidetone>,
    /// Preset name → band values; key order is the preset index.
    #[serde(default)]
    pub equalizer_presets: Map<String, Value>,
    #[serde(default)]
    pub errors: Map<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Battery {
    pub status: String,
    pub level: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Sidetone {
    pub level: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatteryState {
    Available(u8),
    /// Some devices report -1 while charging.
    Charging(Option<u8>),
    Unavailable,
}

impl Device {
    /// `vid:pid` without the `0x` prefixes, e.g. `1038:2202`.
    pub fn key(&self) -> String {
        let strip = |s: &str| s.trim_start_matches("0x").to_owned();
        format!("{}:{}", strip(&self.id_vendor), strip(&self.id_product))
    }

    pub fn has(&self, cap: &str) -> bool {
        self.capabilities.iter().any(|c| c == cap)
    }

    pub fn battery_state(&self) -> Option<BatteryState> {
        let battery = self.battery.as_ref()?;
        let level = u8::try_from(battery.level).ok().filter(|l| *l <= 100);
        Some(match (battery.status.as_str(), level) {
            ("BATTERY_CHARGING", level) => BatteryState::Charging(level),
            ("BATTERY_AVAILABLE", Some(level)) => BatteryState::Available(level),
            _ => BatteryState::Unavailable,
        })
    }

    /// Whether the headset itself answers (not just its dongle).
    pub fn is_online(&self) -> bool {
        if self.has(CAP_BATTERY) {
            !matches!(self.battery_state(), None | Some(BatteryState::Unavailable))
        } else {
            true
        }
    }

    pub fn preset_names(&self) -> Vec<String> {
        self.equalizer_presets.keys().cloned().collect()
    }

    fn error_summary(&self) -> Option<String> {
        (!self.errors.is_empty()).then(|| {
            self.errors
                .iter()
                .map(|(k, v)| format!("{k}: {}", v.as_str().unwrap_or_default()))
                .collect::<Vec<_>>()
                .join("; ")
        })
    }
}

/// Runs `headsetcontrol <args> -o json` and returns the first device, if any.
async fn invoke(args: &[String], test_device: bool) -> Result<(Option<Device>, bool), HscError> {
    let mut cmd = tokio::process::Command::new(BIN);
    if test_device {
        cmd.arg("--test-device");
    }
    cmd.args(args)
        .args(["-o", "json"])
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    tracing::debug!(?args, test_device, "headsetcontrol");

    let out = match tokio::time::timeout(TIMEOUT, cmd.output()).await {
        Err(_) => return Err(HscError::Failed(format!("{BIN} timed out"))),
        Ok(Err(err)) if err.kind() == ErrorKind::NotFound => return Err(HscError::NotInstalled),
        Ok(Err(err)) => return Err(HscError::Failed(format!("cannot run {BIN}: {err}"))),
        Ok(Ok(out)) => out,
    };

    let device = parse(&out.stdout).map_err(|err| {
        let stderr = String::from_utf8_lossy(&out.stderr);
        HscError::Failed(format!("{err} {}", stderr.trim()).trim_end().to_owned())
    })?;
    Ok((device, out.status.success()))
}

/// Parses `-o json` output and returns the first device, if any.
fn parse(stdout: &[u8]) -> Result<Option<Device>, String> {
    let report: Report =
        serde_json::from_slice(stdout).map_err(|err| format!("unexpected output ({err})"))?;
    if !report.api_version.starts_with("1.") {
        return Err(format!(
            "unsupported headsetcontrol API {}",
            report.api_version
        ));
    }
    Ok(report.devices.into_iter().next())
}

/// Reads status (battery, chatmix, plus `extra` queries like `-s`).
pub async fn query(extra: Vec<String>, test_device: bool) -> Result<Option<Device>, HscError> {
    invoke(&extra, test_device).await.map(|(device, _)| device)
}

/// Applies settings, e.g. `["-s", "64", "-p", "1"]`.
pub async fn apply(args: Vec<String>, test_device: bool) -> Result<(), String> {
    match invoke(&args, test_device).await {
        Err(err) => Err(err.to_string()),
        Ok((None, _)) => Err("no headset found".into()),
        Ok((Some(device), success)) => match device.error_summary() {
            Some(errors) => Err(errors),
            None if !success => Err(format!("{BIN} failed")),
            None => Ok(()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Option<Device> {
        let path = format!("{}/tests/fixtures/{name}.json", env!("CARGO_MANIFEST_DIR"));
        parse(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn nova7_online() {
        let device = fixture("nova7").unwrap();
        assert_eq!(device.key(), "1038:2202");
        assert!(device.is_online());
        assert!(matches!(
            device.battery_state(),
            Some(BatteryState::Available(_) | BatteryState::Charging(_))
        ));
        assert!(device.chatmix.is_some());
        assert_eq!(device.preset_names(), ["flat", "bass", "focus", "smiley"]);
        let readable: Vec<_> = crate::settings::SETTINGS
            .iter()
            .filter(|s| s.readable(&device))
            .collect();
        assert!(
            readable.is_empty(),
            "Nova 7 (gen 1) cannot read settings back"
        );
    }

    #[test]
    fn nova7_offline() {
        let device = fixture("nova7-offline").unwrap();
        assert!(!device.is_online());
        assert_eq!(device.battery_state(), Some(BatteryState::Unavailable));
        assert!(device.error_summary().unwrap().contains("offline"));
    }

    #[test]
    fn test_device_reads_sidetone() {
        let device = fixture("test-device").unwrap();
        let sidetone = crate::settings::get("sidetone").unwrap();
        assert!(sidetone.readable(&device));
        assert_eq!(sidetone.read(&device), Some(85));
        assert!(
            crate::settings::SETTINGS
                .iter()
                .all(|s| s.supported(&device))
        );
    }

    #[test]
    fn no_device() {
        assert!(fixture("no-device").is_none());
    }

    #[test]
    fn rejects_new_major_api() {
        let json = br#"{"api_version": "2.0", "devices": []}"#;
        assert!(parse(json).is_err());
    }
}
