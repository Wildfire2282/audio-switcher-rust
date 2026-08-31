//! Audio Switcher — Windows tray audio switcher.
//! Library crate holds all reusable logic; `main.rs` is a minimal entry point
//! per `proj-lib-main-split`.
//!
//! # Architecture
//! - `config` — 强类型配置与持久化
//! - `audio` — `AudioBackend` 抽象
//! - `platform` — Windows 平台封装
//! - `ui` — 托盘 UI
//! - `app` — 运行时

#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
#![allow(unsafe_op_in_unsafe_fn)]

pub mod app;
pub mod audio;
pub mod config;
pub mod platform;
pub mod prelude;
pub mod ui;

//  curated re-exports for external consumers
pub use config::{AppConfig, Lang};
pub use platform::{ComGuard, SingleInstanceGuard};
