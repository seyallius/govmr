//! Module update - Replaces the currently-executing `govmr` binary with a freshly
//! downloaded one.
//!
//! On Linux the running executable's text segment is write-locked by the kernel
//! (`ETXTBSY`, errno 26), so the new binary is staged as a *sibling* file in the
//! same directory and then `rename(2)`d over the old one: `rename` swaps the
//! directory entry, never the mapped inode, and is atomic. Staging the sibling in
//! the *target* directory (not `/tmp`) also dodges `EXDEV`, since `rename` cannot
//! cross filesystem boundaries.

use crate::{errors::GovmError, logging};
use std::{fs, path::Path};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

// ----------------------------------------- Public API ----------------------------------------- //

/// Atomically replaces the currently-running executable with `new_bin`.
///
/// `new_bin` is the fully-extracted candidate (e.g. `/tmp/govmr_update/govmr`).
/// Returns the path that was replaced, for the completion log line.
///
/// # Errors
/// Returns [`GovmError::Io`] if the current executable path cannot be resolved,
/// the sibling cannot be written, or the final rename fails.
pub fn replace_current_binary(new_bin: &Path) -> Result<std::path::PathBuf, GovmError> {
    let current_exe = std::env::current_exe()?;
    replace_executable_platform(new_bin, &current_exe)
}

// -------------------------------------- Internal Helpers -------------------------------------- //

/// Unix: copy to a same-directory sibling, then `rename` over the running binary.
#[cfg(unix)]
fn replace_executable_platform(
    new_bin: &Path,
    current_exe: &Path,
) -> Result<std::path::PathBuf, GovmError> {
    // A sibling in the SAME directory guarantees rename() stays on one filesystem
    // (no EXDEV) and writes a brand-new inode (no ETXTBSY).
    let parent = current_exe
        .parent()
        .ok_or_else(|| GovmError::Extraction("current exe has no parent dir".to_string()))?;
    let sibling = parent.join(format!(
        ".{}.new",
        current_exe
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("govmr")
    ));

    // 1) Fresh file: copying into a not-yet-existing path never touches the
    //    running text segment, so the kernel has nothing to lock.
    fs::copy(new_bin, &sibling)?;

    // 2) Extraction can drop the exec bit; restore it BEFORE the rename so the
    //    binary is runnable the instant it takes the well-known path.
    fs::set_permissions(&sibling, fs::Permissions::from_mode(0o755))?;

    // 3) Atomic swap: rename replaces the directory entry. The old inode stays
    //    mapped by *this* running process until it exits; new launches get the
    //    new binary. This is the step that used to fail with ETXTBSY.
    fs::rename(&sibling, current_exe).map_err(|e| {
        // Never leave a stray `.govmr.new` behind on failure.
        let _ = fs::remove_file(&sibling);
        GovmError::Io(e)
    })?;

    logging::debug(&format!(
        "update: replaced via rename sibling=\"{}\" exe=\"{}\"",
        sibling.display(),
        current_exe.display()
    ));
    Ok(current_exe.to_path_buf())
}

/// Windows: a running exe cannot be renamed over either (sharing violation), but
/// it CAN be renamed *away*. Move the live binary aside, drop the new one in its
/// place, and schedule the stale copy for cleanup.
#[cfg(windows)]
fn replace_executable_platform(
    new_bin: &Path,
    current_exe: &Path,
) -> Result<std::path::PathBuf, GovmError> {
    let stale = current_exe.with_extension("old.exe");
    let _ = fs::remove_file(&stale); // drop a leftover from a previous update

    // Renaming a running exe is permitted on Windows; writing over it is not.
    fs::rename(current_exe, &stale)?;

    // Now the well-known path is free; move the new binary into it.
    if let Err(e) = fs::copy(new_bin, current_exe) {
        // Roll back so we never leave the user with no binary at all.
        let _ = fs::rename(&stale, current_exe);
        return Err(GovmError::Io(e));
    }

    // The old binary is still locked by this process; it can only be deleted
    // after exit. Best-effort: the next update's `remove_file(&stale)` above
    // cleans it up, or the user removes it manually.
    logging::debug(&format!(
        "update: replaced via rename-aside stale=\"{}\" exe=\"{}\"",
        stale.display(),
        current_exe.display()
    ));
    Ok(current_exe.to_path_buf())
}
