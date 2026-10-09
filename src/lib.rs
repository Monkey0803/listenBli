//! listenBli — a Bilibili music player with synced lyrics.
//!
//! The crate is split into a library (everything testable without a window) and
//! a thin binary that wires it to `eframe`.

pub mod api;
pub mod app;
pub mod audio;
pub mod config;
pub mod lyrics;
pub mod net;
pub mod platform;
pub mod ui;
pub mod util;
