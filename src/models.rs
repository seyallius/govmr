use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseFile {
    pub filename: String,
    pub os: String,
    pub arch: String,
    pub size: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoRelease {
    pub version: String,
    pub stable: bool,
    pub files: Vec<ReleaseFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoVersion {
    pub raw_version: String,
    pub display_name: String,
    pub filename: String,
    pub url: String,
    pub installed: bool,
    pub active: bool,
    pub path: Option<PathBuf>,
    pub stable: bool,
}
