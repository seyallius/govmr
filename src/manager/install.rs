//! Module install - Streaming download of toolchain archives with progress reporting.

use super::{GoManager, archive};
use crate::{errors::GovmError, logging, version::GoVersion};
use futures_util::StreamExt;
use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};
use tokio::io::AsyncWriteExt;

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Lifecycle progress events emitted while a toolchain is being installed.
#[derive(Debug, Clone, Copy)]
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
        let expected_bytes = version.size;
        // `size` is the manifest's own figure; when it is absent (0) say so
        // rather than printing a misleading "0 B" as if it were measured.
        let size_note = if expected_bytes > 0 {
            format!(" size=\"{}\"", GoVersion::format_size(expected_bytes))
        } else {
            " size=unknown".to_string()
        };
        logging::info(&format!(
            "install: started version={} url={} dest=\"{}\" bytes={expected_bytes}{size_note}",
            version.raw_version,
            version.url,
            download_path.display()
        ));
        let started_at = Instant::now();

        let is_tar = version.filename.ends_with(".tar.gz");
        self.download_archive(version, &download_path, is_tar, &progress)
            .await?;

        if target_dir.exists() {
            fs::remove_dir_all(&target_dir)?;
        }
        fs::create_dir_all(&target_dir)?;

        let dl_path_clone = download_path.clone();
        let target_dir_clone = target_dir.clone();

        // Measured before extraction deletes the archive, so the log records how
        // much was on disk even when unpacking then fails halfway through.
        let archive_bytes = fs::metadata(&download_path).map_or(0, |meta| meta.len());

        // The archive is fully on disk, so the blocking extraction work moves
        // onto the blocking pool instead of stalling the async runtime.
        let extraction = tokio::task::spawn_blocking(move || {
            archive::extract_archive(&dl_path_clone, &target_dir_clone, is_tar)
        })
        .await
        .map_err(|e| GovmError::Extraction(e.to_string()))?;

        // Deliberately not logged here: `handle_install_failed` owns the single
        // `install: failed` line, and returning twice as loud is how one 404
        // used to become two ERROR lines.
        extraction?;

        logging::info(&format!(
            "install: complete version={} dest=\"{}\" archive_bytes={archive_bytes} elapsed_s={:.1}",
            version.raw_version,
            target_dir.display(),
            started_at.elapsed().as_secs_f64()
        ));
        Ok(target_dir)
    }

    /// Streams the archive for `version` to `download_path`, reporting live
    /// progress, then validates the payload's magic bytes before extraction.
    ///
    /// # Arguments
    /// * `version` - The version metadata to install.
    /// * `download_path` - Destination file the archive is streamed into.
    /// * `is_tar` - Whether a `.tar.gz` (true) or `.zip` payload is expected.
    /// * `progress` - Callback invoked with [`InstallProgress`] events.
    ///
    /// # Errors
    /// Returns [`GovmError`] on download failure, IO interruption, or an
    /// unexpected (non-archive) payload.
    async fn download_archive<F>(
        &self,
        version: &GoVersion,
        download_path: &Path,
        is_tar: bool,
        progress: &F,
    ) -> Result<(), GovmError>
    where
        F: Fn(InstallProgress),
    {
        // Local timer for the transfer only; the install-wide duration (which also
        // covers extraction) is measured by the caller.
        let transfer_started_at = Instant::now();
        let res = self.client.get(&version.url).send().await?;
        let status = res.status();
        let final_url = res.url().clone(); // where we *actually* ended up
        logging::debug(&format!(
            "download: response status={status} url={final_url} requested={}",
            version.url
        ));
        // No log line for a bad status: the `download response` DEBUG line above
        // already carries status + final URL, and the handler logs the failure.
        if !status.is_success() {
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

        let mut file = tokio::fs::File::create(download_path).await?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            // Emit progress at most ~4x per second, or whenever a whole percent is crossed,
            // so high-frequency chunk arrivals don't flood the UI.
            let now = Instant::now();
            let elapsed = now.duration_since(last_report).as_secs_f64();
            if elapsed >= 0.25 || total_size == 0 {
                // Byte deltas stay far below 2^53, so the f64 cast cannot lose
                // precision for any realistic download.
                #[allow(clippy::cast_precision_loss)]
                let instant_speed = (downloaded - last_bytes) as f64 / elapsed.max(1e-3);
                // Exponential moving average for a smoother speed readout.
                smoothed_speed = if smoothed_speed == 0.0 {
                    instant_speed
                } else {
                    smoothed_speed * 0.6 + instant_speed * 0.4
                };

                // The ratio is bounded to 0..=100 by construction, so the u8
                // cast can neither truncate meaningfully nor flip the sign.
                #[allow(
                    clippy::cast_precision_loss,
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss
                )]
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
        // Integer math keeps the figures exact and free of lossy float casts;
        // `max(1)` guarantees the divide-by-zero case cannot happen.
        let elapsed_ms = transfer_started_at.elapsed().as_millis().max(1);
        let avg_bps = u64::try_from(u128::from(downloaded) * 1000 / elapsed_ms).unwrap_or(u64::MAX);
        logging::info(&format!(
            "download: complete version={} bytes={} dest=\"{}\" elapsed_ms={elapsed_ms} avg_bps={avg_bps}",
            version.raw_version,
            downloaded,
            download_path.display()
        ));

        if total_size > 0 && downloaded != total_size {
            logging::warn(&format!(
                "download: size mismatch expected_bytes={total_size} received_bytes={downloaded} url={final_url}"
            ));
        }

        // Final 100% report, then flip to the extraction phase.
        progress(InstallProgress::Downloading {
            downloaded,
            total: total_size,
            bytes_per_sec: smoothed_speed,
        });
        progress(InstallProgress::Extracting);

        // Forensic breadcrumb: what did we ACTUALLY save? `1f 8b` = real gzip;
        // `3c 68 74 6d` ("<htm") = HTML error page; plain tar bytes = something
        // pre-decoded our stream. Read once and reuse the same head for the
        // magic check — probing twice used to write the identical line twice.
        let head = archive::read_head(download_path, 16);
        if let Some(bytes) = &head {
            let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
            logging::debug(&format!(
                "archive: head path=\"{}\" bytes=\"{}\" expected=\"{}\"",
                download_path.display(),
                hex.join(" "),
                if is_tar { "1f 8b" } else { "50 4b" }
            ));
        }
        archive::check_archive_magic(
            head.as_deref().unwrap_or_default(),
            is_tar,
            final_url.as_str(),
        )?;

        Ok(())
    }
}
