// SPDX-License-Identifier: GPL-3.0-only

//! Ported from pop-os/cosmic-applets cosmic-applet-audio at ecb6b61: `model.rs` verbatim, and
//! the output and input icon logic from `lib.rs` in `icons.rs`. `crate::adapter::audio` uses
//! them.

pub mod icons;
// system-applet: upstream's active-player update (`MprisUpdate::Player`) isn't read by our
// adapter, which lists every player; allowed here so the file stays as upstream has it.
#[allow(dead_code)]
pub mod mpris_subscription;
// system-applet: not every upstream method is used by our adapter; allowed here so that
// model.rs itself stays verbatim.
#[allow(dead_code)]
pub mod model;
