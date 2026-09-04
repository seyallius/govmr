//! Module actions - Execution of queued [`Action`]s against the [`GoManager`].
//!
//! Network-bound work (refresh, install) is spawned as a background task that
//! reports back through the shared channel, keeping the render loop fluid;
//! quick local operations (switch, delete) run inline and post a follow-up
//! action where a refresh of the UI is needed.

use crate::{
    app::{Action, App, BusyState, MsgKind, Phase},
    logging,
    manager::GoManager,
};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Drains and processes all queued background actions for one event-loop tick.
///
/// Each arm only manages the busy state of its *own* operation. Long-running
/// tasks are spawned and report back via the MPSC channel to keep the UI fluid.
pub async fn handle_actions(
    action_rx: &mut mpsc::UnboundedReceiver<Action>,
    app: &mut App,
    manager: &Arc<GoManager>,
    action_tx: &mpsc::UnboundedSender<Action>,
) -> anyhow::Result<()> {
    while let Ok(action) = action_rx.try_recv() {
        match action {
            Action::Refresh => {
                app.state.busy = Some(BusyState::Refreshing);
                let mgr = manager.clone();
                let tx = action_tx.clone();

                // Spawn a background task to fetch versions without blocking the render loop
                tokio::spawn(async move {
                    let result = mgr.fetch_versions().await;
                    let _ = tx.send(Action::RefreshDone(result.map_err(|e| e.to_string())));
                });
            }
            Action::RefreshDone(result) => {
                app.state.busy = None;
                match result {
                    Ok(versions) => {
                        app.state.versions = versions;
                        app.state.status_message = None;
                        app.clamp_selection();
                    }
                    Err(e) => {
                        app.set_status(e, MsgKind::Error);
                    }
                }
            }
            Action::Install(v) => {
                app.state.busy = Some(BusyState::Installing {
                    version: v.raw_version.clone(),
                    phase: Phase::Downloading,
                    downloaded: 0,
                    total: v.size,
                    speed: 0.0,
                    started_at: std::time::Instant::now(),
                });
                app.state.status_message = None;
                let mgr = manager.clone();
                let progress_tx = action_tx.clone();
                let done_tx = action_tx.clone();

                tokio::spawn(async move {
                    let progress_tx2 = progress_tx.clone();
                    let result = mgr
                        .download_and_install(&v, move |p| {
                            let _ = progress_tx2.send(Action::InstallProgress(p));
                        })
                        .await;
                    match result {
                        Ok(_) => {
                            let _ = done_tx.send(Action::InstallDone(v));
                        }
                        Err(e) => {
                            let _ = done_tx.send(Action::InstallFailed(e.to_string()));
                        }
                    }
                });
            }
            Action::InstallProgress(p) => {
                app.update_install_progress(p);
            }
            Action::InstallDone(v) => {
                app.state.busy = None;
                app.set_status(
                    format!("Go {} installed successfully", v.raw_version),
                    MsgKind::Success,
                );
                let _ = action_tx.send(Action::Refresh);
            }
            Action::InstallFailed(err) => {
                logging::error(&format!("install failed: {err}"));
                app.state.busy = None;
                app.set_status(format!("Installation failed: {}", err), MsgKind::Error);
            }
            Action::Use(v) => {
                app.state.busy = Some(BusyState::Switching(v.raw_version.clone()));
                match manager.switch_version(&v) {
                    Ok(in_path) => {
                        app.set_status(
                            format!("Switched to Go {}", v.raw_version),
                            MsgKind::Success,
                        );

                        // Update local UI state instantly without a network round-trip
                        app.state.is_shim_in_path = in_path;
                        for ver in &mut app.state.versions {
                            ver.active = ver.raw_version == v.raw_version;
                        }
                    }
                    Err(e) => {
                        logging::error(&format!("use failed: {e}"));
                        app.set_status(e.to_string(), MsgKind::Error)
                    }
                }
                // Clear the busy state directly instead of waiting for RefreshDone
                app.state.busy = None;
            }
            Action::Delete(v) => {
                app.state.busy = Some(BusyState::Deleting(v.raw_version.clone()));
                match manager.delete_version(&v) {
                    Ok(_) => {
                        app.set_status(format!("Deleted Go {}", v.raw_version), MsgKind::Success)
                    }
                    Err(e) => {
                        logging::error(&format!("delete failed: {e}"));
                        app.set_status(e.to_string(), MsgKind::Error)
                    }
                }
                let _ = action_tx.send(Action::Refresh);
            }
        }
    }
    Ok(())
}
