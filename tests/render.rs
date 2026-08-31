//! Headless smoke tests: render every TUI state into an in-memory buffer and
//! assert that drawing never panics and key strings appear where expected.

use govmr::app::{ActiveTab, AppState, BusyState, Phase};
use govmr::models::GoVersion;
use govmr::tui::views::{render, render_overlays};
use ratatui::Terminal;
use ratatui::{backend::TestBackend, buffer::Buffer};

fn make_terminal() -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(100, 30)).unwrap()
}

fn versions_fixture() -> Vec<GoVersion> {
    vec![
        GoVersion {
            raw_version: "1.22.0".into(),
            display_name: "go1.22.0".into(),
            filename: "go1.22.0.tar.gz".into(),
            url: "https://example.com/go1.22.0.tar.gz".into(),
            size: 68_000_000,
            installed: false,
            active: false,
            path: None,
            stable: true,
        },
        GoVersion {
            raw_version: "1.24rc1".into(),
            display_name: "go1.24rc1".into(),
            filename: "go1.24rc1.tar.gz".into(),
            url: "https://example.com/go1.24rc1.tar.gz".into(),
            size: 72_000_000,
            installed: true,
            active: true,
            path: Some(std::path::PathBuf::from("/home/tester/.govmr/versions/go1.24rc1")),
            stable: false,
        },
        GoVersion {
            raw_version: "1.21.6".into(),
            display_name: "go1.21.6".into(),
            filename: "go1.21.6.tar.gz".into(),
            url: "https://example.com/go1.21.6.tar.gz".into(),
            size: 65_000_000,
            installed: true,
            active: false,
            path: Some(std::path::PathBuf::from("/home/tester/.govmr/versions/go1.21.6")),
            stable: true,
        },
    ]
}

#[test]
fn renders_available_tab() {
    let mut terminal = make_terminal();
    let mut state = AppState::from_versions(versions_fixture(), true);
    terminal.draw(|f| { render(f, &mut state); render_overlays(f, &state); }).unwrap();

    let text = buffer_as_text(terminal.backend().buffer());
    assert!(text.contains("GoVMR"), "brand title should render");
    assert!(text.contains("Available"), "available tab should render");
    assert!(text.contains("Installed"), "installed tab should render");
    assert!(text.contains("go1.22.0"), "version rows should render");
    assert!(text.contains("active"), "active badge should render");
}

#[test]
fn renders_installed_tab_with_paths() {
    let mut terminal = make_terminal();
    let mut state = AppState::from_versions(versions_fixture(), true);
    state.active_tab = ActiveTab::Installed;
    state.list_state.select(Some(0));
    terminal.draw(|f| { render(f, &mut state); render_overlays(f, &state); }).unwrap();

    let text = buffer_as_text(terminal.backend().buffer());
    assert!(text.contains("go1.24rc1"), "installed rows should render");
    assert!(text.contains(".govmr"), "install path should render");
}

#[test]
fn renders_install_download_modal_with_gauge() {
    let mut terminal = make_terminal();
    let mut state = AppState::from_versions(versions_fixture(), true);
    state.busy = Some(BusyState::Installing {
        version: "1.22.0".into(),
        phase: Phase::Downloading,
        downloaded: 34_000_000,
        total: 68_000_000,
        speed: 5_000_000.0,
        started_at: std::time::Instant::now(),
    });
    terminal.draw(|f| { render(f, &mut state); render_overlays(f, &state); }).unwrap();

    let text = buffer_as_text(terminal.backend().buffer());
    assert!(text.contains("Installing Go 1.22.0"), "modal title");
    assert!(text.contains("50.0%"), "gauge percentage should show");
}

#[test]
fn renders_extraction_phase_modal() {
    let mut terminal = make_terminal();
    let mut state = AppState::from_versions(versions_fixture(), true);
    state.busy = Some(BusyState::Installing {
        version: "1.22.0".into(),
        phase: Phase::Extracting,
        downloaded: 68_000_000,
        total: 68_000_000,
        speed: 0.0,
        started_at: std::time::Instant::now(),
    });
    terminal.draw(|f| { render(f, &mut state); render_overlays(f, &state); }).unwrap();

    let text = buffer_as_text(terminal.backend().buffer());
    assert!(text.contains("extracting archive"), "extraction phase");
}

#[test]
fn renders_delete_confirmation_modal() {
    let mut terminal = make_terminal();
    let mut state = AppState::from_versions(versions_fixture(), true);
    state.confirming_delete = Some("1.21.6".into());
    terminal.draw(|f| { render(f, &mut state); render_overlays(f, &state); }).unwrap();

    let text = buffer_as_text(terminal.backend().buffer());
    assert!(text.contains("Deletion"), "delete modal title");
    assert!(text.contains("1.21.6"), "delete target shown");
}

#[test]
fn renders_filter_mode_and_filters_rows() {
    let mut terminal = make_terminal();
    let mut state = AppState::from_versions(versions_fixture(), true);
    state.filter_mode = true;
    state.filter = "1.22".into();
    state.list_state.select(Some(0));
    terminal.draw(|f| { render(f, &mut state); render_overlays(f, &state); }).unwrap();

    let text = buffer_as_text(terminal.backend().buffer());
    assert!(text.contains("Filter"), "filter prompt shown");
    assert!(text.contains("go1.22.0"), "matching row shown");
}

#[test]
fn renders_path_warning_when_shim_missing() {
    let mut terminal = make_terminal();
    let mut state = AppState::from_versions(versions_fixture(), false);
    terminal.draw(|f| { render(f, &mut state); render_overlays(f, &state); }).unwrap();

    let text = buffer_as_text(terminal.backend().buffer());
    assert!(text.contains("PATH"), "path warning banner shown");
}

#[test]
fn selection_navigation_wraps_within_visible_list() {
    let mut state = AppState::from_versions(versions_fixture(), true);
    state.next_item();
    state.next_item();
    assert!(state.list_state.selected() == Some(2));
    state.next_item();
    assert!(state.list_state.selected() == Some(0), "wraps to top");
    state.previous_item();
    assert!(state.list_state.selected() == Some(2), "wraps to bottom");
}

/// Flattens a ratatui test buffer into a plain string for substring assertions.
fn buffer_as_text(buf: &Buffer) -> String {
    let area = buf.area();
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}
