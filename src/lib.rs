//! Audio Switcher — Windows tray audio switcher.
//!
//! Library crate holds all reusable logic; `main.rs` is a minimal entry point
//! per `proj-lib-main-split`.

#![warn(missing_docs)]
#![allow(unsafe_op_in_unsafe_fn)]

pub mod app;
pub mod audio;
pub mod config;
pub mod platform;
pub mod ui;
