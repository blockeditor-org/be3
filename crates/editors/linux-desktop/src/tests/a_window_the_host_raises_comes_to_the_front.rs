use super::*;

fn topmost(fixture: &Fixture) -> Option<u64> {
    placed_windows(fixture).last().map(|(id, _)| *id)
}

fn raised(id: u64, title: &str, times: u64) -> HostWindow {
    HostWindow {
        raised: times,
        ..window(id, title, None)
    }
}

#[test]
fn a_window_the_host_raises_comes_to_the_front() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.set_host_value::<HostWindows>(&vec![
        window(7, "Terminal", None),
        window(8, "Browser", None),
    ]);
    fixture.settle();
    assert_eq!(
        topmost(&fixture),
        Some(8),
        "the window opened last is in front"
    );

    fixture
        .test
        .set_host_value::<HostWindows>(&vec![raised(7, "Terminal", 1), raised(8, "Browser", 0)]);
    fixture.settle();
    assert_eq!(
        topmost(&fixture),
        Some(7),
        "the window the host raises comes to the front"
    );

    fixture
        .test
        .set_host_value::<HostWindows>(&vec![raised(7, "Terminal", 1), raised(8, "Browser", 1)]);
    fixture.settle();
    assert_eq!(topmost(&fixture), Some(8));

    fixture.test.set_host_value::<HostWindows>(&vec![
        raised(7, "Terminal - ~", 1),
        raised(8, "Browser", 1),
    ]);
    fixture.settle();
    assert_eq!(
        topmost(&fixture),
        Some(8),
        "a window raised before is not raised again by other news of it"
    );

    fixture.test.set_host_value::<HostWindows>(&vec![
        raised(7, "Terminal - ~", 2),
        raised(8, "Browser", 1),
    ]);
    fixture.settle();
    assert_eq!(
        topmost(&fixture),
        Some(7),
        "raising it again brings it forward again"
    );
}
