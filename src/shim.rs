//! Module shim - Shim generation and PATH validation utilities.
//!
//! Every step is written to the audit log: PATH misconfiguration is the most
//! common support issue, and a session that "can't find `go`" is only
//! diagnosable if shim creation and the `PATH` check left a trace.

use crate::{errors::GovmError, logging};
use std::{
    env,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Manager responsible for generating executable shims and verifying environment PATH integrity.
pub(crate) struct ShimManager {
    /// Directory where generated shims reside (`~/.govmr/shim`).
    shim_dir: PathBuf,
}
impl ShimManager {
    // ------------------------------------- Public (crate) API ------------------------------------- //

    /// Creates a new `ShimManager`, ensuring the underlying shim directory exists.
    ///
    /// # Errors
    /// Returns [`GovmError::HomeNotFound`] if the user's home directory cannot be determined,
    /// or [`GovmError::Io`] if directory creation fails.
    pub(crate) fn new() -> Result<Self, GovmError> {
        let home = dirs::home_dir().ok_or(GovmError::HomeNotFound)?;
        let shim_dir = home.join(".govmr").join("shim");
        fs::create_dir_all(&shim_dir).map_err(|e| {
            logging::error(&format!(
                "shim failed: action=create_dir dir=\"{}\" error={e}",
                shim_dir.display()
            ));
            GovmError::Io(e)
        })?;
        logging::debug(&format!(
            "shim: ready dir=\"{}\"",
            shim_dir.to_string_lossy()
        ));
        Ok(Self { shim_dir })
    }

    /// Returns a reference to the directory containing executable shims.
    #[must_use]
    pub(crate) fn get_shim_dir(&self) -> &Path {
        &self.shim_dir
    }

    /// Checks if the `GoVMR` shim directory is present in the system's `PATH` environment variable.
    ///
    /// The answer is logged (with the directory it compared against), because
    /// "the shim exists but `go` still doesn't resolve" is exactly a `PATH`
    /// mismatch, and the two facts have to appear together to prove it.
    #[must_use]
    pub(crate) fn is_in_path(&self) -> bool {
        let in_path = path_contains_dir(&self.shim_dir);
        logging::debug(&format!(
            "shim: path check dir=\"{}\" in_path={in_path}",
            self.shim_dir.to_string_lossy()
        ));
        in_path
    }

    /// Generates shims for all executables found within the specified version binary directory.
    ///
    /// # Arguments
    /// * `bin_dir` - Path to the `bin/` directory of the installed Go toolchain.
    ///
    /// # Errors
    /// Returns [`GovmError::Io`] if reading the directory or writing shims fails.
    pub(crate) fn setup_shims_for_version(&self, bin_dir: &Path) -> Result<(), GovmError> {
        let started_at = Instant::now();

        // Sweep out any stale shims before generating the fresh set.
        self.cleanup_shims()?;

        // The directory is the one thing a caller cannot reconstruct after the
        // fact, so name it on the way out instead of letting a bare IO error
        // reach the UI as "No such file or directory (os error 2)".
        let entries = fs::read_dir(bin_dir).map_err(|e| {
            logging::error(&format!(
                "shim failed: action=read_bin_dir dir=\"{}\" error={e}",
                bin_dir.display()
            ));
            GovmError::Io(e)
        })?;

        let mut created: Vec<String> = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if path.is_file() {
                // Use `file_stem()` to get the binary name WITHOUT the extension.
                //
                // On Windows: `go.exe` → `go`, `gofmt.exe` → `gofmt`
                // On Unix:    `go`     → `go`, `gofmt`     → `gofmt` (no-op)
                let bin_name = match path.file_stem() {
                    Some(name) => name.to_string_lossy().to_string(),
                    None => continue,
                };
                self.create_shim(&bin_name, &path)?;
                created.push(bin_name);
            }
        }

        logging::info(&format!(
            "shim: created count={} names=[{}] for=\"{}\" elapsed_ms={}",
            created.len(),
            created.join(", "),
            bin_dir.display(),
            started_at.elapsed().as_millis()
        ));
        if created.is_empty() {
            // A silently empty shim set is how "go not found" starts.
            logging::warn(&format!(
                "shim: created nothing dir=\"{}\" note=no_executables_found",
                bin_dir.display()
            ));
        }
        Ok(())
    }

    // -------------------------------------- Internal Helpers -------------------------------------- //

    /// Removes all existing shim files from the shim directory.
    ///
    /// Called before generating a fresh shim set to guarantee no stale or
    /// incorrectly-named shims linger from a previous version or the legacy
    /// `go.exe.bat` naming bug.
    fn cleanup_shims(&self) -> Result<(), GovmError> {
        if !self.shim_dir.exists() {
            return Ok(());
        }
        let mut removed = 0usize;
        let mut failed = 0usize;
        for entry in fs::read_dir(&self.shim_dir)? {
            let path = entry?.path();
            if path.is_file() {
                // Removal failures stay non-fatal (the overwrite below still
                // works in the common case), but they are counted and reported.
                if fs::remove_file(&path).is_ok() {
                    removed += 1;
                } else {
                    failed += 1;
                }
            }
        }
        if failed > 0 {
            logging::warn(&format!(
                "shim: cleanup dir=\"{}\" removed={removed} failed={failed}",
                self.shim_dir.to_string_lossy()
            ));
        } else if removed > 0 {
            logging::debug(&format!(
                "shim: cleanup dir=\"{}\" removed={removed}",
                self.shim_dir.to_string_lossy()
            ));
        }
        Ok(())
    }

    /// Creates an executable POSIX shell script shim pointing to the target binary.
    #[cfg(unix)]
    fn create_shim(&self, bin_name: &str, target_path: &Path) -> Result<(), GovmError> {
        let shim_path = self.shim_dir.join(bin_name);
        let content = format!(
            "#!/usr/bin/env bash\n\"{}\" \"$@\"\n",
            target_path.display()
        );
        let mut file = File::create(&shim_path)?;
        file.write_all(content.as_bytes())?;
        fs::set_permissions(&shim_path, fs::Permissions::from_mode(0o755))?;
        logging::debug(&format!(
            "shim: wrote name={bin_name} path=\"{}\" target=\"{}\" mode=755",
            shim_path.display(),
            target_path.display()
        ));
        Ok(())
    }

    /// Creates an executable Windows batch file shim pointing to the target binary.
    #[cfg(windows)]
    fn create_shim(&self, bin_name: &str, target_path: &Path) -> Result<(), GovmError> {
        let shim_path = self.shim_dir.join(format!("{bin_name}.bat"));
        let content = format!("@echo off\r\n\"{}\" %*\r\n", target_path.display());
        let mut file = File::create(&shim_path)?;
        file.write_all(content.as_bytes())?;
        logging::debug(&format!(
            "shim: wrote name={bin_name} path=\"{}\" target=\"{}\"",
            shim_path.display(),
            target_path.display()
        ));
        Ok(())
    }
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Reports whether `dir` appears as an entry of the process `PATH`.
fn path_contains_dir(dir: &Path) -> bool {
    env::var_os("PATH").is_some_and(|paths| env::split_paths(&paths).any(|p| p == dir))
}
