//! Module manager - Core lifecycle coordinator for fetching, installing, switching, and deleting Go versions.

mod archive;
mod install;

pub use archive::check_archive_magic;
pub use install::InstallProgress;

use crate::{
    config::Config,
    errors::GovmError,
    logging,
    shim::ShimManager,
    theme::{Theme, ThemeName},
    version::{GoRelease, GoVersion, compare_versions},
};
use std::{
    env::consts::{ARCH, OS},
    fs,
    path::PathBuf,
    sync::Mutex,
    time::Duration,
};

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Primary orchestrator managing installed toolchains, downloads, and version switching.
pub struct GoManager {
    /// Base configuration directory (`~/.govmr`).
    base_dir: PathBuf,
    /// Root directory storing extracted Go toolchains (`~/.govmr/versions`).
    versions_dir: PathBuf,
    /// Directory storing temporary download archives (`~/.govmr/downloads`).
    downloads_dir: PathBuf,
    /// Handler for creating and managing executable binary shims.
    shim_mgr: ShimManager,
    /// Persisted user preferences (color theme, …), mutable behind a lock so the
    /// theme can be switched from the `Arc<GoManager>` used by the TUI and CLI.
    config: Mutex<Config>,
    /// Reusable asynchronous HTTP client for network operations.
    client: reqwest::Client,
}
impl GoManager {
    /// Initializes a new instance of `GoManager`, creating required directories if missing.
    ///
    /// # Errors
    /// Returns [`GovmError`] if directory creation or initialization fails.
    pub fn new() -> Result<Self, GovmError> {
        let home = dirs::home_dir().ok_or(GovmError::HomeNotFound)?;
        let base_dir = home.join(".govmr");
        let versions_dir = base_dir.join("versions");
        let downloads_dir = base_dir.join("downloads");

        fs::create_dir_all(&versions_dir)?;
        fs::create_dir_all(&downloads_dir)?;

        let config = Mutex::new(Config::load(&base_dir));

        Ok(Self {
            base_dir,
            versions_dir,
            downloads_dir,
            shim_mgr: ShimManager::new()?,
            config,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()?,
        })
    }

    /// Provides access to the underlying [`ShimManager`].
    pub fn get_shim_manager(&self) -> &ShimManager {
        &self.shim_mgr
    }

    /// Returns the user's currently selected color theme.
    pub fn theme_name(&self) -> ThemeName {
        // A poisoned lock only means another thread panicked mid-update; the
        // config value itself is still readable.
        self.config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .theme
    }

    /// Returns the concrete palette for the currently selected theme.
    pub fn theme(&self) -> Theme {
        Theme::for_name(self.theme_name())
    }

    /// Persists a new color-theme choice and returns the resulting palette.
    ///
    /// # Errors
    /// Returns [`GovmError::Io`] if the updated configuration cannot be persisted.
    pub fn set_theme(&self, theme: ThemeName) -> Result<Theme, GovmError> {
        self.config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_theme(theme)?;
        logging::info(&format!("theme set: {}", theme.key()));
        Ok(Theme::for_name(theme))
    }

    /// Retrieves the currently active Go version string from disk, if set.
    pub fn get_active_version(&self) -> Option<String> {
        let active_file = self.base_dir.join("active_version");
        fs::read_to_string(active_file)
            .ok()
            .map(|v| v.trim().to_string())
    }

    /// Queries `go.dev` for available Go releases and cross-references them against locally installed versions.
    ///
    /// # Errors
    /// Returns [`GovmError::Network`] if the request fails or [`GovmError::Io`] on filesystem read failure.
    pub async fn fetch_versions(&self) -> Result<Vec<GoVersion>, GovmError> {
        let url = "https://go.dev/dl/?mode=json&include=all";
        logging::debug(&format!("refresh: GET {url}"));
        let res = self.client.get(url).send().await.map_err(|e| {
            logging::error(&format!("refresh failed (request): {e}"));
            GovmError::from(e)
        })?;
        let status = res.status();
        if !status.is_success() {
            logging::error(&format!("refresh failed: HTTP {status} for {}", res.url()));
            return Err(GovmError::HttpStatus {
                status: status.as_u16(),
                url: res.url().to_string(),
            });
        }
        let releases: Vec<GoRelease> = res.json().await.map_err(|e| {
            logging::error(&format!("refresh failed (decode): {e}"));
            GovmError::from(e)
        })?;

        let go_os = match OS {
            "macos" => "darwin",
            other => other,
        };
        let go_arch = match ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            other => other,
        };

        let active_version = self.get_active_version();
        let mut versions = Vec::new();

        for release in releases {
            let ver_clean = release.version.trim_start_matches("go").to_string();
            if let Some(file) = release
                .files
                .into_iter()
                .find(|f| f.os == go_os && f.arch == go_arch)
            {
                let install_dir = self.versions_dir.join(format!("go{ver_clean}"));
                let installed = install_dir.join("bin").exists();
                let active = active_version.as_deref() == Some(&ver_clean);

                versions.push(GoVersion {
                    raw_version: ver_clean.clone(),
                    display_name: format!("go{ver_clean}"),
                    filename: file.filename.clone(),
                    url: format!("https://go.dev/dl/{}", file.filename),
                    size: file.size as u64,
                    installed,
                    active,
                    path: if installed { Some(install_dir) } else { None },
                    stable: release.stable,
                });
            }
        }

        versions.sort_by(|a, b| compare_versions(&b.raw_version, &a.raw_version));
        logging::info(&format!("refresh ok: {} versions listed", versions.len()));
        Ok(versions)
    }

    /// Sets the specified version as active by generating shims and recording selection on disk.
    ///
    /// # Returns
    /// Returns `true` if the shim directory is correctly configured in system `PATH`.
    ///
    /// # Errors
    /// Returns [`GovmError`] if shim generation or the active-version file
    /// write fails, or [`GovmError::NotInstalled`] if the version has no local path.
    pub fn switch_version(&self, version: &GoVersion) -> Result<bool, GovmError> {
        let version_path = version
            .path
            .as_ref()
            .ok_or_else(|| GovmError::NotInstalled(version.raw_version.clone()))?;
        let bin_dir = version_path.join("bin");

        self.shim_mgr.setup_shims_for_version(&bin_dir)?;

        let active_file = self.base_dir.join("active_version");
        fs::write(active_file, &version.raw_version)?;

        let is_in_path = self.shim_mgr.is_in_path();
        logging::info(&format!(
            "use: active version is now go{} (shim in PATH: {is_in_path})",
            version.raw_version
        ));
        Ok(is_in_path)
    }

    /// Deletes an installed Go version from disk.
    ///
    /// # Errors
    /// Returns [`GovmError::CannotDeleteActive`] if trying to delete the active version,
    /// or [`GovmError::NotInstalled`] if the version is not found locally.
    pub fn delete_version(&self, version: &GoVersion) -> Result<(), GovmError> {
        if !version.installed {
            return Err(GovmError::NotInstalled(version.raw_version.clone()));
        }
        if version.active {
            return Err(GovmError::CannotDeleteActive(version.raw_version.clone()));
        }
        if let Some(path) = &version.path
            && path.exists()
        {
            fs::remove_dir_all(path)?;
        }
        logging::info(&format!("delete: removed go{}", version.raw_version));
        Ok(())
    }

    /// Applies the permanent PATH fix by running the platform snippet in a
    /// hidden child process and returns a human-readable summary of
    /// exactly what was done, so the UI can reassure the user.
    ///
    /// * **Windows**: runs an idempotent `PowerShell` snippet that appends the
    ///   shim dir to the *User* PATH (no `setx`, so no 1024-char truncation).
    ///   Only takes effect in *new* terminal sessions (Windows limitation).
    /// * **Unix**: appends an `export PATH=...` line to the detected shell
    ///   profile (`~/.zshrc` / `~/.config/fish/config.fish` / `~/.bashrc`),
    ///   guarded by a marker comment so repeats never duplicate it.
    ///
    /// # Errors
    /// Returns [`GovmError`] if the home dir cannot be resolved, the profile
    /// cannot be written, or the child process fails to spawn/run.
    pub fn fix_path_permanently(&self) -> Result<Vec<String>, GovmError> {
        let shim_dir = self.shim_mgr.get_shim_dir();
        let shim = shim_dir.to_string_lossy().to_string();

        #[cfg(windows)]
        {
            use std::{os::windows::process::CommandExt, process};

            // Idempotent, truncation-safe User-PATH update. Hidden window so
            // the TUI is never clobbered by a console flash.
            let script = format!(
                "$p=[Environment]::GetEnvironmentVariable('PATH','User');\
                 if($p -notlike \"*{shim}*\"){{[Environment]::SetEnvironmentVariable('PATH',\"$p;{shim}\",'User')}}",
            );
            const CREATE_NO_WINDOW: u32 = 0x0800_0000; // Prevents a console window from appearing
            let status = process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    &script,
                ])
                .creation_flags(CREATE_NO_WINDOW) // CREATE_NO_WINDOW: never flash a console
                .status()?;
            if !status.success() {
                return Err(GovmError::Extraction(format!(
                    "PowerShell PATH fix exited with {}",
                    status
                )));
            }

            logging::info("fix-path: Windows User PATH updated (new terminals only)");
            Ok(vec![
                "Done — ran in a hidden PowerShell window:".to_string(),
                format!("    {script}"),
                "Open a NEW terminal for `go` to resolve.".to_string(),
            ])
        }

        #[cfg(unix)]
        {
            use std::io::Write;

            let home = dirs::home_dir().ok_or(GovmError::HomeNotFound)?;
            let profile = match std::env::var("SHELL").unwrap_or_default().as_str() {
                s if s.ends_with("/zsh") => home.join(".zshrc"),
                s if s.ends_with("/fish") => home.join(".config/fish/config.fish"),
                _ => home.join(".bashrc"),
            };
            let marker = "# Added by govmr";

            let existing = fs::read_to_string(&profile).unwrap_or_default();
            // Idempotency guard: never append the same line twice.
            if existing.lines().any(|l| l.trim_start().starts_with(marker)) {
                logging::info("fix-path: profile already patched, nothing to do");
                return Ok(vec![
                    format!(
                        "Already done — {} already contains the govmr export line.",
                        profile.display()
                    ),
                    "Open a NEW terminal (or source it) for `go` to resolve.".to_string(),
                ]);
            }

            let source_path = format!("export PATH=\"{shim}:$PATH\"");
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&profile)?;
            writeln!(file)?;
            writeln!(file, "{marker}")?;
            writeln!(file, "{source_path}")?;

            logging::info(&format!(
                "fix-path: appended export line to {}",
                profile.display()
            ));
            Ok(vec![
                format!("Done — appended to {}:", profile.display()),
                format!("    {source_path}"),
                "Open a NEW terminal (or source it) for `go` to resolve.".to_string(),
            ])
        }
    }
}
