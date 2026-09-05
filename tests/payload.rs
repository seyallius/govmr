//! Module payload. Ensures non-archive payloads (like the HTML page
//! that caused the "invalid gzip header" incident) are rejected before extraction.

// Tests exist to fail loudly: panicking on a broken setup or a failed helper
// is exactly the desired behavior, so unwraps are idiomatic here.
#![allow(clippy::unwrap_used)]

use govmr::errors::GovmError;
use govmr::manager::check_archive_magic;

#[test]
fn accepts_gzip_and_zip_magic() {
    assert!(check_archive_magic(&[0x1f, 0x8b, 0x08, 0x00], true, "u").is_ok());
    assert!(check_archive_magic(&[0x50, 0x4b, 0x03, 0x04], false, "u").is_ok());
}

#[test]
fn rejects_html_payload_like_the_incident() {
    // The exact bytes from the 2026-08-31 govmr.log forensics.
    let html = b"\n<!DOCTYPE html>\n<html>";
    let err = check_archive_magic(html, true, "https://go.dev/dl/x.tar.gz").unwrap_err();
    match err {
        GovmError::NotAnArchive { kind, head, url } => {
            assert_eq!(kind, "tar.gz");
            assert!(head.starts_with("0a 3c"), "hex head: {head}");
            assert!(url.contains("go.dev"));
        }
        other => panic!("wrong variant: {other}"),
    }
}

#[test]
fn rejects_truncated_or_empty_payload() {
    assert!(check_archive_magic(&[0x1f], true, "u").is_err());
    assert!(check_archive_magic(&[], false, "u").is_err());
}
