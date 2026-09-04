//! Module shim - Shim generation and PATH validation utilities.

use crate::errors::GovmError;
use std::{
    env,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Manager responsible for generating executable shims and verifying environment PATH integrity.
pub struct ShimManager {
    /// Directory where generated shims reside (`~/.govmr/shim`).
    shim_dir: PathBuf,
}
impl ShimManager {
    // ----------------------------------------- Public API ----------------------------------------- //

    /// Creates a new `ShimManager`, ensuring the underlying shim directory exists.
    ///
    /// # Errors
    /// Returns [`GovmError::HomeNotFound`] if the user's home directory cannot be determined,
    /// or [`GovmError::Io`] if directory creation fails.
    pub fn new() -> Result<Self, GovmError> {
        let home = dirs::home_dir().ok_or(GovmError::HomeNotFound)?;
        let shim_dir = home.join(".govmr").join("shim");
        fs::create_dir_all(&shim_dir)?;
        Ok(Self { shim_dir })
    }

    /// Returns a reference to the directory containing executable shims.
    pub fn get_shim_dir(&self) -> &Path {
        &self.shim_dir
    }

    /// Checks if the GoVMR shim directory is present in the system's `PATH` environment variable.
    pub fn is_in_path(&self) -> bool {
        if let Some(paths) = env::var_os("PATH") {
            for path in env::split_paths(&paths) {
                if path == self.shim_dir {
                    return true;
                }
            }
        }
        false
    }

    /// Generates shims for all executables found within the specified version binary directory.
    ///
    /// # Arguments
    /// * `bin_dir` - Path to the `bin/` directory of the installed Go toolchain.
    ///
    /// # Errors
    /// Returns [`GovmError::Io`] if reading the directory or writing shims fails.
    pub fn setup_shims_for_version(&self, bin_dir: &Path) -> Result<(), GovmError> {
        // Sweep out any stale shims before generating the fresh set.
        self.cleanup_shims()?;

        for entry in fs::read_dir(bin_dir)? {
            let entry = entry?;
            let path = entry.path();
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
            }
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
        if self.shim_dir.exists() {
            for entry in fs::read_dir(&self.shim_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    let _ = fs::remove_file(&path);
                }
            }
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
        Ok(())
    }

    /// Creates an executable Windows batch file shim pointing to the target binary.
    #[cfg(windows)]
    fn create_shim(&self, bin_name: &str, target_path: &Path) -> Result<(), GovmError> {
        let shim_path = self.shim_dir.join(format!("{}.bat", bin_name));
        let content = format!("@echo off\r\n\"{}\" %*\r\n", target_path.display());
        let mut file = File::create(&shim_path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }
}
