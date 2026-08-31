//! Copyright (c) 2026 SeyedAli
//! Licensed under the MIT License. See LICENSE file in the project root for details.
//
//! Module manager - Core lifecycle coordinator for fetching, installing, switching, and deleting Go versions.

use crate::{
    config::Config,
    errors::GovmError,
    logging,
    models::{GoRelease, GoVersion},
    shim::ShimManager,
    theme::{Theme, ThemeName},
};
use futures_util::StreamExt;
use std::{
    env::consts::{ARCH, OS},
    fs::{self, File},
    io::Read,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant},
};
use tokio::io::AsyncWriteExt;

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Lifecycle progress events emitted while a toolchain is being installed.
#[derive(Debug, Clone)]
pub enum InstallProgress {
    /// A chunk of the archive finished downloading.
    Downloading {
        /// Number of bytes downloaded so far.
        downloaded: u64,
        /// Total archive size in bytes (0 if the server did not report it).
        total: u64,
        /// Smoothed download speed in bytes per second.
        bytes_per_sec: f64,
    },
    /// The archive has finished downloading and is being unpacked to disk.
    Extracting,
}

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
    // ----------------------------------------- Public API ----------------------------------------- //

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
        self.config.lock().expect("config lock").theme
    }

    /// Returns the concrete palette for the currently selected theme.
    pub fn theme(&self) -> Theme {
        Theme::for_name(self.theme_name())
    }

    /// Persists a new color-theme choice and returns the resulting palette.
    pub fn set_theme(&self, theme: ThemeName) -> Result<Theme, GovmError> {
        self.config.lock().expect("config lock").set_theme(theme)?;
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
                let install_dir = self.versions_dir.join(format!("go{}", ver_clean));
                let installed = install_dir.join("bin").exists();
                let active = active_version.as_deref() == Some(&ver_clean);

                versions.push(GoVersion {
                    raw_version: ver_clean.clone(),
                    display_name: format!("go{}", ver_clean),
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

        versions.sort_by(|a, b| Self::compare_versions(&b.raw_version, &a.raw_version));
        logging::info(&format!("refresh ok: {} versions listed", versions.len()));
        Ok(versions)
    }

    /// Asynchronously streams and extracts a target Go toolchain archive.
    ///
    /// # Arguments
    /// * `version` - The version metadata to install.
    /// * `progress` - Callback invoked with [`InstallProgress`] events as the install advances.
    ///
    /// # Errors
    /// Returns [`GovmError`] on download failure, IO interruption, or extraction error.
    pub async fn download_and_install<F>(
        &self,
        version: &GoVersion,
        progress: F,
    ) -> Result<PathBuf, GovmError>
    where
        F: Fn(InstallProgress) + Send + 'static,
    {
        let download_path = self.downloads_dir.join(&version.filename);
        let target_dir = self.versions_dir.join(format!("go{}", version.raw_version));
        logging::info(&format!(
            "install started: go{} url={} dest={}",
            version.raw_version,
            version.url,
            download_path.display()
        ));

        let res = self.client.get(&version.url).send().await?;
        let status = res.status();
        let final_url = res.url().clone(); // where we *actually* ended up
        logging::debug(&format!(
            "download response: HTTP {status} (final url: {final_url})"
        ));
        if !status.is_success() {
            logging::error(&format!(
                "install failed: go{}: HTTP {status} for {final_url}",
                version.raw_version
            ));
            return Err(GovmError::HttpStatus {
                status: status.as_u16(),
                url: final_url.to_string(),
            });
        }
        let total_size = res.content_length().unwrap_or(version.size);
        let mut downloaded: u64 = 0;
        let mut stream = res.bytes_stream();

        // Throttling / speed estimation state.
        let mut last_report = Instant::now();
        let mut last_bytes: u64 = 0;
        let mut smoothed_speed: f64 = 0.0;
        let mut last_pct_reported: u8 = 0;

        let mut file = tokio::fs::File::create(&download_path).await?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            // Emit progress at most ~4x per second, or whenever a whole percent is crossed,
            // so high-frequency chunk arrivals don't flood the UI.
            let now = Instant::now();
            let elapsed = now.duration_since(last_report).as_secs_f64();
            if elapsed >= 0.25 || total_size == 0 {
                let instant_speed = (downloaded - last_bytes) as f64 / elapsed.max(1e-3);
                // Exponential moving average for a smoother speed readout.
                smoothed_speed = if smoothed_speed == 0.0 {
                    instant_speed
                } else {
                    smoothed_speed * 0.6 + instant_speed * 0.4
                };

                let pct = if total_size > 0 {
                    ((downloaded as f64 / total_size as f64) * 100.0) as u8
                } else {
                    0
                };
                if elapsed >= 0.25 || pct > last_pct_reported || total_size == 0 {
                    progress(InstallProgress::Downloading {
                        downloaded,
                        total: total_size,
                        bytes_per_sec: smoothed_speed,
                    });
                    last_pct_reported = pct;
                    last_report = now;
                    last_bytes = downloaded;
                }
            }
        }
        file.flush().await?;
        logging::info(&format!(
            "download complete: go{} {} bytes -> {}",
            version.raw_version,
            downloaded,
            download_path.display()
        ));

        // Forensic breadcrumb: what did we ACTUALLY save? `1f 8b` = real gzip;
        // `3c 68 74 6d` ("<htm") = HTML error page; plain tar bytes = something
        // pre-decoded our stream.
        if let Ok(mut probe) = File::open(&download_path) {
            let mut head = [0u8; 16];
            if let Ok(n) = probe.read(&mut head) {
                let hex: Vec<String> = head[..n].iter().map(|b| format!("{b:02x}")).collect();
                logging::debug(&format!("archive head: {}", hex.join(" ")));
            }
        }

        if total_size > 0 && downloaded != total_size {
            logging::warn(&format!(
                "size mismatch: expected {total_size} bytes, received {downloaded} ({final_url})"
            ));
        }

        // Final 100% report, then flip to the extraction phase.
        progress(InstallProgress::Downloading {
            downloaded,
            total: total_size,
            bytes_per_sec: smoothed_speed,
        });
        progress(InstallProgress::Extracting);

        let is_tar = version.filename.ends_with(".tar.gz");
        let head = read_head(&download_path, 16);
        if let Some(bytes) = &head {
            let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
            logging::debug(&format!("archive head: {}", hex.join(" ")));
        }
        check_archive_magic(
            head.as_deref().unwrap_or_default(),
            is_tar,
            &final_url.to_string(),
        )?;

        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)?;
        }
        fs::create_dir_all(&target_dir)?;

        let dl_path_clone = download_path.clone();
        let target_dir_clone = target_dir.clone();

        let extraction = tokio::task::spawn_blocking(move || -> Result<(), GovmError> {
            if is_tar {
                let tar_gz = File::open(&dl_path_clone)?;
                let tar = flate2::read::GzDecoder::new(tar_gz);
                let mut archive = tar::Archive::new(tar);

                for entry in archive.entries()? {
                    let mut entry = entry?;
                    let path = entry.path()?;
                    let stripped: PathBuf = path.components().skip(1).collect();
                    if !stripped.as_os_str().is_empty() {
                        let out_path = target_dir_clone.join(stripped);
                        if entry.header().entry_type().is_dir() {
                            fs::create_dir_all(&out_path)?;
                        } else {
                            if let Some(p) = out_path.parent() {
                                fs::create_dir_all(p)?;
                            }
                            entry.unpack(&out_path)?;
                        }
                    }
                }
            } else {
                let zip_file = File::open(&dl_path_clone)?;
                let mut archive = zip::ZipArchive::new(zip_file)
                    .map_err(|e| GovmError::Extraction(e.to_string()))?;
                for i in 0..archive.len() {
                    let mut file = archive
                        .by_index(i)
                        .map_err(|e| GovmError::Extraction(e.to_string()))?;
                    let outpath = match file.enclosed_name() {
                        Some(path) => {
                            let stripped: PathBuf = path.components().skip(1).collect();
                            target_dir_clone.join(stripped)
                        }
                        None => continue,
                    };
                    if (*file.name()).ends_with('/') {
                        fs::create_dir_all(&outpath)?;
                    } else {
                        if let Some(p) = outpath.parent() {
                            fs::create_dir_all(p)?;
                        }
                        let mut outfile = File::create(&outpath)?;
                        std::io::copy(&mut file, &mut outfile)?;
                    }
                }
            }
            let _ = fs::remove_file(dl_path_clone);
            Ok(())
        })
        .await
        .map_err(|e| GovmError::Extraction(e.to_string()))?;

        if let Err(e) = &extraction {
            logging::error(&format!("install failed: go{}: {e}", version.raw_version));
        }
        extraction?;

        logging::info(&format!(
            "install complete: go{} -> {}",
            version.raw_version,
            target_dir.display()
        ));
        Ok(target_dir)
    }

    /// Sets the specified version as active by generating shims and recording selection on disk.
    ///
    /// # Returns
    /// Returns `true` if the shim directory is correctly configured in system `PATH`.
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
        if let Some(path) = &version.path {
            if path.exists() {
                fs::remove_dir_all(path)?;
            }
        }
        logging::info(&format!("delete: removed go{}", version.raw_version));
        Ok(())
    }

    /// Compares two semver-like version strings numerically.
    pub fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
        let parse = |v: &str| -> Vec<u32> {
            v.split('.')
                .filter_map(|p| {
                    p.chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .parse()
                        .ok()
                })
                .collect()
        };
        parse(v1).cmp(&parse(v2))
    }
}

// ----------------------------------------- Public API ----------------------------------------- //

/// Validates that a downloaded payload starts with the expected archive magic.
///
/// gzip archives begin with `1f 8b`, zip archives with `50 4b` ("PK"). Anything
/// else — most commonly an HTML error or proxy page — is rejected *before*
/// extraction so users get an actionable error instead of "invalid gzip header".
pub fn check_archive_magic(head: &[u8], is_tar: bool, url: &str) -> Result<(), GovmError> {
    let magic: &[u8] = if is_tar { &[0x1f, 0x8b] } else { &[0x50, 0x4b] };
    if head.len() >= 2 && &head[..2] == magic {
        return Ok(());
    }
    let hex: Vec<String> = head.iter().map(|b| format!("{b:02x}")).collect();
    Err(GovmError::NotAnArchive {
        kind: if is_tar { "tar.gz" } else { "zip" }.to_string(),
        head: hex.join(" "),
        url: url.to_string(),
    })
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Best-effort reader of a file's first `n` bytes, used for magic-byte sniffing.
/// Returns `None` if the file can't be opened/read (validation then fails safely).
fn read_head(path: &std::path::Path, n: usize) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    let mut buf = vec![0u8; n];
    let read = file.read(&mut buf).ok()?;
    buf.truncate(read);
    Some(buf)
}
