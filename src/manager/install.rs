//! Module install - Streaming download of toolchain archives with progress reporting.

use super::{archive, GoManager};
use crate::{errors::GovmError, logging, version::GoVersion};
use futures_util::StreamExt;
use std::{
    fs::{self, File},
    io::Read,
    path::PathBuf,
    time::Instant,
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

impl GoManager {
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
        let head = archive::read_head(&download_path, 16);
        if let Some(bytes) = &head {
            let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
            logging::debug(&format!("archive head: {}", hex.join(" ")));
        }
        archive::check_archive_magic(
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

        // The archive is fully on disk, so the blocking extraction work moves
        // onto the blocking pool instead of stalling the async runtime.
        let extraction = tokio::task::spawn_blocking(move || {
            archive::extract_archive(&dl_path_clone, &target_dir_clone, is_tar)
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
}
