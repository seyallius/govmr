<div align="center">

# govmr

**Blazingly fast, asynchronous Go version manager with a rich Terminal UI.**

![GitHub Release Downloads](https://img.shields.io/github/downloads/seyallius/govmr/total?label=downloads&logo=github&color=pink&style=for-the-badge)
![Latest Release Downloads](https://img.shields.io/github/downloads/seyallius/govmr/latest/total?label=latest%20release&logo=github&style=for-the-badge)
[![Release](https://img.shields.io/github/v/release/seyallius/govmr?include_prereleases&style=for-the-badge)](https://github.com/seyallius/govmr/releases)
![Crates.io Downloads](https://img.shields.io/crates/d/govmr?label=cargo%20installs&logo=rust&color=orange&style=for-the-badge)
[![Rust](https://img.shields.io/badge/rust-1.93.0%2B-orange.svg?style=for-the-badge)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg?style=for-the-badge)](LICENSE)
![GitHub Stars](https://img.shields.io/github/stars/seyallius/govmr?style=social)

[Features](#features) • [Installation](#installation) • [Quick Setup](#path-setup) • [Usage](#terminal-user-interface-tui) • [Architecture](#how-it-works)

</div>

---

## Demo

<div align="center">
  <img src="https://github.com/seyallius/govmr/tree/main/docs/assets/demo_c.gif" alt="govmr interactive TUI demo" width="850px" />
  <p><em>Interactive dashboard powered by Ratatui & Tokio async runtime</em></p>

  <img src="https://github.com/seyallius/govmr/tree/main/docs/assets/themes_c.gif" alt="govmr TUI themes" width="850px" />
  <p><em>`govmr` TUI's persistent themes</em></p>
</div>

---

## Features

- **Non-Blocking Async Engine**: Powered by [Tokio](https://tokio.rs/)
  and [Ratatui](https://github.com/ratatui/ratatui). Fetching releases, streaming downloads, and decompressing archives
  happen entirely on background threads without locking the UI.
- **Cancellable Background Operations**: Abort in-flight downloads and archive extractions instantly by pressing `c` or
  `Esc` without leaving orphaned files behind.
- **Transparent Shim Dispatch**: Zero environment pollution. Switches toolchains in place via `~/.govmr/shim` without
  spawning subshells or requiring terminal reloads.
- **Dual Engine**: Use the interactive TUI for visual tracking, or invoke the script-friendly CLI for headless
  automation and CI/CD pipelines.
- **Intelligent Version Matching**: Automatically resolves prefixes (e.g., `1.22` resolves to the latest stable release
  of `1.22.x`).
- **Rich Customization**: 19 built-in color themes across Dark/Light families, browsable via a live-preview TUI picker.
- **Docked Operation Log**: IDE-style log panel (`L` to toggle) with live tailing, focus-based scrolling, and 1MiB
  rotation for post-mortem debugging.
- **Cross-Platform Parity**: First-class support for Linux, macOS, and Windows.

---

## Installation

### Install Script

#### Linux & macOS

```bash
curl -fsSL https://raw.githubusercontent.com/seyallius/govmr/v1.0.0/install.sh | bash
```

_To install a specific version:_

```bash
curl -fsSL https://raw.githubusercontent.com/seyallius/govmr/v1.0.0/install.sh | bash -s v1.0.0
```

#### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/seyallius/govmr/v1.0.0/install.ps1 | iex
```

_To install a specific version:_

```powershell
iex "& { $( irm https://raw.githubusercontent.com/seyallius/govmr/v1.0.0/install.ps1 ) } -Version v1.0.0"
```

### Cargo Binstall (Fastest if you have cargo installed)

If you have [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) installed, fetch and install pre-compiled
binaries directly:

```bash
cargo binstall govmr
```

### Cargo (Build from Source)

```bash
cargo install govmr --locked
```

### GitHub Releases

Download signed, pre-built binary archives for your architecture directly from
the [GitHub Releases](https://github.com/seyallius/govmr/releases) page and extract `govmr` into your system
`$PATH`.

---

## 🪄 Automatic Shell Completions

**Zero configuration required.** On its very first run, `govmr` automatically generates and installs shell completion
scripts for:

- **Bash** (`~/.local/share/bash-completion/completions/govmr`)
- **Zsh** (`~/.local/share/zsh/site-functions/_govmr`)
- **Fish** (`~/.config/fish/completions/govmr.fish`)
- **PowerShell** (`~/.config/powershell/govmr.ps1` on Unix, `~/Documents/PowerShell/govmr.ps1` on Windows)

### ✨ How It Works

1. **First run** → `govmr` detects your active shell via `$SHELL`
2. **Generates** → Writes the completion script to `~/.govmr/completions/`
3. **Symlinks** → Creates a symlink to your shell's standard discovery directory
4. **Updates intelligently** → Content-aware staleness detection rewrites the script only when the CLI changes (new
   commands, flags, themes)

### 🧠 Smart Features

- **Content-aware regeneration**: Not just timestamp-based — compares the _actual script content_ byte-for-byte. Add a
  new subcommand? The script gets updated automatically.
- **Single source of truth**: All completion scripts live in `~/.govmr/completions/` as canonical files, with symlinks
  pointing to shell discovery directories. No scattered files across your system.
- **Clean uninstall**: Running `govmr uninstall` removes **every** completion symlink and `~/.govmr/completions/` — no
  orphaned files left behind.

### 🔄 Reloading Completions

After `govmr` updates its completion script (e.g., after a self-update or adding new commands), **open a new terminal
window** or source the script manually:

```bash
# Bash
source ~/.local/share/bash-completion/completions/govmr

# Zsh
source ~/.local/share/zsh/site-functions/_govmr

# Fish
source ~/.config/fish/completions/govmr.fish

# PowerShell
. $HOME/Documents/PowerShell/govmr.ps1  # Windows
. ~/.config/powershell/govmr.ps1        # Unix
```

> 💡 **Tip**: Opening a new terminal tab/window is usually the easiest way to reload completions — your shell
> automatically scans its discovery directories on startup.

### 🧪 Test It!

```bash
# Command completions
govmr <TAB><TAB>
# → install  use  delete  list  theme  update  uninstall  help

# Argument completions (themes)
govmr theme <TAB><TAB>
# → gocyan  newisland  cursordark  midnight  tokyonight  mocha  ...
```

---

## PATH Setup

`govmr` uses a centralized shim directory (`~/.govmr/shim`). Add this directory to your `$PATH` once to allow shims
(`go`, `gofmt`) to intercept and dispatch calls to the active Go runtime.

### 🪄 The Easy Way (TUI Auto-Fix)

**You usually don't need to configure this manually.**

Just launch the TUI by running `govmr`. If the shim directory is missing from your PATH, an interactive setup overlay
will appear automatically. Simply press **`f`**, and GoVMR will safely and idempotently apply the correct PATH fix for
your OS:

- **Windows**: Updates the persistent User PATH via PowerShell in a hidden window.
- **Linux/macOS**: Appends the export line to your shell profile (`~/.zshrc`, `~/.bashrc`, or
  `~/.config/fish/config.fish`) with an idempotency guard.

### 🛠️ The Manual Way (Headless / CI)

If you are running headless or prefer manual configuration, use the commands below:

#### Linux / macOS

Add the shim directory to your shell configuration file (`~/.bashrc`, `~/.zshrc`, or `~/.config/fish/config.fish`):

```bash
# Bash / Zsh
export PATH="$HOME/.govmr/shim:$PATH"
```

```fish
# Fish
fish_add_path $HOME/.govmr/shim
```

Reload your current session:

```bash
source ~/.bashrc # or ~/.zshrc
```

#### Windows

**PowerShell (Recommended)**

Set the persistent User environment variable without truncating system strings:

```powershell
$shimDir = "$HOME\.govmr\shim"
$currentPath = [Environment]::GetEnvironmentVariable("Path", "User")

if ($currentPath -notlike "*$shimDir*")
{
    [Environment]::SetEnvironmentVariable("Path", "$shimDir;$currentPath", "User")
    $env:Path = "$shimDir;" + $env:Path
}
```

**Command Prompt (Legacy)**

> ⚠️ **Warning**: `setx` truncates the PATH variable to 1024 characters. If your PATH is long, this can silently corrupt
> it. We strongly recommend using the PowerShell method above or the TUI's `f` key.

```cmd
setx PATH "%USERPROFILE%\.govmr\shim;%PATH%"
```

---

## Terminal User Interface (TUI)

Launch the interactive interface with:

```bash
govmr
```

### Keybindings & Navigation

| Keybinding                            | Action                                                                                 |
|---------------------------------------|----------------------------------------------------------------------------------------|
| `Tab`                                 | Switch between **Available Versions** and **Installed Versions** tabs                  |
| `↑` / `k`, `↓` / `j`                  | Navigate through list rows                                                             |
| `i`                                   | Download and install the selected version                                              |
| `u`                                   | Activate/Switch to the selected version                                                |
| `d`                                   | Delete the selected version (prompts confirmation modal; cannot delete active version) |
| `c` / `Esc`                           | Cancel an ongoing background download or extraction                                    |
| `r`                                   | Refresh the remote version index from `go.dev`                                         |
| `T`                                   | Open the color theme picker                                                            |
| `L`                                   | Toggle the docked operation log panel                                                  |
| `f`                                   | Apply permanent PATH fix (when PATH setup overlay is active)                           |
| `q` / `Ctrl+C`                        | Gracefully quit `govmr`                                                                |
| `j/k`, `↑/↓`, `PgUp/PgDn` (help open) | Scroll the keyboard help panel                                                         |
| `h`                                   | PATH setup overlay — **only while the shim is missing**; inert once configured         |
| `?`                                   | Toggle the keyboard reference panel (dims the dashboard)                               |
| `U`                                   | Self-update govmr (confirmation prompt)                                                |
| `X`                                   | Self-uninstall govmr (confirmation prompt, then optional purge prompt)                 |

---

## CLI Documentation

`govmr` includes a complete non-interactive command-line interface for terminal workflows, scripts, and CI runners.

### Command Overview

```text
govmr [COMMAND]

Commands:
  install   Download and install a specified Go version
  use       Switch the active system Go version to an installed release
  delete    Remove an installed Go version from disk
  list      List all locally installed Go versions
  theme     View or change the TUI color theme
  help      Print this message or the help of the given subcommand(s)
```

---

### Subcommands

#### `govmr install <VERSION>`

Downloads, verifies, extracts, and automatically activates a Go toolchain.

```bash
# Install an exact patch version
govmr install 1.22.4

# Install the latest stable release for a minor release line
govmr install 1.23
```

---

#### `govmr use <VERSION>`

Updates the global shim pointer in `~/.govmr/shim` to the target installation.

```bash
# Switch to a specific installed version
govmr use 1.22.4

# Switch to the highest installed patch of a series
govmr use 1.22
```

_Notes:_

- If the requested version is not installed locally, `govmr` prints an actionable error asking to install it first.
- Re-pointing happens instantly via file shims; existing terminals execute the updated binary without restarting.

---

#### `govmr list`

Lists all locally installed toolchains and marks the active version.

```bash
govmr list
```

---

#### `govmr delete <VERSION>`

Removes an installed Go toolchain from `~/.govmr/versions`.

```bash
govmr delete 1.21.5
```

_Notes:_

- The CLI deletes immediately without a prompt. Use the TUI (`d` key) if you prefer interactive confirmation.
- **Safety check**: You cannot delete the currently active toolchain. Run `govmr use <OTHER_VERSION>` first.

---

#### `govmr theme [NAME]`

Lists available color themes or applies one permanently.

```bash
# List all 19 available themes
govmr theme

# Apply and save the Midnight theme
govmr theme midnight
```

---

#### `govmr update`

Checks GitHub for the latest release and automatically downloads/replaces the current binary.

```bash
govmr update
```

#### `govmr uninstall`

Removes the `govmr` executable from your system.

```bash
# Remove binary only (keeps ~/.govmr and installed Go versions)
govmr uninstall

# Remove binary AND purge ~/.govmr (deletes all installed Go versions, config, and logs)
govmr uninstall --purge
```

## How It Works

```text
                        ~/.govmr/shim/
[Terminal Invocation] ───────> [ go / gofmt ]
                                     │
                 ┌───────────────────┴───────────────────┐
                 ▼                                      ▼
       (Unix: POSIX wrapper)                     (Windows: go.bat)
       #!/usr/bin/env bash                       @echo off
       exec "/path/to/active/go" "$@"            "%USERPROFILE%\.govmr\versions\...\go.exe" %*
```

1. **Isolation**: Every Go toolchain resides in its own sandboxed directory under `~/.govmr/versions/go<VERSION>/`.
2. **Shim Multiplexing**: Rather than modifying shell variables like `GOROOT` or prepending different binary paths on
   every switch, `govmr` maintains standard shims (`go`, `gofmt`) inside `~/.govmr/shim/`.

- On **Linux and macOS**, shims are minimal shell wrappers delegating standard inputs and exit codes.
- On **Windows**, shims are generated as `go.bat` and `gofmt.bat` files forwarding `%*` arguments to the target binary.

3. **Download Cleanup**: Partial downloads are kept in `~/.govmr/downloads/` and verified with magic-byte sniffing.
   Failed or canceled transfers clean up temporary files automatically to prevent disk bloat.

---

## Development

```bash
# Clone the repository
git clone https://github.com/seyallius/govmr.git
cd govmr

# Run tests
cargo test

# Check documentation validity
cargo doc --no-deps --all-features

# Build optimized release binary
cargo build --release
```

---

## License

This project is licensed under the [MIT License](./LICENSE)
