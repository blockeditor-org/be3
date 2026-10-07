use block_editor_beui::HostPanel;

use super::*;

#[test]
fn a_host_panel_the_host_asks_for_is_placed_in_its_own_window() {
    let (mut fixture, _) = editor();

    fixture.host.show_panel(HostPanel::Performance);
    fixture.settle();

    assert!(
        fixture
            .test
            .children()
            .iter()
            .any(|placement| placement.content == ChildContent::Host(HostPanel::Performance)),
        "the panel is placed for the host to draw"
    );
    assert!(fixture.says("Performance"), "the panel's window is titled");
    assert!(
        fixture.says("No file open"),
        "a panel's window leaves the empty pane beside the files"
    );
    let opened = Uuid::new_v4();
    show(&mut fixture, opened, None);
    assert_eq!(fixture.shown(), vec![opened]);
    assert!(
        fixture
            .test
            .children()
            .iter()
            .any(|placement| placement.content == ChildContent::Host(HostPanel::Performance)),
        "a block opened while the panel is focused gets a pane of its own, not the panel's window"
    );

    let tab = (1u64 << 40) + 1;
    fixture.test.click(&format!("dock.tab.{tab}.close"));
    fixture.settle();

    assert!(
        fixture
            .test
            .children()
            .iter()
            .all(|placement| !matches!(placement.content, ChildContent::Host(_))),
        "closing the window stops placing the panel"
    );
}
