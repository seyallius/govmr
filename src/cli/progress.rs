//! Module progress - CLI download progress, bridging install events to indicatif.

use crate::manager::InstallProgress;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::{Mutex, MutexGuard};

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Owns the indicatif progress indicator and reacts to install progress events.
pub(crate) struct CliProgress {
    inner: Mutex<Option<ProgressBar>>,
    bar_style: ProgressStyle,
    spin_style: ProgressStyle,
}
impl CliProgress {
    /// Locks the shared indicator, recovering from a poisoned lock: a panic
    /// elsewhere must never take the CLI down with it.
    fn lock_indicator(&self) -> MutexGuard<'_, Option<ProgressBar>> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn new(bar_style: ProgressStyle, spin_style: ProgressStyle) -> Self {
        Self {
            inner: Mutex::new(None),
            bar_style,
            spin_style,
        }
    }

    pub(crate) fn on_event(&self, event: InstallProgress) {
        let mut guard = self.lock_indicator();
        match event {
            InstallProgress::Downloading {
                downloaded, total, ..
            } => {
                if guard.is_none() {
                    let pb = if total > 0 {
                        ProgressBar::new(total)
                    } else {
                        ProgressBar::new_spinner()
                    };
                    pb.set_style(self.bar_style.clone());
                    *guard = Some(pb);
                }
                if let Some(pb) = guard.as_ref() {
                    pb.set_position(downloaded);
                    if total == 0 {
                        pb.tick();
                    }
                }
            }
            InstallProgress::Extracting => {
                if let Some(pb) = guard.take() {
                    pb.finish_and_clear();
                }
                let spinner = ProgressBar::new_spinner();
                spinner.set_style(self.spin_style.clone());
                spinner.set_message("Unpacking archive…");
                spinner.enable_steady_tick(std::time::Duration::from_millis(80));
                *guard = Some(spinner);
            }
        }
    }

    pub(crate) fn finish(&self) {
        if let Some(pb) = self.lock_indicator().as_ref() {
            pb.finish_and_clear();
        }
    }
}
