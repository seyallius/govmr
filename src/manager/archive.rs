//! Module archive - Archive format validation and extraction for downloaded toolchains.

use crate::errors::GovmError;
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

/// Validates that a downloaded payload starts with the expected archive magic.
///
/// gzip archives begin with `1f 8b`, zip archives with `50 4b` ("PK"). Anything
/// else — most commonly an HTML error or proxy page — is rejected *before*
/// extraction so users get an actionable error instead of "invalid gzip header".
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
    Err(GovmError::NotAnArchive {
        kind: if is_tar { "tar.gz" } else { "zip" }.to_string(),
        head: hex.join(" "),
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
    if is_tar {
        let tar_gz = File::open(download_path)?;
        let tar = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(tar);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let stripped: PathBuf = path.components().skip(1).collect();
            if !stripped.as_os_str().is_empty() {
                let out_path = target_dir.join(stripped);
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
        let zip_file = File::open(download_path)?;
        let mut archive =
            zip::ZipArchive::new(zip_file).map_err(|e| GovmError::Extraction(e.to_string()))?;
        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| GovmError::Extraction(e.to_string()))?;
            let outpath = match file.enclosed_name() {
                Some(path) => {
                    let stripped: PathBuf = path.components().skip(1).collect();
                    target_dir.join(stripped)
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
    let _ = fs::remove_file(download_path);
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
