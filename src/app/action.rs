//! Module action - Message protocol between the UI and the background task layer.
//!
//! Every long-running operation the user triggers becomes one [`Action`]
//! pushed through the UI's channel; completion (or failure) arrives back as
//! another `*Done` / `*Failed` variant, so the render loop never blocks.

use crate::{manager::InstallProgress, version::GoVersion};

/// Actions dispatched asynchronously to execute backend operations.
pub enum Action {
    /// Reload remote version list from `go.dev`.
    Refresh,
    /// Finished reloading remote version list.
    RefreshDone(Result<Vec<GoVersion>, String>),
    /// Download and install the specified Go version.
    Install(GoVersion),
    /// A progress event emitted mid-installation.
    InstallProgress(InstallProgress),
    /// Installation finished successfully.
    InstallDone(GoVersion),
    /// Installation failed with the supplied message.
    InstallFailed(String),
    /// Activate the specified Go version via shims.
    Use(GoVersion),
    /// Remove an installed Go version from disk.
    Delete(GoVersion),
    /// Apply the permanent PATH fix by running the platform snippet in a
    /// hidden child process (the `f` key in the setup/help overlay).
    FixPath,
}
