# Script: setup_govmr_github.ps1
# Description: Automates GitHub repository cleanup, milestones, labels, and issue creation
# for the GoVMR roadmap.

# ============================================
# GOVMR GITHUB MASTER SETUP SCRIPT (SAFE MODE)
# ============================================
$REPO = "seyallius/govmr"
$OWNER = "seyallius"
$PROJECT_NUM = 8 # Update this based on your `gh project list` output after Phase 3 runs!

Write-Host "🧹 Phase 1: Cleaning up existing data in $REPO..." -ForegroundColor Yellow

# Delete open issues (Explicitly targeted with -R)
gh issue list -R $REPO --state open --json number --jq '.[].number' | ForEach-Object {
    gh issue delete $_ -R $REPO --yes 2>$null
}

# Delete labels (Explicitly targeted with -R)
gh label list -R $REPO --json name --jq '.[].name' | ForEach-Object {
    gh label delete $_ -R $REPO --yes 2>$null
}

# Delete milestones (API calls are inherently safe as they include the repo in the URL)
gh api repos/$REPO/milestones --jq '.[].number' | ForEach-Object {
    gh api -X DELETE repos/$REPO/milestones/$_ 2>$null
}

# Delete project board (Projects are tied to the Owner, not the repo directly)
gh project delete $PROJECT_NUM --owner $OWNER --yes 2>$null

Write-Host "✅ Cleanup complete for $REPO!`n" -ForegroundColor Green

Write-Host "🏗️ Phase 2: Creating Labels & Milestones..." -ForegroundColor Cyan

# Create labels (Explicitly targeted with -R)
gh label create "priority:P0" -R $REPO --color "B60205" --description "v1.0.0 Launch" >$null
gh label create "priority:P1" -R $REPO --color "D93F0B" --description "v1.x Polish" >$null
gh label create "priority:P2" -R $REPO --color "FBCA04" --description "v2.0 Power User" >$null
gh label create "priority:P3" -R $REPO --color "0E8A16" --description "v2.1+ Enterprise" >$null

gh label create "type:enhancement" -R $REPO --color "A2EEEF" --description "Feature" >$null
gh label create "type:bug" -R $REPO --color "D73A4A" --description "Bug" >$null
gh label create "type:documentation" -R $REPO --color "0075CA" --description "Docs" >$null
gh label create "type:chore" -R $REPO --color "F9D0C4" --description "Maintenance" >$null
gh label create "type:refactor" -R $REPO --color "1D76DB" --description "Refactor" >$null
gh label create "status:blocked" -R $REPO --color "D93F0B" --description "Blocked" >$null
gh label create "status:needs-review" -R $REPO --color "FBCA04" --description "Review" >$null

# Create milestones via API (Safe)
gh api repos/$REPO/milestones -f title="v1.0.0 - Production Ready" -f description="README polish, docs, and official 1.0 launch" -f due_on="2026-10-15T23:59:59Z" >$null
gh api repos/$REPO/milestones -f title="v1.1.0 - Shell Completions" -f description="clap_complete integration" -f due_on="2026-10-31T23:59:59Z" >$null
gh api repos/$REPO/milestones -f title="v1.2.0 - Fuzzy Filtering" -f description="nucleo/fuzzy-matcher integration" -f due_on="2026-11-15T23:59:59Z" >$null
gh api repos/$REPO/milestones -f title="v1.3.0 - Keybindings Overlay" -f description="Global ? help modal" -f due_on="2026-11-30T23:59:59Z" >$null
gh api repos/$REPO/milestones -f title="v1.4.0 - Hide Unstable Toggle" -f description="Filter out rc/beta versions" -f due_on="2026-12-15T23:59:59Z" >$null
gh api repos/$REPO/milestones -f title="v2.0.0 - Power User Workflows" -f description="govmr exec and govmr doctor" -f due_on="2027-01-31T23:59:59Z" >$null
gh api repos/$REPO/milestones -f title="v2.1.0 - Enterprise & Insights" -f description="Local archives, cross-arch, disk stats" -f due_on="2027-03-31T23:59:59Z" >$null

Write-Host "🏗️ Phase 3: Creating Project Board..." -ForegroundColor Cyan
gh project create --title "GoVMR Development" --owner $OWNER >$null
Write-Host "⚠️  IMPORTANT: Run 'gh project list' now and update `$PROJECT_NUM at the top of this script before continuing!" -ForegroundColor Yellow
Read-Host "Press Enter once you have updated `$PROJECT_NUM"

Write-Host "🏗️ Phase 4: Creating Issues..." -ForegroundColor Cyan

# Helper function to avoid CLI escaping and here-string errors
function New-GovmrIssue {
    param([string]$Title, [string]$Body, [string]$Labels, [string]$Milestone)
    $temp = "temp_govmr_issue.md"
    $Body | Out-File -FilePath $temp -Encoding utf8

    # Explicitly targeted with -R
    $out = gh issue create -R $REPO --title $Title --body-file $temp --label $Labels --milestone $Milestone
    $url = $out | Where-Object { $_ -match "https://github.com" } | Select-Object -First 1

    if ($url) {
        Write-Host "✅ $Title" -ForegroundColor Green
        gh project item-add $PROJECT_NUM --owner $OWNER --url $url >$null
    } else {
        Write-Host "❌ $Title" -ForegroundColor Red; Write-Host $out
    }
    Remove-Item $temp -ErrorAction SilentlyContinue
}

# --- MILESTONE 1: v1.0.0 - Production Ready ---
$M1 = "v1.0.0 - Production Ready"
$L1 = "type:documentation,priority:P0"

$b1 = @"
## Description
Remove all WIP warnings from the README, ensure installation scripts are up to date, and finalize documentation for the official v1.0.0 launch.
## Tasks
- [ ] Remove '> WIP - Current version is still a prototype' from README
- [ ] Verify install.sh and install.ps1 scripts are pointing to correct release tags
- [ ] Ensure `cargo doc` builds cleanly without warnings
- [ ] Write v1.0.0 release notes highlighting the async TUI and shim management
## Acceptance Criteria
- [ ] README looks professional and production-ready
- [ ] v1.0.0 tag is created and published
"@
New-GovmrIssue "Polish README and prepare for v1.0.0 launch" $b1 $L1 $M1

# --- MILESTONE 2: v1.1.0 - Shell Completions ---
$M2 = "v1.1.0 - Shell Completions"
$L2 = "type:enhancement,priority:P1"

$b2 = @"
## Description
Integrate `clap_complete` to automatically generate and ship shell completion scripts for Bash, Zsh, Fish, and PowerShell.
## Tasks
- [ ] Add `clap_complete` to Cargo.toml
- [ ] Create a hidden `govmr completions <shell>` subcommand
- [ ] Update README with instructions on how to install completions for each shell
- [ ] Test completions locally for all 4 major shells
## Acceptance Criteria
- [ ] Typing `govmr ins<TAB>` auto-completes to `install`
- [ ] Typing `govmr theme <TAB>` lists available themes
"@
New-GovmrIssue "Implement shell completions via clap_complete" $b2 $L2 $M2

# --- MILESTONE 3: v1.2.0 - Fuzzy Filtering ---
$M3 = "v1.2.0 - Fuzzy Filtering"
$L3 = "type:enhancement,priority:P1"

$b3 = @"
## Description
Upgrade the TUI `/` search from strict `.contains()` matching to a proper fuzzy matcher (e.g., `nucleo` or `fuzzy-matcher`) so users can type `122` and match `1.22.0`.
## Tasks
- [ ] Evaluate and add `nucleo` or `fuzzy-matcher` crate
- [ ] Replace `.contains()` logic in `visible_indices()` with fuzzy scoring
- [ ] Sort filtered results by fuzzy match score (best matches first)
- [ ] Ensure filter mode UI remains responsive during typing
## Acceptance Criteria
- [ ] Typing `121` highlights `1.21.6`
- [ ] Typing `rc1` highlights `1.24rc1`
"@
New-GovmrIssue "Upgrade TUI search to fuzzy filtering" $b3 $L3 $M3

# --- MILESTONE 4: v1.3.0 - Keybindings Overlay ---
$M4 = "v1.3.0 - Keybindings Overlay"
$L4 = "type:enhancement,priority:P1"

$b4 = @"
## Description
Add a `?` key to open a centered, comprehensive cheat sheet overlay in the TUI, separate from the PATH setup guide.
## Tasks
- [ ] Create a new `render_keybindings_modal` function in `views.rs`
- [ ] Map `?` to toggle the new overlay state
- [ ] Design a clean, multi-column layout for all shortcuts
- [ ] Ensure `q` and `Esc` close the modal cleanly
## Acceptance Criteria
- [ ] Pressing `?` opens the cheat sheet from any tab
- [ ] Modal is fully theme-aware and centered
"@
New-GovmrIssue "Add global keybindings overlay (?)" $b4 $L4 $M4

# --- MILESTONE 5: v1.4.0 - Hide Unstable Toggle ---
$M5 = "v1.4.0 - Hide Unstable Toggle"
$L5 = "type:enhancement,priority:P1"

$b5 = @"
## Description
Add a quick keybind (e.g., `H` or `U`) to toggle the visibility of `rc` and `beta` versions in the Available tab.
## Tasks
- [ ] Add `hide_unstable: bool` to `AppState`
- [ ] Update `visible_indices()` to filter out non-stable releases when toggled
- [ ] Add visual indicator in the footer/status bar when hidden
- [ ] Persist preference in `config.toml`
## Acceptance Criteria
- [ ] Pressing the toggle key instantly hides/shows RCs and Betas
- [ ] State survives app restarts
"@
New-GovmrIssue "Add Hide Unstable toggle for release candidates" $b5 $L5 $M5

# --- MILESTONE 6: v2.0.0 - Power User Workflows ---
$M6 = "v2.0.0 - Power User Workflows"
$L6 = "type:enhancement,priority:P2"

$b6 = @"
## Description
Implement `govmr exec <version> -- <command>` to run commands against a specific Go version without globally switching the active shim.
## Tasks
- [ ] Add `Exec` subcommand to `cli.rs`
- [ ] Resolve target version and verify installation
- [ ] Prepend version's `bin` directory to `PATH` environment variable
- [ ] Spawn child process and wait for completion
## Acceptance Criteria
- [ ] `govmr exec 1.21 -- go test ./...` runs tests using Go 1.21
- [ ] Global active version remains unchanged after execution
"@
New-GovmrIssue "Implement govmr exec for temporary version execution" $b6 $L6 $M6

$b7 = @"
## Description
Implement `govmr doctor` diagnostic health-check command to verify environment integrity.
## Tasks
- [ ] Add `Doctor` subcommand to `cli.rs`
- [ ] Check if `~/.govmr/shim` is in system `PATH`
- [ ] Verify `active_version` file points to a valid directory
- [ ] Test network connectivity to `go.dev`
- [ ] Output clear ✅/❌ status for each check
## Acceptance Criteria
- [ ] `govmr doctor` provides actionable feedback for broken setups
"@
New-GovmrIssue "Implement govmr doctor diagnostic command" $b7 $L6 $M6

# --- MILESTONE 7: v2.1.0 - Enterprise & Insights ---
$M7 = "v2.1.0 - Enterprise & Insights"
$L7 = "type:enhancement,priority:P3"

$b8 = @"
## Description
Support installing Go from local `.tar.gz` or `.zip` archives for air-gapped networks or custom forks.
## Tasks
- [ ] Detect if `version` argument is a local file path
- [ ] Bypass network download and proceed straight to extraction
- [ ] Validate archive magic bytes before extraction
- [ ] Update CLI help text
## Acceptance Criteria
- [ ] `govmr install ./my-custom-go.tar.gz` works seamlessly
"@
New-GovmrIssue "Support local archive installation" $b8 $L7 $M7

$b9 = @"
## Description
Allow forcing specific architecture downloads (e.g., `--arch arm64` on an `amd64` host) for Docker prep and cross-compilation.
## Tasks
- [ ] Add `--arch` and `--os` flags to `Install` subcommand
- [ ] Override `env::consts::ARCH` and `OS` in `fetch_versions()`
- [ ] Validate requested arch/os combination against manifest
## Acceptance Criteria
- [ ] `govmr install 1.22 --arch arm64` downloads the ARM64 binary
"@
New-GovmrIssue "Support cross-architecture downloads" $b9 $L7 $M7

$b10 = @"
## Description
Show total disk footprint of `~/.govmr/versions` in the TUI footer and via a `govmr df` CLI command.
## Tasks
- [ ] Implement recursive directory size calculation for `versions_dir`
- [ ] Add `govmr df` subcommand
- [ ] Display total size in TUI status bar or footer
- [ ] Cache size calculation to avoid blocking the render loop
## Acceptance Criteria
- [ ] `govmr df` outputs exact MB/GB used by toolchains
- [ ] TUI shows disk usage non-intrusively
"@
New-GovmrIssue "Implement disk usage stats (govmr df)" $b10 $L7 $M7

Write-Host "`n=========================================" -ForegroundColor Cyan
Write-Host "🎉 ALL ISSUES CREATED AND ADDED TO PROJECT!" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan
