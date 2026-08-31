use crate::{manager::GoManager, models::GoVersion};
use ratatui::widgets::ListState;
use std::sync::Arc;

pub enum ActiveTab {
    Available,
    Installed,
}

pub struct AppState {
    pub versions: Vec<GoVersion>,
    pub list_state: ListState,
    pub active_tab: ActiveTab,
    pub loading: bool,
    pub action_target: Option<String>,
    pub status_message: Option<(String, bool)>, // (message, is_error)
    pub confirming_delete: Option<String>,
    pub is_shim_in_path: bool,
}

pub enum Action {
    Refresh,
    Install(GoVersion),
    Use(GoVersion),
    Delete(GoVersion),
}

pub struct App {
    pub state: AppState,
    manager: Arc<GoManager>,
}
impl App {
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

    pub fn selected_version(&self) -> Option<&GoVersion> {
        let idx = self.state.list_state.selected()?;
        self.state.versions.get(idx)
    }

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
