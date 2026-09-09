# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.0.0](https://github.com/seyallius/govmr/compare/v1.1.1...v2.0.0) - 2026-09-09

### Added

- *(logs)* add clear, separator, and blank-line controls

### Fixed

- *(update)* replace running binary atomically to avoid ETXTBSY

### Other

- *(readme)* correct gif url usage

## [1.1.1](https://github.com/seyallius/govmr/compare/v1.1.0...v1.1.1) - 2026-09-09

### Fixed

- *(logging)* align update verbs and extract version before clearing
- *(logging)* enforce single-error ownership and structured audit trail

## [1.1.0](https://github.com/seyallius/govmr/compare/v1.0.0...v1.1.0) - 2026-09-08

### Added

- *(completions/stale)* add staleness detection and symlink consolidation
- *(completions/detect)* detect active shell and generate only for that shell
- *(completions/generate)* auto-generate shell compl on first run

### Fixed

- *(uninstall)* improve reliability with self-deleting binary

### Other

- *(readme)* add automatic shell completions section
- *(infra)* add git graph pretty command
- *(cli)* use clap's ValueEnum derive for theme parsing

## [1.0.0](https://github.com/seyallius/govmr/compare/v0.4.0...v1.0.0) - 2026-09-06

### Added

- *(uninstall)* add binary-only removal option and clearer confirmation flow
- *(keys)* allow filter and system prompts even when busy
- *(update)* add status message feedback for self-update operations
- *(tui)* use Shift+U/X for update/uninstall and simplify help
- *(tui)* add dimmed overlay effect for modals and prompts
- *(tui)* add right-docked keyboard help panel with scrolling and maintenance shortcuts
- *(self-update)* add self-update/uninstall with TUI prompts and CLI commands

### Fixed

- *(clippy)* address clippy warnings and failing render tests
- *(tui)* increase system prompt modal height to 29 rows
- *(update)* gracefully handle 404 when no public releases exist
- *(build)* align binstall pkg-url template with release asset naming

### Other

- *(release)* bump version to 1.0.0 and refresh README assets
- *(readme)* [**breaking**] comprehensive rewrite with badges, demos, and updated content
- fix doc comment formatting and constant reference

## [0.4.0](https://github.com/seyallius/govmr/compare/v0.3.1...v0.4.0) - 2026-09-06

### Added

- *(theme)* add 11 new themes and group by dark/light
- *(theme)* two-level dark/light folder picker with live preview

### Fixed

- *(tui)* wipe stale glyphs under modals before painting theme background

### Other

- *(theme)* re-tune JetBrains New Island and Cursor Dark palettes
- *(clippy)* suppress too_many_lines lint and fix formatting nits

## [0.3.1](https://github.com/seyallius/govmr/compare/v0.3.0...v0.3.1) - 2026-09-05

### Fixed

- *(manager)* scope platform-specific imports

## [0.3.0](https://github.com/seyallius/govmr/compare/v0.2.0...v0.3.0) - 2026-09-05

### Fixed

- *(windows)* use named constant for CREATE_NO_WINDOW flag

### Other

- *(clippy)* address clippy warnings and improve code quality
- *(build)* add rust-version, binstall metadata, and clippy lints

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
