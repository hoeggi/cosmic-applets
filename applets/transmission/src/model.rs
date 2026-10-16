// SPDX-License-Identifier: MPL-2.0

//! What the applet derives from a [`Snapshot`]: torrent states, list filters,
//! the panel summary, and formatting of rates and times.

use crate::rpc::{Snapshot, Torrent};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    // Declaration order is the list order.
    Error,
    Downloading,
    Checking,
    Queued,
    Seeding,
    Paused,
}

impl Torrent {
    pub fn state(&self) -> State {
        // Tracker warnings (1) are shown as text but do not make the torrent fail.
        if self.error >= 2 {
            return State::Error;
        }
        match self.status {
            1 | 2 => State::Checking,
            3 | 5 => State::Queued,
            4 => State::Downloading,
            6 => State::Seeding,
            _ => State::Paused,
        }
    }

    pub fn is_done(&self) -> bool {
        self.size_when_done > 0 && self.left_until_done == 0
    }

    /// Downloading or waiting to download.
    fn wants_data(&self) -> bool {
        matches!(self.status, 3 | 4) && !self.is_done()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Filter {
    /// Everything that is doing something: downloads, errors, checks, and
    /// seeding torrents that are uploading right now.
    #[default]
    Active,
    Downloading,
    Seeding,
    All,
}

impl Filter {
    pub const ALL: [Self; 4] = [Self::Active, Self::Downloading, Self::Seeding, Self::All];

    pub fn matches(self, torrent: &Torrent) -> bool {
        let state = torrent.state();
        match self {
            Self::Active => match state {
                State::Paused => false,
                State::Seeding => torrent.rate_upload > 0,
                State::Queued => torrent.wants_data(),
                _ => true,
            },
            Self::Downloading => torrent.wants_data(),
            Self::Seeding => matches!(torrent.status, 5 | 6),
            Self::All => true,
        }
    }
}

/// Torrents matching `filter`, in list order.
pub fn visible(snapshot: &Snapshot, filter: Filter) -> Vec<&Torrent> {
    let mut list: Vec<_> = snapshot
        .torrents
        .iter()
        .filter(|t| filter.matches(t))
        .collect();
    list.sort_by_key(|t| (t.state(), t.name.to_lowercase()));
    list
}

pub fn count(snapshot: &Snapshot, filter: Filter) -> usize {
    snapshot
        .torrents
        .iter()
        .filter(|t| filter.matches(t))
        .count()
}

/// What the panel button shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// Not configured, NAS unreachable, or login failed.
    Offline,
    /// At least one torrent failed.
    Error,
    /// Connected, nothing downloading.
    Idle,
    /// `count` torrents downloading, `percent` overall progress of them.
    Downloading { count: usize, percent: u8 },
}

impl Panel {
    pub fn from_snapshot(snapshot: &Snapshot) -> Self {
        if snapshot.torrents.iter().any(|t| t.state() == State::Error) {
            return Self::Error;
        }
        let downloading: Vec<_> = snapshot
            .torrents
            .iter()
            .filter(|t| t.wants_data())
            .collect();
        if downloading.is_empty() {
            return Self::Idle;
        }
        let total: i64 = downloading.iter().map(|t| t.size_when_done.max(0)).sum();
        let left: i64 = downloading
            .iter()
            .map(|t| t.left_until_done.clamp(0, t.size_when_done.max(0)))
            .sum();
        let percent = if total > 0 {
            ((total - left) * 100 / total) as u8
        } else {
            0
        };
        Self::Downloading {
            count: downloading.len(),
            percent,
        }
    }
}

/// Per-torrent facts remembered between polls to detect transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seen {
    done: bool,
    failed: bool,
}

pub fn seen(snapshot: &Snapshot) -> HashMap<i64, Seen> {
    snapshot
        .torrents
        .iter()
        .map(|t| {
            (
                t.id,
                Seen {
                    done: t.is_done(),
                    failed: t.state() == State::Error,
                },
            )
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Finished(String),
    Failed { name: String, error: String },
}

/// Torrents that finished or failed since `before`. Torrents added in between
/// only count when they were seen unfinished, so adding a complete torrent is quiet.
pub fn events(before: &HashMap<i64, Seen>, now: &Snapshot) -> Vec<Event> {
    let mut events = Vec::new();
    for t in &now.torrents {
        let Some(prev) = before.get(&t.id) else {
            continue;
        };
        if t.is_done() && !prev.done {
            events.push(Event::Finished(t.name.clone()));
        }
        if t.state() == State::Error && !prev.failed {
            events.push(Event::Failed {
                name: t.name.clone(),
                error: t.error_string.clone(),
            });
        }
    }
    events
}

/// Decimal units like Transmission itself: "812 B/s", "4.2 MB/s".
pub fn rate(bytes_per_sec: i64) -> String {
    format!("{}/s", size(bytes_per_sec))
}

pub fn size(bytes: i64) -> String {
    const UNITS: [&str; 4] = ["kB", "MB", "GB", "TB"];
    let bytes = bytes.max(0);
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if value < 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

/// "45 s", "12 min", "1 h 4 min", "2 d 3 h"; `None` when unknown.
pub fn eta(secs: i64) -> Option<String> {
    if secs < 0 {
        return None;
    }
    let (d, h, m) = (secs / 86_400, secs / 3600 % 24, secs / 60 % 60);
    Some(if d > 0 {
        format!("{d} d {h} h")
    } else if h > 0 {
        format!("{h} h {m} min")
    } else if m > 0 {
        format!("{m} min")
    } else {
        format!("{secs} s")
    })
}

pub fn ratio(ratio: f64) -> String {
    if ratio == -2.0 {
        "∞".into()
    } else if ratio < 0.0 {
        "–".into()
    } else {
        format!("{ratio:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Snapshot {
        let body: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/torrent_get.json")).unwrap();
        let torrents = serde_json::from_value(body["result"]["torrents"].clone()).unwrap();
        Snapshot {
            torrents,
            alt_speed: false,
        }
    }

    fn names(list: &[&Torrent]) -> Vec<String> {
        list.iter().map(|t| t.name.clone()).collect()
    }

    #[test]
    fn states() {
        let s = fixture();
        let states: Vec<_> = s.torrents.iter().map(Torrent::state).collect();
        assert_eq!(
            states,
            [
                State::Downloading,
                State::Downloading,
                State::Seeding,
                State::Seeding, // tracker warning only
                State::Paused,
                State::Error,
            ]
        );
    }

    #[test]
    fn active_filter_hides_idle_seeds_and_paused() {
        let s = fixture();
        assert_eq!(
            names(&visible(&s, Filter::Active)),
            [
                "project-backup-2026-09.tar.zst",
                "fedora-workstation-live-45-x86_64.iso",
                "ubuntu-26.04-desktop-amd64.iso",
                "debian-13.1.0-amd64-netinst.iso",
            ]
        );
        assert_eq!(count(&s, Filter::Downloading), 2);
        assert_eq!(count(&s, Filter::Seeding), 2);
        assert_eq!(count(&s, Filter::All), 6);
    }

    #[test]
    fn panel_summary() {
        let mut s = fixture();
        assert_eq!(Panel::from_snapshot(&s), Panel::Error);
        s.torrents.retain(|t| t.state() != State::Error);
        // (4.6 + 2.3 GB) total, (1.656 + 1.886 GB) left → 48 %.
        assert_eq!(
            Panel::from_snapshot(&s),
            Panel::Downloading {
                count: 2,
                percent: 48
            }
        );
        s.torrents.retain(|t| t.status != 4);
        assert_eq!(Panel::from_snapshot(&s), Panel::Idle);
    }

    #[test]
    fn events_on_transitions_only() {
        let mut s = fixture();
        s.torrents.retain(|t| t.error < 2);
        let before = seen(&s);
        assert!(events(&before, &s).is_empty());

        s.torrents[0].left_until_done = 0;
        s.torrents[1].error = 3;
        s.torrents[1].error_string = "disk full".into();
        s.torrents.push(Torrent {
            id: 99,
            name: "added complete".into(),
            size_when_done: 1,
            ..Default::default()
        });
        assert_eq!(
            events(&before, &s),
            [
                Event::Finished("ubuntu-26.04-desktop-amd64.iso".into()),
                Event::Failed {
                    name: "fedora-workstation-live-45-x86_64.iso".into(),
                    error: "disk full".into()
                },
            ]
        );
    }

    #[test]
    fn formatting() {
        assert_eq!(rate(0), "0 B/s");
        assert_eq!(rate(812), "812 B/s");
        assert_eq!(rate(4_200_000), "4.2 MB/s");
        assert_eq!(rate(310_000), "310 kB/s");
        assert_eq!(size(4_600_000_000), "4.6 GB");
        assert_eq!(eta(-1), None);
        assert_eq!(eta(45).as_deref(), Some("45 s"));
        assert_eq!(eta(720).as_deref(), Some("12 min"));
        assert_eq!(eta(3840).as_deref(), Some("1 h 4 min"));
        assert_eq!(eta(2 * 86_400 + 3 * 3600).as_deref(), Some("2 d 3 h"));
        assert_eq!(ratio(1.4239), "1.42");
        assert_eq!(ratio(-1.0), "–");
        assert_eq!(ratio(-2.0), "∞");
    }
}
