// SPDX-License-Identifier: GPL-3.0-only

//! Our side of the upstream ports: each module turns the state and events of the code in
//! `crate::upstream` into what our pages show, and our UI's commands into upstream calls. When
//! upstream changes, only these modules should need adjusting. See `UPSTREAM.md`.

pub mod audio;
pub mod battery;
pub mod bluetooth;
pub mod network;
