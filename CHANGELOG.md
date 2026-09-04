# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/seyallius/govmr/compare/v0.1.7...v0.2.0) - 2026-09-04

### Added

- *(focus)* implement high-contrast active tab with pill-style background
- *(install)* add graceful cancellation for ongoing downloads and extractions
- *(log)* dock IDE-style log panel with focus routing and word wrap
- *(log)* add live operation log viewer overlay
- *(install)* auto-activate Go version immediately after installation
- *(path)* show result notice inside help overlay for permanent PATH fix
- *(path)* add one-key permanent PATH fix in help overlay

### Other

- *(readme)* use absolute GitHub URLs for demo GIFs
- *(imports)* reorder and clean up import statements across codebase
- reorganize codebase into modular structure and clean up metadata
- *(cargo)* add repository metadata and clean up doc comments

## [0.1.7](https://github.com/seyallius/govmr/compare/v0.1.6...v0.1.7) - 2026-09-03

### Fixed

- *(tui)* replace jarring extraction gauge with smooth breathing pulse
- *(setup)* provide safe Windows PATH command with no truncation

## [0.1.6](https://github.com/seyallius/govmr/compare/v0.1.5...v0.1.6) - 2026-09-03

### Fixed

- *(setup)* use PowerShell $env:PATH command instead of setx on Windows
- *(shim)* use file_stem for Windows shim naming and clean up stale

### Other

- *(tui)* remove unnecessary network re-fetch after switching versions
- *(repo)* add GitHub setup script for milestones, labels, and issues

## [0.1.5](https://github.com/seyallius/govmr/compare/v0.1.4...v0.1.5) - 2026-09-02

### Added

- *(tui)* allow any key to dismiss setup guide and help overlay,
- *(tui)* show loading spinner during initial version fetch
- *(tui)* implement non-blocking background version fetching
- *(install)* add one-line installation scripts and README badges

### Other

- *(readme)* add demo GIFs showcasing TUI features

## [0.1.4](https://github.com/seyallius/govmr/compare/v0.1.3...v0.1.4) - 2026-09-01

### Added

- *(logging)* append operation log with 1 MiB rotation
- *(theme)* add eight themes with live picker and background fill
- *(config)* add TOML config with legacy migration
- *(theme)* add persistent, selectable color themes for the TUI

### Fixed

- *(manager)* reject non-2xx responses and non-archive payloads
- *(app)* own Refreshing lifecycle in refresh_versions to prevent stuck spinner

## [0.1.3](https://github.com/seyallius/govmr/compare/v0.1.2...v0.1.3) - 2026-09-01

### Added

- *(theme)* add eight themes with live picker and background fill
- *(config)* add TOML config with legacy migration
- *(theme)* add persistent, selectable color themes for the TUI

### Fixed

- *(resolve)* implement semver‑aware version prefix matching

### Other

- *(app)* reorganize Action enum and visible_indices

## [0.1.2](https://github.com/seyallius/govmr/compare/v0.1.1...v0.1.2) - 2026-08-31

### Added

- *(tui)* add live progress modal, filtering, and help overlay
- *(govm)* initial implementation of Go version manager

### Fixed

- *(name)* rename project to govmr and finalize initial implementation

### Other

- *(release)* add git-cliff and release-plz configuration
- *(rlz-plz)* use release_created output for asset jobs and add fail_on_failure
- *(release)* run release job even if build partially fails
- release v0.1.1
- *(release)* add GitHub Actions workflows for automated releases
- Initial commit

## [0.1.1](https://github.com/seyallius/govmr/compare/v0.1.0...v0.1.1) - 2026-08-31

### Added

- *(govm)* initial implementation of Go version manager

### Fixed

- *(name)* rename project to govmr and finalize initial implementation

### Other

- *(release)* add GitHub Actions workflows for automated releases
- Initial commit
