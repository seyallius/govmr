//! Module app - Central state container and action dispatcher for interactive TUI execution.

mod action;
mod actions;
mod keys;
mod state;

pub use action::Action;
pub use actions::handle_actions;
pub use keys::{handle_key, KeyOutcome};
pub use state::{visible_indices, ActiveTab, AppState, BusyState, MsgKind, Phase, StatusMessage};

use crate::{
    logging,
    manager::{GoManager, InstallProgress},
    theme::{Theme, ThemeName},
    version::GoVersion,
};
use ratatui::widgets::ListState;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

// ------------------------------------------ Types & Impls ------------------------------------- //

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
                path_fix_notice: None,
                show_logs: false,
                log_lines: Vec::new(),
                log_scroll: 0,
                log_follow: true,
                log_refreshed: None,
                log_focus: false,
                log_wrap: false,
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

    /// Opens the docked log panel *without* stealing keyboard focus, so the
    /// dashboard stays fully usable while logs tail live.
    pub fn open_logs(&mut self) {
        self.state.show_logs = true;
        self.state.log_focus = false;
        self.state.log_follow = true;
        self.state.log_scroll = 0;
        self.refresh_logs();
    }

    /// Closes the docked log panel and drops focus back to the dashboard.
    pub fn close_logs(&mut self) {
        self.state.show_logs = false;
        self.state.log_focus = false;
    }

    /// Toggles word wrapping of long log lines (URLs, stack traces, …).
    pub fn toggle_log_wrap(&mut self) {
        self.state.log_wrap = !self.state.log_wrap;
    }

    /// Re-reads the log file into the viewer cache. When following, stays
    /// pinned to the newest entry; otherwise preserves the scroll position.
    pub fn refresh_logs(&mut self) {
        self.state.log_lines = logging::read_lines();
        self.state.log_refreshed = Some(Instant::now());
        let max = self.state.log_lines.len().saturating_sub(1);
        if self.state.log_follow {
            self.state.log_scroll = 0;
        } else if self.state.log_scroll > max {
            self.state.log_scroll = max;
        }
    }

    /// Re-reads the log cache if the viewer is open and the throttle window
    /// has elapsed. Cheap no-op otherwise, so it is safe to call every tick.
    pub fn refresh_logs_if_open(&mut self) {
        if !self.state.show_logs {
            return;
        }
        let stale = self
            .state
            .log_refreshed
            .map_or(true, |t| t.elapsed() >= Duration::from_millis(500));
        if stale {
            self.refresh_logs();
        }
    }

    /// Scrolls the log viewer by `delta` lines: positive moves up into older
    /// history, negative moves down toward newer entries.
    pub fn scroll_logs(&mut self, delta: i64) {
        let max = self.state.log_lines.len().saturating_sub(1);
        let next = (self.state.log_scroll as i64 + delta).clamp(0, max as i64) as usize;
        self.state.log_scroll = next;
        // Following only makes sense while pinned to the newest entry.
        self.state.log_follow = next == 0;
    }

    /// Toggles auto-follow; re-enabling snaps back to the newest entry.
    pub fn toggle_log_follow(&mut self) {
        self.state.log_follow = !self.state.log_follow;
        if self.state.log_follow {
            self.state.log_scroll = 0;
        }
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
