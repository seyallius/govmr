# GoVMR - Go Version Manager (Rust Edition)

A high-performance, asynchronous Go Version Manager written in Rust. GoVMR features a rich, interactive Terminal User
Interface (TUI) powered by Ratatui, as well as a full-featured Command Line Interface (CLI) for scripting and
automation.

## Features

- **Blazing Fast & Lightweight**: Zero external runtime dependencies; compiled directly to a standalone native binary.
- **Asynchronous Network & Streaming**: Concurrent downloads and in-memory archive extraction using Tokio, Reqwest,
  Flate2, and Tar/Zip.
- **Interactive TUI**: Built with Ratatui and Crossterm, featuring live installation feedback, status dashboards, and
  keyboard navigation.
- **Non-Destructive Version Switching**: Uses executable shims (`~/.govmr/shim`) to instantly switch active versions
  without modifying global binary paths.
- **Cross-Platform**: Full support for Linux, macOS, and Windows.

---

## Installation

### From `cargo-binstall` (Recommended)

```bash
# If you don't have binstall
cargo install cargo-binstall

cargo binstall govmr
```

### From `cargo`

```bash
cargo install govmr
```

### From Source

Ensure you have Rust and Cargo installed:

```bash
git clone [https://github.com/melkeydev/govmr.git](https://github.com/melkeydev/govmr.git)
cd govmr
cargo build --release
```

Move the compiled binary to your `PATH`:

```bash
# On Linux / macOS
sudo install -m 755 target/release/govmr /usr/local/bin/

# On Windows
copy target\release\govmr.exe C:\Windows\System32\
```

---

## Shell Configuration (First-Time Setup)

GoVMR uses executable shims located in `~/.govmr/shim`. Add this directory to your shell configuration so `go` resolves
through GoVMR.

### Linux & macOS

Add the following to your `~/.bashrc`, `~/.zshrc`, or equivalent shell profile:

```bash
export PATH="$HOME/.govmr/shim:$PATH"
```

Reload your environment:

```bash
source ~/.bashrc  # or ~/.zshrc
```

### Windows (PowerShell / Command Prompt)

Run the following command in Command Prompt:

```cmd
setx PATH "%USERPROFILE%\.govmr\shim;%PATH%"
```

Restart your terminal session to apply changes.

---

## Usage

### Interactive TUI

Launch the full interactive TUI by executing `govmr` without subcommands:

```bash
govmr
```

#### Keybindings

* `Up` / `k`: Move selection up.
* `Down` / `j`: Move selection down.
* `Tab`: Switch between **Available Versions** and **Installed Versions** views.
* `i`: Download and install the selected version.
* `u`: Switch active Go version to the selected release.
* `d`: Delete the selected installed version (prompts confirmation).
* `r`: Refresh remote version manifests from `go.dev`.
* `q` / `Ctrl+C`: Exit GoVMR.

### CLI Commands

```bash
# Install a specific or partial Go release
govmr install 1.22
govmr install 1.21.6

# Switch active Go version
govmr use 1.22

# List installed versions and show current active version
govmr list

# Delete an installed Go release
govmr delete 1.21.6

# Show help options
govmr --help
```

---

## Architecture Overview

* **`src/manager.rs`**: Core orchestrator managing remote version manifests, asynchronous streaming, decompression, and
  disk persistence.
* **`src/shim.rs`**: Manages shell wrappers inside `~/.govmr/shim` to forward execution to the currently active Go
  binary.
* **`src/tui/`**: Pure terminal UI rendering widgets, state transitions, color themes, and first-time setup flows via
  Ratatui.
* **`src/cli.rs`**: Subcommand definitions and execution pathways using Clap.

## License

[MIT OR Apache-2.0](./LICENSE)
