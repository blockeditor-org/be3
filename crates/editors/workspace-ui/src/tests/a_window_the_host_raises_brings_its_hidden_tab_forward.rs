use super::*;

fn shown_windows(fixture: &Fixture) -> Vec<u64> {
    placed_windows(fixture).iter().map(|(id, _)| *id).collect()
}

fn raised(id: u64, title: &str, times: u64) -> block_editor_beui::HostWindow {
    block_editor_beui::HostWindow {
        raised: times,
        ..window(id, title, None)
    }
}

#[test]
fn a_window_the_host_raises_brings_its_hidden_tab_forward() {
    let (mut fixture, _) = editor();
    fixture.host.set_watched::<WindowsWatch>(&vec![
        window(7, "Terminal", None),
        window(8, "Firefox", None),
    ]);
    fixture.settle();
    assert_eq!(
        shown_windows(&fixture),
        vec![8],
        "the window opened last is the tab on show"
    );

    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![raised(7, "Terminal", 1), raised(8, "Firefox", 0)]);
    fixture.settle();
    assert_eq!(
        shown_windows(&fixture),
        vec![7],
        "the window the host raises has its tab shown"
    );

    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![raised(7, "Terminal", 1), raised(8, "Firefox", 1)]);
    fixture.settle();
    assert_eq!(shown_windows(&fixture), vec![8]);

    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![raised(7, "~ - Terminal", 1), raised(8, "Firefox", 1)]);
    fixture.settle();
    assert_eq!(
        shown_windows(&fixture),
        vec![8],
        "a window raised before is not raised again by other news of it"
    );

    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![raised(7, "~ - Terminal", 2), raised(8, "Firefox", 1)]);
    fixture.settle();
    assert_eq!(
        shown_windows(&fixture),
        vec![7],
        "raising it again brings it forward again"
    );
}
