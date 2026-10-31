// SPDX-License-Identifier: MPL-2.0

//! Picks one of an applet's processes for work that must happen only once.
//!
//! cosmic-panel runs one process per output, and each sees the same events, so
//! e.g. a notification sent from all of them shows up once per monitor. The first
//! process to need it takes an advisory lock and keeps it; the kernel drops the lock
//! with the process (e.g. when its monitor is unplugged), and another takes over at
//! its next event.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct Leader {
    held: Option<File>,
}

impl Leader {
    /// Whether this process should do the work. Without a working lock every
    /// process answers yes: doing it twice is better than not at all.
    pub fn acquire(&mut self, app_id: &str) -> bool {
        self.acquire_at(&path(app_id))
    }

    fn acquire_at(&mut self, path: &Path) -> bool {
        if self.held.is_some() {
            return true;
        }
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path);
        let file = match file {
            Ok(file) => file,
            Err(err) => {
                tracing::warn!(%err, "cannot open leader lock");
                return true;
            }
        };
        match file.try_lock() {
            Ok(()) => {
                self.held = Some(file);
                true
            }
            Err(TryLockError::WouldBlock) => false,
            Err(TryLockError::Error(err)) => {
                tracing::warn!(%err, "cannot take leader lock");
                true
            }
        }
    }
}

fn path(app_id: &str) -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join(format!("{app_id}.leader.lock"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_holder_and_takeover() {
        let path = std::env::temp_dir().join(format!("leader-lock-{}", std::process::id()));
        let (mut a, mut b) = (Leader::default(), Leader::default());

        assert!(a.acquire_at(&path));
        assert!(a.acquire_at(&path));
        assert!(!b.acquire_at(&path));

        drop(a);
        assert!(b.acquire_at(&path));

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn unusable_lock_still_runs() {
        let path = Path::new("/nonexistent-directory/leader.lock");
        assert!(Leader::default().acquire_at(path));
    }
}
