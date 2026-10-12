use super::*;

fn frozen(id: u64, title: &str) -> HostWindow {
    HostWindow {
        responding: false,
        ..window(id, title, None)
    }
}

fn force_closed(fixture: &Fixture) -> bool {
    fixture
        .test
        .actions::<WindowAction>()
        .contains(&WindowAction::Close(HostWindowId(3)))
}

#[test]
fn a_window_that_stops_responding_offers_to_wait_or_force_close() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture
        .test
        .set_host_value::<HostWindows>(&vec![window(3, "Terminal", None)]);
    fixture.settle();
    let calm = fixture.test.occluders().len();

    fixture
        .test
        .set_host_value::<HostWindows>(&vec![frozen(3, "Terminal")]);
    fixture.settle();
    assert!(
        fixture.says("Terminal (not responding)"),
        "the tab says the window is not responding"
    );
    assert!(
        fixture.test.occluders().len() > calm,
        "a banner covers part of the window"
    );
    fixture
        .test
        .snapshot("a_window_that_stops_responding_offers_to_wait_or_force_close");

    fixture.test.click("workspace.window.wait");
    fixture.settle();
    assert_eq!(
        fixture.test.occluders().len(),
        calm,
        "waiting puts the banner away"
    );
    assert!(!force_closed(&fixture));

    fixture
        .test
        .set_host_value::<HostWindows>(&vec![window(3, "Terminal", None)]);
    fixture.settle();
    assert!(
        !fixture.says("Terminal (not responding)"),
        "an answer clears it"
    );
    fixture
        .test
        .set_host_value::<HostWindows>(&vec![frozen(3, "Terminal")]);
    fixture.settle();
    assert!(
        fixture.test.occluders().len() > calm,
        "freezing again brings the banner back"
    );

    fixture.test.click("workspace.window.force_close");
    fixture.settle();
    assert!(
        force_closed(&fixture),
        "force close asks the host to close it"
    );
}
