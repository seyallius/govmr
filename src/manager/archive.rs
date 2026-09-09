//! Module archive - Archive format validation and extraction for downloaded toolchains.
//!
//! Extraction is hundreds of megabytes and the step most likely to be interrupted,
//! so it logs what it opened, what it wrote, what it refused, and how long it took.
//! Entries that are *not* written are counted here, because a silently truncated
//! toolchain otherwise looks like a successful install.

use crate::{errors::GovmError, logging};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};

// ------------------------------------------ Types & Impls ------------------------------------- //

/// Per-entry accounting for one extraction pass, reported on the closing line.
#[derive(Debug, Default)]
struct ExtractStats {
    /// Regular files written to disk.
    files: usize,
    /// Directories created.
    dirs: usize,
    /// Entries intentionally not written (the wrapper root of a Go archive).
    skipped: usize,
    /// Entries rejected as unsafe paths (zip traversal attempts).
    unsafe_paths: usize,
}
impl ExtractStats {
    /// Formats the counters as the trailing fields of an `extract` log line.
    fn as_fields(&self) -> String {
        format!(
            "files={} dirs={} skipped={} unsafe={}",
            self.files, self.dirs, self.skipped, self.unsafe_paths
        )
    }
}

// ----------------------------------------- Public API ----------------------------------------- //

/// Validates that a downloaded payload starts with the expected archive magic.
///
/// gzip archives begin with `1f 8b`, zip archives with `50 4b` ("PK"). Anything
/// else — most commonly an HTML error or proxy page — is rejected *before*
/// extraction so users get an actionable error instead of "invalid gzip header".
///
/// The rejected bytes are logged here because no other layer still has them: by
/// the time the error reaches the UI the payload has been reported once and the
/// `NotAnArchive` message is what the user sees, not what the log keeps.
///
/// # Errors
/// Returns [`GovmError::NotAnArchive`] when the payload does not start with
/// the expected magic bytes.
pub fn check_archive_magic(head: &[u8], is_tar: bool, url: &str) -> Result<(), GovmError> {
    let magic: &[u8] = if is_tar { &[0x1f, 0x8b] } else { &[0x50, 0x4b] };
    if head.len() >= 2 && &head[..2] == magic {
        return Ok(());
    }
    let hex: Vec<String> = head.iter().map(|b| format!("{b:02x}")).collect();
    let head_hex = hex.join(" ");
    let expected_hex = magic
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    let kind = if is_tar { "tar.gz" } else { "zip" };

    logging::error(&format!(
        "archive: rejected kind={kind} expected=\"{expected_hex}\" head=\"{head_hex}\" bytes={} url={url}",
        head.len()
    ));
    Err(GovmError::NotAnArchive {
        kind: kind.to_string(),
        head: head_hex,
        url: url.to_string(),
    })
}

/// Extracts a downloaded archive into `target_dir`, stripping the wrapper
/// directory each Go archive ships inside (the leading `goX.Y.Z/` root), and
/// finally removes the archive file.
///
/// Blocking by design: callers run this via `tokio::task::spawn_blocking`.
pub(crate) fn extract_archive(
    download_path: &Path,
    target_dir: &Path,
    is_tar: bool,
) -> Result<(), GovmError> {
    let kind = if is_tar { "tar.gz" } else { "zip" };
    let archive_bytes = fs::metadata(download_path).map_or(0, |meta| meta.len());
    let started_at = Instant::now();
    let mut stats = ExtractStats::default();

    logging::info(&format!(
        "extract: started kind={kind} archive=\"{}\" bytes={archive_bytes} target=\"{}\"",
        download_path.display(),
        target_dir.display()
    ));

    if is_tar {
        let tar_gz = File::open(download_path)?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let stripped: PathBuf = path.components().skip(1).collect();
            if stripped.as_os_str().is_empty() {
                // The wrapper root (`go/`) itself — nothing to write, by design.
                stats.skipped += 1;
                continue;
            }
            let out_path = target_dir.join(stripped);
            if entry.header().entry_type().is_dir() {
                fs::create_dir_all(&out_path)?;
                stats.dirs += 1;
            } else {
                if let Some(p) = out_path.parent() {
                    fs::create_dir_all(p)?;
                }
                entry.unpack(&out_path)?;
                stats.files += 1;
            }
        }
    } else {
        let zip_file = File::open(download_path)?;
        let mut archive =
            zip::ZipArchive::new(zip_file).map_err(|e| GovmError::Extraction(e.to_string()))?;
        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| GovmError::Extraction(e.to_string()))?;
            // `enclosed_name()` rejects path traversal ("../", absolute). Dropping
            // such an entry is correct, but a non-zero count on a Go archive means
            // we were handed something hostile or broken, so it is counted.
            let Some(path) = file.enclosed_name() else {
                stats.unsafe_paths += 1;
                continue;
            };
            let stripped: PathBuf = path.components().skip(1).collect();
            let outpath = target_dir.join(stripped);
            if (*file.name()).ends_with('/') {
                fs::create_dir_all(&outpath)?;
                stats.dirs += 1;
            } else {
                if let Some(p) = outpath.parent() {
                    fs::create_dir_all(p)?;
                }
                let mut outfile = File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
                stats.files += 1;
            }
        }
    }

    let elapsed_ms = started_at.elapsed().as_millis();
    logging::info(&format!(
        "extract: complete kind={kind} target=\"{}\" {} elapsed_ms={elapsed_ms}",
        target_dir.display(),
        stats.as_fields()
    ));
    if stats.unsafe_paths > 0 {
        logging::warn(&format!(
            "extract: rejected path: count={} archive=\"{}\" reason=outside_target_dir",
            stats.unsafe_paths,
            download_path.display()
        ));
    }
    if stats.files == 0 {
        logging::warn(&format!(
            "extract: wrote no files kind={kind} archive=\"{}\" bytes={archive_bytes} note=toolchain_unusable",
            download_path.display()
        ));
    }

    // The archive is only removed once the tree is complete, and its removal is
    // logged too: a leftover in ~/.govmr/downloads otherwise looks intentional.
    match fs::remove_file(download_path) {
        Ok(()) => logging::debug(&format!(
            "extract: removed archive path=\"{}\" bytes={archive_bytes}",
            download_path.display()
        )),
        Err(e) => logging::warn(&format!(
            "extract: archive removal failed path=\"{}\" error={e}",
            download_path.display()
        )),
    }
    Ok(())
}

/// Best-effort reader of a file's first `n` bytes, used for magic-byte sniffing.
/// Returns `None` if the file can't be opened/read (validation then fails safely).
pub(crate) fn read_head(path: &Path, n: usize) -> Option<Vec<u8>> {
    let mut file = File::open(path).ok()?;
    let mut buf = vec![0u8; n];
    let read = file.read(&mut buf).ok()?;
    buf.truncate(read);
    Some(buf)
}

// -------------------------------------------- Tests ------------------------------------------- //

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::expect_used)]

    use super::*;
    use flate2::Compression;

    /// Builds a Go-style `.tar.gz` from `src`, putting every entry under a
    /// wrapper `go/` directory exactly like the real toolchain archives.
    fn write_tar_gz_from(path: &Path, src: &Path) {
        let file = File::create(path).expect("create archive");
        let mut builder =
            tar::Builder::new(flate2::write::GzEncoder::new(file, Compression::default()));
        builder.append_dir_all("go", src).expect("append tree");
        builder.finish().expect("finish tar");
        builder
            .into_inner()
            .expect("into_inner")
            .try_finish()
            .expect("finish gzip");
    }

    /// Log lines with the `YYYY-MM-DD HH:MM:SSZ LEVEL ` prefix removed, so the
    /// assertions read like the messages themselves.
    fn read_log(path: &Path) -> Vec<String> {
        fs::read_to_string(path)
            .expect("log file")
            .lines()
            .map(|line| crate::logging::message_of(line).to_string())
            .collect()
    }

    /// Extraction is hundreds of megabytes and the step most likely to be
    /// interrupted, so its start, outcome and cleanup must all be in the log.
    #[test]
    fn extract_reports_start_counts_and_cleanup() {
        let dir = std::env::temp_dir().join(format!("govmr-extract-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch");
        let log = dir.join("govmr.log");
        logging::init_in(&log);

        let payload = dir.join("payload");
        fs::create_dir_all(payload.join("bin")).expect("payload tree");
        fs::write(payload.join("bin/go"), b"#!/bin/sh\n").expect("go");
        fs::write(payload.join("VERSION"), b"go1.27.1\n").expect("VERSION");

        let archive = dir.join("go1.27.1.linux-amd64.tar.gz");
        write_tar_gz_from(&archive, &payload);
        let expected_bytes = fs::metadata(&archive).expect("meta").len();
        let target = dir.join("versions/go1.27.1");
        fs::create_dir_all(&target).expect("target");

        extract_archive(&archive, &target, true).expect("extract");

        let lines = read_log(&log);
        let started = lines
            .iter()
            .find(|l| l.starts_with("extract: started "))
            .expect("extract must log a start line");
        assert!(
            started.contains("kind=tar.gz")
                && started.contains(&format!("bytes={expected_bytes}"))
                && started.contains(&format!("target=\"{}\"", target.display())),
            "start line must carry kind, archive size and target: {started}"
        );

        let done = lines
            .iter()
            .find(|l| l.starts_with("extract: complete "))
            .expect("extract must log a completion line");
        assert!(
            done.contains("files=2")
                && done.contains("dirs=1")
                && done.contains("skipped=1")
                && done.contains("unsafe=0")
                && done.contains("elapsed_ms="),
            "completion line must report per-entry counts, got: {done}"
        );
        assert!(
            fs::read(target.join("bin/go"))
                .unwrap()
                .starts_with(b"#!/bin/sh"),
            "the payload must actually be on disk"
        );

        // The archive is consumed on success, and the removal is logged so a
        // missing download looks intentional rather than like a lost file.
        assert!(
            !archive.exists()
                && lines
                    .iter()
                    .any(|l| l.starts_with("extract: removed archive ")),
            "removal of the archive must be logged, got:\n{lines:#?}"
        );

        // An archive that yields nothing is a broken install, not a success.
        let empty_payload = dir.join("empty-payload");
        fs::create_dir_all(&empty_payload).expect("empty payload");
        let empty = dir.join("empty.tar.gz");
        write_tar_gz_from(&empty, &empty_payload);
        extract_archive(&empty, &dir.join("empty-target"), true).expect("extract empty");
        let lines = read_log(&log);
        assert!(
            lines
                .iter()
                .any(|l| l.starts_with("extract: wrote no files")
                    && l.contains("note=toolchain_unusable")),
            "a zero-file extraction must be called out, got:\n{lines:#?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// A payload that is not the expected archive is the one case where the
    /// failing layer logs, because no other layer still has the bytes.
    #[test]
    fn magic_check_reports_the_actual_head_bytes() {
        let err = check_archive_magic(b"<htm", true, "https://x/y.tar.gz")
            .expect_err("html head rejected");
        assert!(
            err.to_string().contains("3c 68 74 6d"),
            "user-facing error must quote the bytes: {err}"
        );
        assert!(
            check_archive_magic(b"PK\x03\x04", false, "https://x/y.zip").is_ok(),
            "zip magic accepted for zip installs"
        );
        assert!(
            check_archive_magic(&[], true, "https://x/y.tar.gz").is_err(),
            "an empty head cannot be validated"
        );
    }
}
