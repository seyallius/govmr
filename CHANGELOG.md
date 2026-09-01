# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
