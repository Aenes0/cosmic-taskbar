// SPDX-License-Identifier: GPL-3.0-only

//! Taskbar v2 popups (window previews & right-click context menu) hosted as
//! standalone layer-shell surfaces directly on cosmic-comp (Vulkan GPU).

mod host;
pub mod proto;
mod wayland;

pub use host::run;
