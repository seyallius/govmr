//! Module manager - Core lifecycle coordinator for fetching, installing, switching, and deleting Go versions.

mod archive;
mod install;
mod update;

pub use archive::check_archive_magic;
pub use install::InstallProgress;

use crate::{
    completions,
    config::Config,
    errors::GovmError,
    logging,
    manager::update::replace_current_binary,
    shim::ShimManager,
    theme::{Theme, ThemeName},
    version::{GoRelease, GoVersion, compare_versions},
};
use std::{
    env::{
        self,
        consts::{ARCH, OS},
    },
    fs,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};

// ---------------------------------- Types, Variables & Constants ------------------------------ //

/// Prevents a console window from appearing.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Allow overriding the "current" version for testing purposes.
const GOVMR_TEST_VERSION: &str = "GOVMR_TEST_VERSION";

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
                .timeout(Duration::from_mins(5))
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
        logging::info(&format!(
            "theme: set name={} config=\"{}\"",
            theme.key(),
            self.base_dir.join("config.toml").display()
        ));
        Ok(Theme::for_name(theme))
    }

    /// Lists the versions present on disk as bare version strings, newest first.
    ///
    /// Purely local (a single `readdir`), so it is cheap enough to call from the
    /// session bookend where the log needs the final installed state.
    #[must_use]
    pub fn installed_versions(&self) -> Vec<String> {
        let mut versions = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.versions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                // Only count a directory as installed if it has a `bin/` — the
                // same rule the manifest cross-reference uses.
                if path.is_dir()
                    && path.join("bin").exists()
                    && let Some(name) = path.file_name().and_then(|n| n.to_str())
                {
                    versions.push(name.strip_prefix("go").unwrap_or(name).to_string());
                }
            }
        }
        versions.sort_by(|a, b| compare_versions(b, a));
        versions
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
        // Which files we will accept is part of the request, so normalize the
        // host identity *before* fetching and log it: "why is 1.20.14 missing"
        // is answered by the arch token, and the two lines must agree on names.
        let go_os = match OS {
            "macos" => "darwin",
            other => other,
        };
        let go_arch = match ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            other => other,
        };
        logging::debug(&format!(
            "refresh started: url={url} os={go_os} arch={go_arch}"
        ));
        let res = self.client.get(url).send().await.map_err(|e| {
            logging::error(&format!("refresh: failed stage=request error=\"{e}\""));
            GovmError::from(e)
        })?;
        let status = res.status();
        if !status.is_success() {
            logging::error(&format!(
                "refresh: failed stage=response status={status} url={}",
                res.url()
            ));
            return Err(GovmError::HttpStatus {
                status: status.as_u16(),
                url: res.url().to_string(),
            });
        }
        let releases: Vec<GoRelease> = res.json().await.map_err(|e| {
            logging::error(&format!("refresh: failed stage=decode error=\"{e}\""));
            GovmError::from(e)
        })?;

        let active_version = self.get_active_version();
        let mut versions = Vec::new();
        // Releases that ship no archive for this host OS/arch (e.g. a `linux-386`
        // only build). Counted so "missing version" reports can be told apart
        // from "version does not exist".
        let mut skipped = 0usize;

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
            } else {
                skipped += 1;
            }
        }

        versions.sort_by(|a, b| compare_versions(&b.raw_version, &a.raw_version));
        logging::info(&format!(
            "refresh ok: count={} os={go_os} arch={go_arch} skipped={skipped}",
            versions.len()
        ));
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
        // "Version X is not installed" is misleading unless the log says *where*
        // we looked, so a stale manifest entry can be told apart from a
        // genuinely missing toolchain directory.
        let Some(version_path) = version.path.as_ref() else {
            let expected = self.versions_dir.join(format!("go{}", version.raw_version));
            logging::warn(&format!(
                "use rejected: version={} reason=no_path_in_manifest checked=\"{}\" manifest_installed={} bin_present={}",
                version.raw_version,
                expected.display(),
                version.installed,
                expected.join("bin").exists()
            ));
            return Err(GovmError::NotInstalled(version.raw_version.clone()));
        };
        let bin_dir = version_path.join("bin");

        self.shim_mgr.setup_shims_for_version(&bin_dir)?;

        let active_file = self.base_dir.join("active_version");
        fs::write(&active_file, &version.raw_version)?;

        let is_in_path = self.shim_mgr.is_in_path();
        logging::info(&format!(
            "use: activated version={} shim_in_path={} active_file=\"{}\"",
            version.raw_version,
            is_in_path,
            active_file.display()
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
            let freed = dir_size(path);
            fs::remove_dir_all(path)?;
            logging::info(&format!(
                "delete: removed version={} freed_bytes={freed} freed=\"{}\" path=\"{}\"",
                version.raw_version,
                GoVersion::format_size(freed),
                path.display()
            ));
        } else {
            logging::warn(&format!(
                "delete: removed version={} freed_bytes=0 note=no_path_on_disk",
                version.raw_version
            ));
        }
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

            logging::info("fix-path: applied target=windows_user_path note=new_terminals_only");
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
                logging::info(&format!(
                    "fix-path: skipped profile=\"{}\" reason=already_patched",
                    profile.display()
                ));
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
                "fix-path: applied profile=\"{}\" line=\"{source_path}\"",
                profile.display()
            ));
            Ok(vec![
                format!("Done — appended to {}:", profile.display()),
                format!("    {source_path}"),
                "Open a NEW terminal (or source it) for `go` to resolve.".to_string(),
            ])
        }
    }

    /// Checks GitHub for a newer release tag. Returns `Some(version)` if an update is available.
    ///
    /// # Errors
    /// Returns [`GovmError`] if the release query or its response parsing fails.
    pub async fn check_for_update(&self) -> Result<Option<String>, GovmError> {
        logging::debug("update check: started");
        let res = self
            .client
            .get("https://api.github.com/repos/seyallius/govmr/releases/latest")
            .header("User-Agent", "govmr")
            .send()
            .await?;

        let status = res.status();
        if status.as_u16() == 404 {
            logging::debug("update check: complete status=404 note=no_public_releases");
            return Ok(None);
        }

        let json: serde_json::Value = res
            .json()
            .await
            .map_err(|e| GovmError::Extraction(e.to_string()))?;

        let tag = json["tag_name"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches('v');

        logging::debug(&format!(
            "update check: complete status={} tag={}",
            status,
            if tag.is_empty() { "none" } else { tag }
        ));

        let current =
            env::var(GOVMR_TEST_VERSION).unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
        if !tag.is_empty() && tag != current {
            Ok(Some(tag.to_string()))
        } else {
            Ok(None)
        }
    }

    /// Downloads the latest release archive, extracts the binary, and replaces the current executable.
    ///
    /// # Errors
    /// Returns [`GovmError`] if any download, archive-extraction, or file-replacement step fails.
    pub async fn perform_update(&self, version: &str) -> Result<(), GovmError> {
        let os = match OS {
            "macos" => "apple-darwin",
            "linux" => "unknown-linux-gnu",
            "windows" => "pc-windows-msvc",
            other => other,
        };
        let arch = ARCH;
        let target = format!("{arch}-{os}");
        let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
        let url = format!(
            "https://github.com/seyallius/govmr/releases/download/v{version}/govmr-v{version}-{target}.{ext}"
        );

        let current =
            env::var(GOVMR_TEST_VERSION).unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
        logging::info(&format!(
            "update: downloading current={current} target={version} url={url}"
        ));
        let started_at = Instant::now();

        let res = self.client.get(&url).send().await?;
        let status = res.status();
        if !status.is_success() {
            return Err(GovmError::HttpStatus {
                status: status.as_u16(),
                url: res.url().to_string(),
            });
        }
        let bytes = res.bytes().await?;
        let archive_bytes = bytes.len() as u64;
        logging::debug(&format!(
            "update: archive fetched target={version} bytes={archive_bytes} size=\"{}\" elapsed_ms={}",
            GoVersion::format_size(archive_bytes),
            started_at.elapsed().as_millis()
        ));

        let temp_dir = std::env::temp_dir().join("govmr_update");
        let _ = fs::create_dir_all(&temp_dir);
        let archive_path = temp_dir.join(format!("govmr.{ext}"));
        fs::write(&archive_path, &bytes)?;

        let bin_name = if cfg!(windows) { "govmr.exe" } else { "govmr" };
        let new_bin_path = temp_dir.join(bin_name);
        let current_exe = replace_current_binary(&new_bin_path)?;

        logging::info(&format!(
            "update: complete current={current} target={version} exe=\"{}\" elapsed_ms={}",
            current_exe.display(),
            started_at.elapsed().as_millis()
        ));
        Ok(())
    }

    /// Removes the govmr binary and optionally purges the ~/.govmr directory.
    ///
    /// The binary is removed FIRST. If this step fails, the function aborts
    /// immediately without touching completions or `~/.govmr`, preventing a
    /// half-uninstalled state where the binary remains but its support files are gone.
    ///
    /// # Errors
    /// Returns [`GovmError`] if the home directory cannot be found or any
    /// file-removal step fails.
    pub fn uninstall(&self, purge: bool) -> Result<(), GovmError> {
        let exe = std::env::current_exe()?;

        // 1. Remove the binary FIRST.
        #[cfg(windows)]
        {
            use std::{os::windows::process::CommandExt, process};

            let exe_path = exe.to_string_lossy().to_string();
            // Escape single quotes for PowerShell string literals
            let escaped_path = exe_path.replace('\'', "''");

            // Spawn a detached PowerShell process that waits for us to exit,
            // then deletes the executable. This avoids the ".old" leftover file.
            let script =
                format!("Start-Sleep -Seconds 2; Remove-Item -Force -LiteralPath '{escaped_path}'");

            match process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    &script,
                ])
                .creation_flags(CREATE_NO_WINDOW) // CREATE_NO_WINDOW: never flash a console
                .spawn()
            {
                Ok(_) => logging::info("uninstall: scheduled exe=removed_on_exit"),
                Err(e) => logging::warn(&format!(
                    "uninstall: schedule failed error=\"{e}\" note=delete_the_executable_manually"
                )),
            }
        }

        #[cfg(not(windows))]
        {
            fs::remove_file(&exe)?;
            logging::info(&format!("uninstall: removed exe=\"{}\"", exe.display()));
        }

        // 2. Binary is gone (or scheduled to be). Now clean up completions.
        completions::remove_completions();

        // 3. Finally, purge ~/.govmr if requested.
        if purge {
            let home = dirs::home_dir().ok_or(GovmError::HomeNotFound)?;
            let base_dir = home.join(".govmr");
            if base_dir.exists() {
                fs::remove_dir_all(&base_dir)?;
                logging::info(&format!(
                    "uninstall: purged path=\"{}\"",
                    base_dir.display()
                ));
            }
        }

        Ok(())
    }
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Recursively sums the size of a directory tree, for "how much space did this
/// free?" reporting.
///
/// Best-effort by design: unreadable or vanished entries count as zero rather
/// than failing the operation that merely wanted a number for the log. Symlinks
/// are measured, not followed, so a link cannot be double-counted or loop.
fn dir_size(path: &std::path::Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let child = entry.path();
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => dir_size(&child),
                _ => fs::symlink_metadata(&child).map_or(0, |meta| meta.len()),
            }
        })
        .sum()
}
