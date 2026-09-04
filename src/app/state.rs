//! Module state - Mutable UI state containers and transient status types for the TUI.

use crate::{
    theme::{Theme, ThemeName},
    version::GoVersion,
};
use ratatui::widgets::ListState;
use std::time::Instant;

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
