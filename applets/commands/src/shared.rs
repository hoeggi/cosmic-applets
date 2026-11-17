// SPDX-License-Identifier: MPL-2.0

//! State shared between the applet's processes.
//!
//! cosmic-panel runs one process per output, and a command runs inside the process
//! that started it. The others learn about it from this state, stored under
//! `~/.local/state/cosmic/<APP_ID>/v1/` and watched through cosmic-config.

use crate::app::APP_ID;
use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

/// A command running in one of the applet's processes.
///
/// The state outlives the processes (it is kept across reboots), and PIDs are reused, so
/// the process is identified by its PID together with its start time and the boot it ran in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedRun {
    /// The process running it. Entries of a process that died are ignored.
    pub pid: u32,
    /// When that process started, in clock ticks after boot. Missing in entries written
    /// before it was recorded; those count as dead.
    #[serde(default)]
    pub pid_start: u64,
    /// `/proc/sys/kernel/random/boot_id` of the boot the process ran in.
    #[serde(default)]
    pub boot_id: String,
    /// Unix timestamp (seconds).
    pub started: i64,
}

impl SharedRun {
    pub fn new() -> Self {
        Self::of_process(std::process::id())
    }

    fn of_process(pid: u32) -> Self {
        Self {
            pid,
            pid_start: process_start(pid).unwrap_or_default(),
            boot_id: boot_id().to_owned(),
            started: chrono::Local::now().timestamp(),
        }
    }

    /// Running in a process other than this one, which is still alive.
    pub fn is_remote(&self) -> bool {
        self.pid != std::process::id() && self.is_alive()
    }

    /// The process that started the run still exists: same boot, and the PID was not
    /// reused since (a reused PID, also a thread ID, has a different start time).
    pub fn is_alive(&self) -> bool {
        !self.boot_id.is_empty()
            && self.boot_id == boot_id()
            && process_start(self.pid) == Some(self.pid_start)
    }

    pub fn elapsed(&self) -> Duration {
        let secs = chrono::Local::now().timestamp() - self.started;
        Duration::from_secs(u64::try_from(secs).unwrap_or(0))
    }
}

/// The current boot's ID, or an empty string if it cannot be read.
fn boot_id() -> &'static str {
    static BOOT_ID: OnceLock<String> = OnceLock::new();
    BOOT_ID.get_or_init(|| {
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .map(|id| id.trim().to_owned())
            .inspect_err(|err| tracing::warn!(%err, "cannot read boot id"))
            .unwrap_or_default()
    })
}

/// Start time of process `pid` in clock ticks after boot, field 22 of `/proc/<pid>/stat`.
fn process_start(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    parse_start(&stat)
}

fn parse_start(stat: &str) -> Option<u64> {
    // Field 2 is the command name in parentheses, which may itself contain spaces and
    // parentheses; the fields after the last `)` start at field 3.
    let (_, rest) = stat.rsplit_once(')')?;
    rest.split_whitespace().nth(22 - 3)?.parse().ok()
}

#[derive(Debug, Clone, Default, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Shared {
    /// Running commands by command ID.
    pub running: BTreeMap<u64, SharedRun>,
    /// Bumped after a finished run was added to the history file.
    pub finished: u64,
}

impl Shared {
    /// Records `run` unless another process is already running the command.
    pub fn claim(&mut self, id: u64, run: SharedRun) -> bool {
        if self.running.get(&id).is_some_and(SharedRun::is_remote) {
            return false;
        }
        self.prune();
        self.running.insert(id, run);
        true
    }

    /// Drops runs of processes that no longer exist, e.g. cut off by a shutdown.
    pub fn prune(&mut self) {
        self.running.retain(|_, r| r.is_alive());
    }

    pub fn has_stale(&self) -> bool {
        self.running.values().any(|r| !r.is_alive())
    }

    pub fn finish(&mut self, id: u64) {
        self.running.remove(&id);
        self.finished = self.finished.wrapping_add(1);
    }
}

/// Changes the state on disk, one process at a time. Returns the new state and the
/// closure's result, or `None` if the state could not be written.
pub fn update<R>(
    ctx: &cosmic_config::Config,
    change: impl FnOnce(&mut Shared) -> R,
) -> Option<(Shared, R)> {
    let _lock = lock();
    let mut current = Shared::get_entry(ctx).unwrap_or_else(|(_, partial)| partial);
    let mut next = current.clone();
    let result = change(&mut next);

    let written = current
        .set_running(ctx, next.running.clone())
        .and_then(|_| current.set_finished(ctx, next.finished));
    match written {
        Ok(_) => Some((next, result)),
        Err(err) => {
            tracing::error!(%err, "failed to save shared state");
            None
        }
    }
}

/// Serialises read-modify-write cycles; released when the file is dropped.
fn lock() -> Option<File> {
    let path = std::env::var_os("XDG_RUNTIME_DIR")
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join(format!("{APP_ID}.state.lock"));
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .and_then(|file| file.lock().map(|()| file));
    file.inspect_err(|err| tracing::warn!(%err, "cannot lock shared state"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The parent process: alive, and not this process.
    fn other() -> SharedRun {
        SharedRun {
            started: 0,
            ..SharedRun::of_process(std::os::unix::process::parent_id())
        }
    }

    /// A process that does not exist (above the kernel's PID limit).
    fn dead() -> SharedRun {
        SharedRun {
            pid: u32::MAX,
            ..other()
        }
    }

    #[test]
    fn a_run_from_another_boot_is_dead() {
        let run = SharedRun {
            boot_id: "00000000-0000-0000-0000-000000000000".into(),
            ..other()
        };
        assert!(!run.is_alive());
        assert!(!run.is_remote());
    }

    #[test]
    fn a_reused_pid_is_dead() {
        let run = SharedRun {
            pid_start: other().pid_start + 1,
            ..other()
        };
        assert!(!run.is_alive());
    }

    #[test]
    fn an_entry_without_process_identity_is_dead() {
        let pid = std::os::unix::process::parent_id();
        let run: SharedRun =
            serde_json::from_str(&format!(r#"{{"pid":{pid},"started":0}}"#)).unwrap();
        assert!(!run.is_alive());
    }

    #[test]
    fn prune_drops_dead_runs_only() {
        let mut shared = Shared::default();
        shared.running.insert(1, other());
        shared.running.insert(2, dead());
        assert!(shared.has_stale());
        shared.prune();
        assert_eq!(shared.running, BTreeMap::from([(1, other())]));
        assert!(!shared.has_stale());
    }

    #[test]
    fn parse_start_handles_names_with_spaces_and_parens() {
        let stat = "42 (a) b) S 1 42 42 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 98765 1000 10";
        assert_eq!(parse_start(stat), Some(98765));
        assert_eq!(parse_start("42 (x"), None);
    }

    #[test]
    fn claim_refuses_a_run_of_another_process() {
        let mut shared = Shared::default();
        shared.running.insert(1, other());
        assert!(!shared.claim(1, SharedRun::new()));
        assert_eq!(shared.running[&1], other());
        assert!(shared.claim(2, SharedRun::new()));
    }

    #[test]
    fn claim_takes_over_from_a_dead_process_and_drops_stale_runs() {
        let mut shared = Shared::default();
        shared.running.insert(1, dead());
        shared.running.insert(2, dead());
        let run = SharedRun::new();
        assert!(shared.claim(1, run.clone()));
        assert_eq!(shared.running, BTreeMap::from([(1, run)]));
    }

    #[test]
    fn own_and_dead_runs_are_not_remote() {
        assert!(other().is_remote());
        assert!(!SharedRun::new().is_remote());
        assert!(!dead().is_remote());
    }

    #[test]
    fn finish_removes_the_run_and_announces_it() {
        let mut shared = Shared::default();
        shared.claim(1, SharedRun::new());
        shared.finish(1);
        assert!(shared.running.is_empty());
        assert_eq!(shared.finished, 1);
    }
}
