//! Module tui - Terminal user interface: dashboard composition, modals, and onboarding flows.

pub mod dashboard;
mod modals;
pub mod setup;
mod status;
mod widgets;

pub use dashboard::{render, render_overlays};
pub use setup::draw_setup_modal;
