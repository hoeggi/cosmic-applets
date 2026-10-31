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
use std::time::Duration;

/// A command running in one of the applet's processes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedRun {
    /// The process running it. Entries of a process that died are ignored.
    pub pid: u32,
    /// Unix timestamp (seconds).
    pub started: i64,
}

impl SharedRun {
    pub fn new() -> Self {
        Self {
            pid: std::process::id(),
            started: chrono::Local::now().timestamp(),
        }
    }

    /// Running in a process other than this one, which is still alive.
    pub fn is_remote(&self) -> bool {
        self.pid != std::process::id() && is_alive(self.pid)
    }

    pub fn elapsed(&self) -> Duration {
        let secs = chrono::Local::now().timestamp() - self.started;
        Duration::from_secs(u64::try_from(secs).unwrap_or(0))
    }
}

fn is_alive(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
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
        self.running.retain(|_, r| is_alive(r.pid));
        self.running.insert(id, run);
        true
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
            pid: std::os::unix::process::parent_id(),
            started: 0,
        }
    }

    /// A process that does not exist (above the kernel's PID limit).
    fn dead() -> SharedRun {
        SharedRun {
            pid: u32::MAX,
            started: 0,
        }
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
        assert!(shared.claim(1, run));
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
