// SPDX-License-Identifier: MPL-2.0

//! Running commands, desktop notifications and the on-disk run history.

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};

/// Captured output is truncated to this many bytes (keeping the tail).
const MAX_OUTPUT: usize = 4096;
/// Rotate the history file once it grows beyond this size.
const HISTORY_MAX_BYTES: u64 = 1024 * 1024;
const HISTORY_KEEP_LINES: usize = 500;

/// Result of a finished, captured run.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub code: Option<i32>,
    pub output: String,
    pub duration: Duration,
}

impl RunOutcome {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Human-readable run time: `0.3 s`, `1m 12s`, `2h 05m`.
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    match secs {
        0..60 => format!("{:.1} s", duration.as_secs_f32()),
        60..3600 => format!("{}m {:02}s", secs / 60, secs % 60),
        _ => format!("{}h {:02}m", secs / 3600, secs % 3600 / 60),
    }
}

/// Runs `command` through `sh -c`, capturing stdout followed by stderr.
pub async fn run_captured(command: String) -> RunOutcome {
    let started = Instant::now();
    let result = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(&command)
        .stdin(Stdio::null())
        .output()
        .await;

    match result {
        Ok(out) => {
            let mut output = String::from_utf8_lossy(&out.stdout).into_owned();
            output.push_str(&String::from_utf8_lossy(&out.stderr));
            RunOutcome {
                code: out.status.code(),
                output: truncate_tail(output.trim_end()),
                duration: started.elapsed(),
            }
        }
        Err(err) => RunOutcome {
            code: None,
            output: format!("failed to start: {err}"),
            duration: started.elapsed(),
        },
    }
}

/// Launches `command` detached in a terminal; the terminal waits for Enter
/// before closing so short-lived output stays readable.
pub async fn spawn_in_terminal(terminal: Vec<String>, command: String) -> bool {
    let Some((program, args)) = terminal.split_first() else {
        return false;
    };
    let mut cmd = std::process::Command::new(program);
    cmd.args(args).args([
        "sh",
        "-c",
        r#"sh -c "$1"; printf '\n[exit %s] press Enter to close' "$?"; read _"#,
        "sh",
        &command,
    ]);
    cosmic::process::spawn(cmd).await.is_some()
}

fn truncate_tail(s: &str) -> String {
    if s.len() <= MAX_OUTPUT {
        return s.to_owned();
    }
    let mut start = s.len() - MAX_OUTPUT;
    while !s.is_char_boundary(start) {
        start += 1;
    }
    format!("…{}", &s[start..])
}

/// Sends a desktop notification for a finished run.
pub async fn notify(name: String, outcome: RunOutcome) {
    let summary = match outcome.code {
        Some(0) => format!("✓ {name} ({})", format_duration(outcome.duration)),
        Some(code) => format!("✗ {name} (exit {code})"),
        None => format!("✗ {name} (killed)"),
    };
    let body: String = outcome
        .output
        .lines()
        .take(3)
        .collect::<Vec<_>>()
        .join("\n");

    let result = tokio::task::spawn_blocking(move || {
        notify_rust::Notification::new()
            .appname("Commands")
            .icon("utilities-terminal-symbolic")
            .summary(&summary)
            .body(&body)
            .show()
            .map(|_| ())
    })
    .await;

    match result {
        Ok(Err(err)) => tracing::warn!(%err, "failed to send notification"),
        Err(err) => tracing::warn!(%err, "notification task panicked"),
        Ok(Ok(())) => {}
    }
}

/// One line of `runs.jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Unix timestamp (seconds).
    pub ts: i64,
    pub name: String,
    pub command: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub output: String,
}

impl HistoryEntry {
    pub fn new(name: String, command: String, outcome: &RunOutcome) -> Self {
        Self {
            ts: chrono::Local::now().timestamp(),
            name,
            command,
            exit_code: outcome.code,
            duration_ms: u64::try_from(outcome.duration.as_millis()).unwrap_or(u64::MAX),
            output: outcome.output.clone(),
        }
    }

    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }

    pub fn duration(&self) -> String {
        format_duration(Duration::from_millis(self.duration_ms))
    }

    /// Local wall-clock time, `HH:MM`.
    pub fn time(&self) -> String {
        chrono::DateTime::from_timestamp(self.ts, 0)
            .map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string())
            .unwrap_or_default()
    }
}

fn history_path(app_id: &str) -> Option<PathBuf> {
    dirs::state_dir().map(|dir| dir.join(app_id).join("runs.jsonl"))
}

/// Loads the most recent `limit` history entries, newest first.
pub fn load_history(app_id: &str, limit: usize) -> Vec<HistoryEntry> {
    let Some(content) = history_path(app_id).and_then(|p| fs::read_to_string(p).ok()) else {
        return Vec::new();
    };
    content
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str(line).ok())
        .take(limit)
        .collect()
}

/// Appends an entry, rotating the file when it grows too large.
pub async fn append_history(app_id: &'static str, entry: HistoryEntry) {
    let result = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        let Some(path) = history_path(app_id) else {
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        writeln!(file, "{}", serde_json::to_string(&entry)?)?;

        if file.metadata()?.len() > HISTORY_MAX_BYTES {
            let content = fs::read_to_string(&path)?;
            let lines: Vec<&str> = content.lines().collect();
            let keep = &lines[lines.len().saturating_sub(HISTORY_KEEP_LINES)..];
            fs::write(&path, keep.join("\n") + "\n")?;
        }
        Ok(())
    })
    .await;

    match result {
        Ok(Err(err)) => tracing::warn!(%err, "failed to write run history"),
        Err(err) => tracing::warn!(%err, "history task panicked"),
        Ok(Ok(())) => {}
    }
}
