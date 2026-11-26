use be_lib_ui_types::Modifiers;
use be_sdk_root_testing::{OPENS_STATISTICS, STATISTICS_LAYER, STATISTICS_SURFACE};
use be_wasm_root_api::StatisticRow;

use super::*;

fn row(name: &str, current: &str, average: &str, peak: &str) -> StatisticRow {
    StatisticRow {
        heading: false,
        name: name.to_owned(),
        current: current.to_owned(),
        average: average.to_owned(),
        peak: peak.to_owned(),
    }
}

fn heading(name: &str) -> StatisticRow {
    StatisticRow {
        heading: true,
        ..row(name, "", "", "")
    }
}

#[test]
fn a_root_shows_its_hosts_statistics_in_a_panel_watching_them_while_it_is_open() {
    let mut root = embedding();
    let mut time = 1;
    step(&mut root, time);
    assert!(
        !root.watching_statistics(),
        "with no panel open, the root does not watch the host's statistics"
    );

    let open = until(
        &mut root,
        &mut time,
        chord(OPENS_STATISTICS, Modifiers::NONE),
        "F3 opens the statistics panel, which watches the host's statistics",
        |done| layer_of(done, STATISTICS_LAYER).is_some(),
    );
    let panel = layer_of(&open, STATISTICS_LAYER).expect("the panel shows");
    assert!(!panel.input.focuses, "the panel never takes the focus");
    assert!(root.watching_statistics());

    // The host shows it what it measured, which the panel draws as rows.
    root.show_statistics(
        vec![
            row("Build", "release", "", ""),
            row("Full frame", "4.0 ms", "4.2 ms", "9.1 ms"),
            heading("Frame pacing"),
            row("Waiting for apps", "0.3 ms", "0.4 ms", "1.2 ms"),
        ],
        vec![
            heading("Memory"),
            row("Resident", "182 MB", "180 MB", "190 MB"),
        ],
        None,
    );
    for _ in 0..4 {
        time += 1;
        step(&mut root, time);
    }
    let painting = own_paintings(&mut root, time)
        .into_iter()
        .find(|(surface, _)| *surface == u64::from(STATISTICS_SURFACE))
        .map(|(_, painting)| painting)
        .expect("the root paints its statistics panel");
    assert!(
        glyphs(&painting.frame.primitives).len() > 60,
        "the panel draws its title and the rows the host showed it"
    );
    beui_paint_snapshot::assert_painting(
        "a_root_shows_its_hosts_statistics_in_a_panel_watching_them_while_it_is_open",
        painting,
    );

    let closed = until(
        &mut root,
        &mut time,
        chord(OPENS_STATISTICS, Modifiers::NONE),
        "F3 again closes the panel",
        |done| layer_of(done, STATISTICS_LAYER).is_none(),
    );
    assert!(layer_of(&closed, STATISTICS_LAYER).is_none());
    for _ in 0..2 {
        time += 1;
        step(&mut root, time);
    }
    assert!(
        !root.watching_statistics(),
        "once the panel closed, the root stops watching the host's statistics"
    );
}
