//! Module actions - Execution of queued [`Action`]s against the [`GoManager`].
//!
//! Network-bound work (refresh, install) is spawned as a background task that
//! reports back through the shared channel, keeping the render loop fluid;
//! quick local operations (switch, delete, path-fix) run inline and post a
//! follow-up action where a refresh of the UI is needed.

use crate::{
    app::{Action, App, BusyState, MsgKind, Phase},
    logging,
    manager::GoManager,
    version::GoVersion,
};
use ratatui::text::{Line, Span};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Drains and processes all queued background actions for one event-loop tick.
///
/// Each arm only manages the busy state of its *own* operation. Long-running
/// tasks are spawned and report back via the MPSC channel to keep the UI fluid.
///
/// # Errors
/// Always returns `Ok(())` today; individual action failures are reported to
/// the user through the status bar instead of aborting the loop.
pub async fn handle_actions(
    action_rx: &mut mpsc::UnboundedReceiver<Action>,
    app: &mut App,
    manager: &Arc<GoManager>,
    action_tx: &mpsc::UnboundedSender<Action>,
) -> anyhow::Result<()> {
    while let Ok(action) = action_rx.try_recv() {
        match action {
            Action::Refresh => spawn_refresh(app, manager, action_tx),
            Action::RefreshDone(result) => handle_refresh_done(app, result),
            Action::Install(v) => start_install(app, manager, action_tx, v),
            Action::InstallProgress(p) => app.update_install_progress(p),
            Action::InstallDone(v) => handle_install_done(app, manager, action_tx, &v),
            Action::InstallFailed(err) => handle_install_failed(app, &err),
            Action::Use(v) => handle_use(app, manager, &v),
            Action::Delete(v) => handle_delete(app, manager, action_tx, &v),
            Action::FixPath => handle_fix_path(app, manager),
        }
    }
    Ok(())
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Spawns the background manifest fetch so the render loop stays fluid.
fn spawn_refresh(
    app: &mut App,
    manager: &Arc<GoManager>,
    action_tx: &mpsc::UnboundedSender<Action>,
) {
    app.state.busy = Some(BusyState::Refreshing);
    let mgr = manager.clone();
    let tx = action_tx.clone();

    // Spawn a background task to fetch versions without blocking the render loop
    tokio::spawn(async move {
        let result = mgr.fetch_versions().await;
        let _ = tx.send(Action::RefreshDone(result.map_err(|e| e.to_string())));
    });
}

/// Applies a finished manifest refresh to the UI state.
fn handle_refresh_done(app: &mut App, result: Result<Vec<GoVersion>, String>) {
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

/// Marks the install busy state and spawns the cancellable download task.
fn start_install(
    app: &mut App,
    manager: &Arc<GoManager>,
    action_tx: &mpsc::UnboundedSender<Action>,
    v: GoVersion,
) {
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
    let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
    app.state.cancel_install = Some(cancel_tx);

    tokio::spawn(async move {
        let progress_tx2 = progress_tx.clone();

        let result = tokio::select! {
            res = mgr.download_and_install(&v, move |p| {
                let _ = progress_tx2.send(Action::InstallProgress(p));
            }) => res,
            () = async {
                // Wait for the cancel signal to become true
                while !*cancel_rx.borrow() {
                    if cancel_rx.changed().await.is_err() {
                        break; // Sender dropped
                    }
                }
            } => {
                logging::info("install cancelled by user (task aborted)");
                Err(crate::errors::GovmError::Cancelled)
            }
        };

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

/// Auto-activates a freshly installed version and posts a follow-up refresh.
fn handle_install_done(
    app: &mut App,
    manager: &Arc<GoManager>,
    action_tx: &mpsc::UnboundedSender<Action>,
    v: &GoVersion,
) {
    app.state.busy = None;
    app.state.cancel_install = None;

    // AUTO-ACTIVATE: Switch to the newly installed version immediately.
    let activated = match manager.switch_version(v) {
        Ok(in_path) => {
            app.state.is_shim_in_path = in_path;
            for ver in &mut app.state.versions {
                ver.active = ver.raw_version == v.raw_version;
            }
            true
        }
        Err(e) => {
            logging::error(&format!("auto-activate failed after install: {e}"));
            false
        }
    };

    let msg = if activated {
        format!(
            "Go {} installed & activated ✓ (archive cleaned to save space)",
            v.raw_version
        )
    } else {
        format!(
            "Go {} installed ✓ — press u to activate (archive cleaned to save space)",
            v.raw_version
        )
    };
    app.set_status(msg, MsgKind::Success);

    // Refresh to pick up any manifest changes or updated installed flags
    let _ = action_tx.send(Action::Refresh);
}

/// Clears the install busy state and reports a failed installation.
fn handle_install_failed(app: &mut App, err: &str) {
    app.state.busy = None;
    app.state.cancel_install = None;
    if err.contains("cancelled") {
        app.set_status("Installation cancelled", MsgKind::Info);
    } else {
        logging::error(&format!("install failed: {err}"));
        app.set_status(format!("Installation failed: {err}"), MsgKind::Error);
    }
}

/// Switches the active toolchain and updates local UI state immediately.
fn handle_use(app: &mut App, manager: &Arc<GoManager>, v: &GoVersion) {
    app.state.busy = Some(BusyState::Switching(v.raw_version.clone()));
    match manager.switch_version(v) {
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
            app.set_status(e.to_string(), MsgKind::Error);
        }
    }
    // Clear the busy state directly instead of waiting for RefreshDone
    app.state.busy = None;
}

/// Deletes an installed toolchain and queues a manifest refresh.
fn handle_delete(
    app: &mut App,
    manager: &Arc<GoManager>,
    action_tx: &mpsc::UnboundedSender<Action>,
    v: &GoVersion,
) {
    app.state.busy = Some(BusyState::Deleting(v.raw_version.clone()));
    match manager.delete_version(v) {
        Ok(()) => {
            app.set_status(format!("Deleted Go {}", v.raw_version), MsgKind::Success);
        }
        Err(e) => {
            logging::error(&format!("delete failed: {e}"));
            app.set_status(e.to_string(), MsgKind::Error);
        }
    }
    let _ = action_tx.send(Action::Refresh);
}

/// Runs the permanent PATH fix and shows the summary in the help overlay.
fn handle_fix_path(app: &mut App, manager: &Arc<GoManager>) {
    // Runs the platform's permanent PATH snippet in a hidden child
    // process; fast and local, so it runs inline. The summary is
    // shown inside the still-open help overlay, not on the dashboard
    // status bar.
    match manager.fix_path_permanently() {
        Ok(lines) => {
            // lines is Vec<String> now
            let styled_lines: Vec<Line<'static>> = lines
                .into_iter()
                .enumerate()
                .map(|(i, line)| {
                    if i == 0 {
                        Line::from(Span::styled(line, app.state.theme.success()))
                    } else if i == 1 && line.starts_with("    ") {
                        Line::from(Span::styled(line, app.state.theme.brand_bold()))
                    } else {
                        Line::from(Span::styled(line, app.state.theme.muted()))
                    }
                })
                .collect();
            app.state.path_fix_notice = Some(styled_lines);
            app.state.is_shim_in_path = manager.get_shim_manager().is_in_path();
        }
        Err(e) => {
            logging::error(&format!("fix-path failed: {e}"));
            app.set_status(format!("Could not fix PATH: {e}"), MsgKind::Error);
        }
    }
}
