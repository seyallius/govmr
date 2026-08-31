//! Renders key TUI screens to plain-text + ANSI snapshots for visual inspection.
use govmr::app::{ActiveTab, AppState, BusyState, Phase};
use govmr::models::GoVersion;
use govmr::tui::views::{render, render_overlays};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn fixtures() -> Vec<GoVersion> {
    vec![
        GoVersion {
            raw_version: "1.23.0".into(),
            display_name: "go1.23.0".into(),
            filename: "go1.23.0.tar.gz".into(),
            url: String::new(),
            size: 75_300_000,
            installed: false,
            active: false,
            path: None,
            stable: true,
        },
        GoVersion {
            raw_version: "1.24rc1".into(),
            display_name: "go1.24rc1".into(),
            filename: "go1.24rc1.tar.gz".into(),
            url: String::new(),
            size: 74_900_000,
            installed: true,
            active: true,
            path: Some("/home/ali/.govmr/versions/go1.24rc1".into()),
            stable: false,
        },
        GoVersion {
            raw_version: "1.22.6".into(),
            display_name: "go1.22.6".into(),
            filename: "go1.22.6.tar.gz".into(),
            url: String::new(),
            size: 67_100_000,
            installed: true,
            active: false,
            path: Some("/home/ali/.govmr/versions/go1.22.6".into()),
            stable: true,
        },
        GoVersion {
            raw_version: "1.21.13".into(),
            display_name: "go1.21.13".into(),
            filename: "go1.21.13.tar.gz".into(),
            url: String::new(),
            size: 66_800_000,
            installed: false,
            active: false,
            path: None,
            stable: true,
        },
    ]
}

fn dump<F: FnOnce(&mut AppState)>(name: &str, configure: F) {
    let mut terminal = Terminal::new(TestBackend::new(92, 24)).unwrap();
    let mut state = AppState::from_versions(fixtures(), false);
    configure(&mut state);
    terminal.draw(|f| { render(f, &mut state); render_overlays(f, &state); }).unwrap();
    let buffer = terminal.backend().buffer().clone();
    let area = buffer.area();

    let mut out = String::new();
    out.push_str(&format!("\n===== {} =====\n", name));
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &buffer[(x, y)];
            out.push_str(cell.symbol());
        }
        out.push('\n');
    }
    print!("{}", out);
}

fn main() {
    dump("AVAILABLE TAB (with PATH warning)", |_| {});
    dump("INSTALLED TAB", |s| {
        s.active_tab = ActiveTab::Installed;
        s.list_state.select(Some(0));
    });
    dump("FILTER MODE  (filter = '1.22')", |s| {
        s.filter_mode = true;
        s.filter = "1.22".into();
        s.list_state.select(Some(0));
    });
    dump("DOWNLOADING MODAL (62%)", |s| {
        s.busy = Some(BusyState::Installing {
            version: "1.23.0".into(),
            phase: Phase::Downloading,
            downloaded: 46_700_000,
            total: 75_300_000,
            speed: 8_200_000.0,
            started_at: std::time::Instant::now(),
        });
    });
    dump("EXTRACTING MODAL", |s| {
        s.busy = Some(BusyState::Installing {
            version: "1.23.0".into(),
            phase: Phase::Extracting,
            downloaded: 75_300_000,
            total: 75_300_000,
            speed: 0.0,
            started_at: std::time::Instant::now(),
        });
    });
    dump("DELETE CONFIRMATION", |s| {
        s.confirming_delete = Some("1.22.6".into());
    });
    dump("HELP / SETUP OVERLAY", |s| {
        s.show_help = true;
        s.shim_path = "/home/ali/.govmr/shim".into();
    });
}
