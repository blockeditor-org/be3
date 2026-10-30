use super::*;

fn splitter(rect: Rect) -> bool {
    rect.width().min(rect.height()) < 10.0
}

fn frozen(id: u64, title: &str) -> block_editor_beui::HostWindow {
    block_editor_beui::HostWindow {
        responding: false,
        ..window(id, title, None)
    }
}

fn force_closed(fixture: &Fixture) -> bool {
    fixture
        .test
        .host_calls(block_ui_test::requested::window)
        .contains(&WindowAction::Close(block_editor_beui::HostWindowId(3)))
}

#[test]
fn a_window_that_stops_responding_offers_to_wait_or_force_close() {
    let (mut fixture, _) = editor();
    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![window(3, "Terminal", None)]);
    fixture.settle();
    assert!(
        fixture
            .test
            .occluders()
            .iter()
            .all(|occluder| splitter(occluder.rect)),
        "a responding window is left alone"
    );

    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![frozen(3, "Terminal")]);
    fixture.settle();
    assert!(
        fixture.says("Terminal (not responding)"),
        "the tab says the window is not responding"
    );
    assert!(
        !fixture
            .test
            .occluders()
            .iter()
            .all(|occluder| splitter(occluder.rect)),
        "a banner covers part of the window"
    );
    fixture
        .test
        .snapshot("a_window_that_stops_responding_offers_to_wait_or_force_close");

    fixture.test.click("workspace.window.wait");
    fixture.settle();
    assert!(
        fixture.test.occluders().iter().all(|occluder| splitter(occluder.rect)),
        "waiting puts the banner away"
    );
    assert!(!force_closed(&fixture));

    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![window(3, "Terminal", None)]);
    fixture.settle();
    assert!(
        !fixture.says("Terminal (not responding)"),
        "an answer clears it"
    );
    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![frozen(3, "Terminal")]);
    fixture.settle();
    assert!(
        !fixture.test.occluders().iter().all(|occluder| splitter(occluder.rect)),
        "freezing again brings the banner back"
    );

    fixture.test.click("workspace.window.force_close");
    fixture.settle();
    assert!(
        force_closed(&fixture),
        "force close asks the host to close it"
    );
}
