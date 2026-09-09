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
            Action::InstallFailed(message) => handle_install_failed(app, &message),
            Action::Use(v) => handle_use(app, manager, &v),
            Action::Delete(v) => handle_delete(app, manager, action_tx, &v),
            Action::FixPath => handle_fix_path(app, manager),
            Action::Update => spawn_update(app, manager, action_tx),
            Action::Uninstall(purge) => handle_uninstall(app, manager, purge),
            Action::UninstallBinaryOnly => handle_uninstall(app, manager, false),
            Action::UpdateDone(msg) => app.set_status(msg, MsgKind::Success),
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
                // Not logged here: `handle_install_failed` writes the one
                // `install: cancelled` line, with the version attached.
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
            // The success half of the auto-activation used to live only in the
            // status bar, which is gone by the time anyone reads the log.
            logging::info(&format!(
                "post-install: auto-activated version={} shim_in_path={in_path}",
                v.raw_version
            ));
            true
        }
        Err(e) => {
            logging::error(&format!(
                "post-install: auto-activate failed version={} error=\"{e}\" note=install_succeeded",
                v.raw_version
            ));
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
///
/// This is the *only* place an install failure is logged. It extracts the
/// target version from the `BusyState` before clearing it, avoiding the need
/// to pass the version through the `Action` enum variant.
fn handle_install_failed(app: &mut App, message: &str) {
    // Extract version BEFORE clearing the busy state!
    let version = app
        .state
        .busy
        .as_ref()
        .and_then(BusyState::target)
        .unwrap_or("unknown")
        .to_string();

    app.state.busy = None;
    app.state.cancel_install = None;

    if is_user_cancel(message) {
        logging::info(&format!(
            "install: cancelled version={version} note=user_abort"
        ));
        app.set_status("Installation cancelled", MsgKind::Info);
    } else {
        logging::error(&format!(
            "install: failed version={version} error=\"{message}\"{}",
            failure_hint(message)
        ));
        app.set_status(format!("Installation failed: {message}"), MsgKind::Error);
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
            logging::error(&format!(
                "use: failed version={} error=\"{e}\"{}",
                v.raw_version,
                failure_hint(&e.to_string())
            ));
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
            // The freed-space figure is logged by `delete_version`, which is the
            // only layer that saw the directory before it was removed.
            app.set_status(format!("Deleted Go {}", v.raw_version), MsgKind::Success);
        }
        Err(e) => {
            logging::error(&format!(
                "delete: failed version={} error=\"{e}\"{}",
                v.raw_version,
                failure_hint(&e.to_string())
            ));
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
            // Sole owner of the fix-path failure line: the help overlay's `f` key
            // dispatches `Action::FixPath` too, so exactly one place can report it.
            logging::error(&format!("fix-path: failed error=\"{e}\""));
            app.set_status(format!("Could not fix PATH: {e}"), MsgKind::Error);
        }
    }
}

/// Spawns the background self-update check.
fn spawn_update(
    app: &mut App,
    manager: &Arc<GoManager>,
    action_tx: &mpsc::UnboundedSender<Action>,
) {
    logging::info("update: started target=latest");
    app.set_status("Checking for updates...", MsgKind::Info);
    let mgr = manager.clone();
    let tx = action_tx.clone();

    tokio::spawn(async move {
        match mgr.check_for_update().await {
            Ok(Some(ver)) => {
                if let Err(e) = mgr.perform_update(&ver).await {
                    let _ = tx.send(Action::InstallFailed(format!("[UPDATE]{ver}|||{e}")));
                } else {
                    let _ = tx.send(Action::UpdateDone(format!(
                        "Updated to v{ver}! Restart govmr to run it."
                    )));
                }
            }
            Ok(None) => {
                let _ = tx.send(Action::UpdateDone(
                    "You are already on the latest version.".to_string(),
                ));
            }
            Err(e) => {
                let _ = tx.send(Action::InstallFailed(format!("[UPDATE]unknown|||{e}")));
            }
        }
    });
}

fn handle_uninstall(app: &mut App, manager: &Arc<GoManager>, purge: bool) {
    if let Err(e) = manager.uninstall(purge) {
        app.set_status(format!("Uninstall failed: {e}"), MsgKind::Error);
    } else {
        let msg = if purge {
            "govmr uninstalled and ~/.govmr purged. Press q to exit."
        } else {
            "govmr binary removed. ~/.govmr kept intact. Press q to exit."
        };
        app.set_status(msg, MsgKind::Success);
    }
}

/// Whether a failure message is really the user pressing cancel.
fn is_user_cancel(message: &str) -> bool {
    message.to_lowercase().contains("cancelled")
}

/// Appends a `hint="..."` field to a failure line when the message matches a
/// known, actionable cause.
///
/// Hints live in the log only: the status bar stays short enough to read in one
/// glance, while whoever later greps the file gets the "so what do I do" half.
fn failure_hint(message: &str) -> String {
    let lower = message.to_lowercase();
    let hint = if lower.contains("text file busy") || lower.contains("os error 26") {
        "the running binary is locked; quit other govmr instances and retry (or replace the executable manually)"
    } else if lower.contains("is not a tar.gz archive") || lower.contains("is not a zip archive") {
        "server sent a non-archive payload; check proxy/captive portal, or whether the release file exists"
    } else if lower.contains("permission denied") {
        "~/.govmr is not writable by the current user"
    } else {
        return String::new();
    };
    format!(" hint=\"{hint}\"")
}
