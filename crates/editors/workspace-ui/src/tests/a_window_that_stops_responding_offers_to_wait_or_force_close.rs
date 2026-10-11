use super::*;

fn frozen(id: u64, title: &str) -> block_editor_beui::HostWindow {
    block_editor_beui::HostWindow {
        responding: false,
        ..window(id, title, None)
    }
}

fn force_closed(fixture: &Fixture) -> bool {
    fixture
        .test
        .actions::<WindowAction>()
        .contains(&WindowAction::Close(block_editor_beui::HostWindowId(3)))
}

#[test]
fn a_window_that_stops_responding_offers_to_wait_or_force_close() {
    let (mut fixture, _) = editor();
    fixture
        .host
        .set_host_value::<HostWindows>(&vec![window(3, "Terminal", None)]);
    fixture.settle();
    let resting = fixture.test.occluders().len();
    assert!(
        fixture
            .test
            .occluders()
            .iter()
            .all(|occluder| occluder.rect.width.min(occluder.rect.height) <= 16.0),
        "a responding window is left alone but for the split handle along its edge"
    );

    fixture
        .host
        .set_host_value::<HostWindows>(&vec![frozen(3, "Terminal")]);
    fixture.settle();
    assert!(
        fixture.says("Terminal (not responding)"),
        "the tab says the window is not responding"
    );
    assert!(
        fixture.test.occluders().len() > resting,
        "a banner covers part of the window"
    );
    fixture
        .test
        .snapshot("a_window_that_stops_responding_offers_to_wait_or_force_close");

    fixture.test.click("workspace.window.wait");
    fixture.settle();
    assert!(
        fixture.test.occluders().len() == resting,
        "waiting puts the banner away"
    );
    assert!(!force_closed(&fixture));

    fixture
        .host
        .set_host_value::<HostWindows>(&vec![window(3, "Terminal", None)]);
    fixture.settle();
    assert!(
        !fixture.says("Terminal (not responding)"),
        "an answer clears it"
    );
    fixture
        .host
        .set_host_value::<HostWindows>(&vec![frozen(3, "Terminal")]);
    fixture.settle();
    assert!(
        fixture.test.occluders().len() > resting,
        "freezing again brings the banner back"
    );

    fixture.test.click("workspace.window.force_close");
    fixture.settle();
    assert!(
        force_closed(&fixture),
        "force close asks the host to close it"
    );
}
