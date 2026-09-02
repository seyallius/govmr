//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//
//! Module app - Central state container and action dispatcher for interactive TUI execution.

use crate::{
    manager::{GoManager, InstallProgress},
    models::GoVersion,
    theme::{Theme, ThemeName},
};
use ratatui::widgets::ListState;
use std::{sync::Arc, time::Instant};

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Identifies the currently active tab view in the TUI.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    /// Browsing all official remote versions.
    Available,
    /// Browsing locally installed toolchains.
    Installed,
}
impl ActiveTab {
    /// Flips to the other tab.
    pub fn toggle(self) -> Self {
        match self {
            ActiveTab::Available => ActiveTab::Installed,
            ActiveTab::Installed => ActiveTab::Available,
        }
    }
}

/// Severity of a transient status message.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MsgKind {
    /// Positive confirmation.
    Success,
    /// Failure or destructive warning.
    Error,
    /// Neutral informational note.
    Info,
}

/// A transient status-bar message.
#[derive(Clone)]
pub struct StatusMessage {
    /// Human-readable text.
    pub text: String,
    /// Visual severity.
    pub kind: MsgKind,
}

/// Lifecycle phase of a running installation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The archive is being downloaded.
    Downloading,
    /// The archive is being extracted to disk.
    Extracting,
}

/// Background task state surfaced to the UI.
#[derive(Clone)]
pub enum BusyState {
    /// Refreshing the version manifest from `go.dev`.
    Refreshing,
    /// Switching the active toolchain.
    Switching(String),
    /// Removing an installed toolchain.
    Deleting(String),
    /// Installing a toolchain, with live progress.
    Installing {
        /// Version being installed.
        version: String,
        /// Current lifecycle phase.
        phase: Phase,
        /// Bytes fetched so far.
        downloaded: u64,
        /// Total archive size in bytes.
        total: u64,
        /// Download speed in bytes per second.
        speed: f64,
        /// When the download started (for ETA calculations).
        started_at: Instant,
    },
}
impl BusyState {
    /// Returns the version targeted by the busy operation, if any.
    pub fn target(&self) -> Option<&str> {
        match self {
            BusyState::Refreshing => None,
            BusyState::Switching(v)
            | BusyState::Deleting(v)
            | BusyState::Installing { version: v, .. } => Some(v),
        }
    }
}

/// Holds all state variables required for rendering and interacting with the TUI.
pub struct AppState {
    /// Full list of available and installed versions.
    pub versions: Vec<GoVersion>,
    /// State container for the interactive version list widget.
    pub list_state: ListState,
    /// The currently selected tab view.
    pub active_tab: ActiveTab,
    /// Current background task, if any.
    pub busy: Option<BusyState>,
    /// Transient status-bar message.
    pub status_message: Option<StatusMessage>,
    /// Targeted version pending user deletion confirmation.
    pub confirming_delete: Option<String>,
    /// Indicates whether the GoVMR shim path is configured in system `PATH`.
    pub is_shim_in_path: bool,
    /// Filesystem path to the shim directory (shown in the help overlay).
    pub shim_path: String,
    /// Live filter query applied to the current view.
    pub filter: String,
    /// Whether the user is actively typing a filter query.
    pub filter_mode: bool,
    /// Whether the PATH-setup help overlay is displayed.
    pub show_help: bool,
    /// Whether the color-theme picker overlay is displayed.
    pub show_theme_picker: bool,
    /// Index of the currently highlighted entry in the theme picker.
    pub theme_picker_index: usize,
    /// The active color palette (reloaded instantly when switching themes).
    pub theme: Theme,
    /// Monotonic render counter used to drive spinner animations.
    pub tick_count: u64,
}
impl AppState {
    /// Constructs a fresh state for the supplied version list (used by tests/setup).
    pub fn from_versions(versions: Vec<GoVersion>, is_shim_in_path: bool) -> Self {
        let mut list_state = ListState::default();
        if !versions.is_empty() {
            list_state.select(Some(0));
        }
        let theme = Theme::for_name(ThemeName::default());
        Self {
            versions,
            list_state,
            active_tab: ActiveTab::Available,
            busy: None,
            status_message: None,
            confirming_delete: None,
            is_shim_in_path,
            shim_path: String::from("~/.govmr/shim"),
            filter: String::new(),
            filter_mode: false,
            show_help: false,
            show_theme_picker: false,
            theme_picker_index: 0,
            theme,
            tick_count: 0,
        }
    }

    /// Returns indices into `versions` visible under the current tab and filter.
    pub fn visible_indices(&self) -> Vec<usize> {
        visible_indices(self)
    }

    /// Keeps the selection index within the bounds of the currently visible list.
    pub fn clamp_selection(&mut self) {
        let len = self.visible_indices().len();
        match self.list_state.selected() {
            Some(i) if len > 0 && i >= len => self.list_state.select(Some(len - 1)),
            Some(_) => {}
            None if len > 0 => self.list_state.select(Some(0)),
            None => {}
        }
    }

    /// Moves the active selection cursor to the next visible item (wraps around).
    pub fn next_item(&mut self) {
        let len = self.visible_indices().len();
        if len == 0 {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    /// Moves the active selection cursor to the previous visible item (wraps around).
    pub fn previous_item(&mut self) {
        let len = self.visible_indices().len();
        if len == 0 {
            return;
        }
        let i = match self.list_state.selected() {
            Some(0) => len - 1,
            Some(i) => i - 1,
            None => 0,
        };
        self.list_state.select(Some(i));
    }
}

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
    ///
    /// The initial version fetch is dispatched via the `Action::Refresh`
    /// message in the main event loop, allowing the UI to render immediately
    /// and show a loading state while the network request proceeds.
    pub fn new(manager: Arc<GoManager>, shim_path: String) -> Self {
        let is_in_path = manager.get_shim_manager().is_in_path();
        let current_theme = manager.theme_name();
        let theme_picker_index = ThemeName::ALL
            .iter()
            .position(|t| *t == current_theme)
            .unwrap_or(0);

        Self {
            state: AppState {
                versions: Vec::new(),
                list_state: ListState::default(),
                active_tab: ActiveTab::Available,
                busy: None,
                status_message: None,
                confirming_delete: None,
                is_shim_in_path: is_in_path,
                shim_path,
                filter: String::new(),
                filter_mode: false,
                show_help: false,
                show_theme_picker: false,
                theme_picker_index,
                theme: Theme::for_name(current_theme),
                tick_count: 0,
            },
            manager,
        }
    }

    /// Asynchronously refreshes the version list from the GoManager.
    #[deprecated(
        since = "1.0.0",
        note = "This method is obsolete - version fetching is now handled by background tasks via handle_actions"
    )]
    pub async fn refresh_versions(&mut self) {
        self.state.busy = Some(BusyState::Refreshing);
        match self.manager.fetch_versions().await {
            Ok(versions) => {
                self.state.versions = versions;
                self.state.status_message = None;
                self.clamp_selection();
            }
            Err(e) => {
                self.set_status(e.to_string(), MsgKind::Error);
            }
        }
        self.state.busy = None;
    }

    /// Records a transient status message.
    pub fn set_status(&mut self, text: impl Into<String>, kind: MsgKind) {
        self.state.status_message = Some(StatusMessage {
            text: text.into(),
            kind,
        });
    }

    /// Whether a background task is currently blocking new actions.
    pub fn is_busy(&self) -> bool {
        self.state.busy.is_some()
    }

    /// Returns indices into [`AppState::versions`] visible under the current tab and filter.
    pub fn visible_indices(&self) -> Vec<usize> {
        visible_indices(&self.state)
    }

    /// Returns the currently selected [`GoVersion`], honoring tab and filter visibility.
    pub fn selected_version(&self) -> Option<&GoVersion> {
        let visible = self.visible_indices();
        let pos = self.state.list_state.selected()?;
        visible.get(pos).map(|&i| &self.state.versions[i])
    }

    /// Switches to the other tab and resets navigation.
    pub fn switch_tab(&mut self) {
        self.state.active_tab = self.state.active_tab.toggle();
        self.state.list_state.select(Some(0));
    }

    /// Opens the theme picker, landing on the currently active theme.
    pub fn open_theme_picker(&mut self) {
        self.state.show_theme_picker = true;
        let current = self.manager.theme_name();
        self.state.theme_picker_index = ThemeName::ALL
            .iter()
            .position(|t| *t == current)
            .unwrap_or(0);
        self.state.theme = self.manager.theme();
    }

    /// Returns the theme currently highlighted in the picker.
    pub fn picker_theme(&self) -> ThemeName {
        ThemeName::ALL[self.state.theme_picker_index]
    }

    /// Moves the picker cursor up/down and live-previews the theme.
    pub fn picker_move(&mut self, delta: i32) {
        let len = ThemeName::ALL.len() as i32;
        let mut i = self.state.theme_picker_index as i32 + delta;
        if i < 0 {
            i = len - 1;
        }
        if i >= len {
            i = 0;
        }
        self.state.theme_picker_index = i as usize;
        self.state.theme = Theme::for_name(self.picker_theme());
    }

    /// Selects the highlighted picker entry, persisting it via the manager.
    pub fn picker_apply(&mut self) {
        let chosen = self.picker_theme();
        match self.manager.set_theme(chosen) {
            Ok(theme) => {
                self.state.theme = theme;
                self.set_status(
                    format!("Theme set to {} and saved", chosen.title()),
                    MsgKind::Success,
                );
            }
            Err(e) => {
                self.set_status(format!("Could not save theme: {}", e), MsgKind::Error);
            }
        }
        self.state.show_theme_picker = false;
    }

    /// Closes the picker and restores the persisted theme (cancelling preview).
    pub fn picker_cancel(&mut self) {
        self.state.show_theme_picker = false;
        self.state.theme = self.manager.theme();
    }

    /// Keeps the selection index within the bounds of the currently visible list.
    pub fn clamp_selection(&mut self) {
        self.state.clamp_selection();
    }

    /// Moves the active selection cursor to the next visible item.
    pub fn next_item(&mut self) {
        self.state.next_item();
    }

    /// Moves the active selection cursor to the previous visible item.
    pub fn previous_item(&mut self) {
        self.state.previous_item();
    }

    /// Applies a [`InstallProgress`] event to the active installation state.
    pub fn update_install_progress(&mut self, progress: InstallProgress) {
        if let Some(BusyState::Installing {
            version,
            phase: _,
            downloaded,
            total,
            speed,
            started_at,
        }) = self.state.busy.clone()
        {
            match progress {
                InstallProgress::Downloading {
                    downloaded: d,
                    total: t,
                    bytes_per_sec,
                } => {
                    self.state.busy = Some(BusyState::Installing {
                        version,
                        phase: Phase::Downloading,
                        downloaded: d,
                        total: t,
                        speed: bytes_per_sec,
                        started_at,
                    });
                }
                InstallProgress::Extracting => {
                    self.state.busy = Some(BusyState::Installing {
                        version,
                        phase: Phase::Extracting,
                        downloaded,
                        total,
                        speed,
                        started_at,
                    });
                }
            }
        }
    }
}

// ----------------------------------------- Public API ----------------------------------------- //

/// Returns indices into [`AppState::versions`] visible under the given state's tab and filter.
pub fn visible_indices(state: &AppState) -> Vec<usize> {
    let query = state.filter.to_lowercase();
    state
        .versions
        .iter()
        .enumerate()
        .filter(|(_, v)| match state.active_tab {
            ActiveTab::Available => true,
            ActiveTab::Installed => v.installed,
        })
        .filter(|(_, v)| {
            query.is_empty()
                || v.raw_version.to_lowercase().contains(&query)
                || v.display_name.to_lowercase().contains(&query)
        })
        .map(|(i, _)| i)
        .collect()
}
