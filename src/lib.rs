//! `GoVMR` library crate - exposes the application modules for reuse and testing.
//!
//! The crate is organized as a small layered architecture, and dependencies
//! flow strictly downward:
//!
//! ```text
//! main (binary entry)
//!   ├── cli        command-line front end
//!   └── tui        terminal-UI front end (render layer)
//!         └── app        controller (state, actions, key handling)
//!               └── manager    engine (fetch, install, switch, delete)
//!                     └── version    Go-version domain model
//! ```
//!
//! `shim`, `config`, `logging`, `errors`, and `theme` are shared support
//! modules used across those layers.

pub mod app;
pub mod cli;
pub mod config;
pub mod errors;
pub mod logging;
pub mod manager;
pub mod shim;
pub mod theme;
pub mod tui;
pub mod version;
