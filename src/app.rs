//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//!
//! Module app - Central state container and action dispatcher for interactive TUI execution.

use crate::{manager::GoManager, models::GoVersion};
use ratatui::widgets::ListState;
use std::sync::Arc;

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Identifies the currently active tab view in the TUI.
pub enum ActiveTab {
    /// Browsing all official remote versions.
    Available,
    /// Browsing locally installed toolchains.
    Installed,
}

/// Holds all state variables required for rendering and interacting with the TUI.
pub struct AppState {
    /// Full list of available and installed versions.
    pub versions: Vec<GoVersion>,
    /// State container for the interactive version list widget.
    pub list_state: ListState,
    /// The currently selected tab view.
    pub active_tab: ActiveTab,
    /// Indicates whether a background task is executing.
    pub loading: bool,
    /// The version string currently undergoing installation or processing.
    pub action_target: Option<String>,
    /// Status or error banner message `(message_text, is_error)`.
    pub status_message: Option<(String, bool)>,
    /// Targeted version pending user deletion confirmation.
    pub confirming_delete: Option<String>,
    /// Indicates whether the GoVMR shim path is configured in system `PATH`.
    pub is_shim_in_path: bool,
}

/// Actions dispatched asynchronously to execute backend operations.
pub enum Action {
    /// Reload remote version list from `go.dev`.
    Refresh,
    /// Download and install the specified Go version.
    Install(GoVersion),
    /// Activate the specified Go version via shims.
    Use(GoVersion),
    /// Remove an installed Go version from disk.
    Delete(GoVersion),
}

/// Main application controller holding application state and business logic references.
pub struct App {
    /// Mutable UI state.
    pub state: AppState,
    /// Core Go manager handling version operations.
    manager: Arc<GoManager>,
}
impl App {
    // ----------------------------------------- Public API ----------------------------------------- //

    /// Instantiates a new application controller, performing initial version manifest loading.
    pub async fn new(manager: Arc<GoManager>) -> Self {
        let is_in_path = manager.get_shim_manager().is_in_path();
        let mut app = Self {
            state: AppState {
                versions: Vec::new(),
                list_state: ListState::default(),
                active_tab: ActiveTab::Available,
                loading: true,
                action_target: None,
                status_message: None,
                confirming_delete: None,
                is_shim_in_path: is_in_path,
            },
            manager,
        };
        app.refresh_versions().await;
        app
    }

    /// Asynchronously refreshes the version list from the GoManager.
    pub async fn refresh_versions(&mut self) {
        self.state.loading = true;
        match self.manager.fetch_versions().await {
            Ok(versions) => {
                self.state.versions = versions;
                if !self.state.versions.is_empty() && self.state.list_state.selected().is_none() {
                    self.state.list_state.select(Some(0));
                }
                self.state.status_message = None;
            }
            Err(e) => {
                self.state.status_message = Some((e.to_string(), true));
            }
        }
        self.state.loading = false;
    }

    /// Returns a reference to the currently highlighted [`GoVersion`], if any.
    pub fn selected_version(&self) -> Option<&GoVersion> {
        let idx = self.state.list_state.selected()?;
        self.state.versions.get(idx)
    }

    /// Moves the active selection cursor to the next item in the list.
    pub fn next_item(&mut self) {
        if self.state.versions.is_empty() {
            return;
        }
        let i = match self.state.list_state.selected() {
            Some(i) => (i + 1) % self.state.versions.len(),
            None => 0,
        };
        self.state.list_state.select(Some(i));
    }

    /// Moves the active selection cursor to the previous item in the list.
    pub fn previous_item(&mut self) {
        if self.state.versions.is_empty() {
            return;
        }
        let i = match self.state.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.state.versions.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.list_state.select(Some(i));
    }
}
