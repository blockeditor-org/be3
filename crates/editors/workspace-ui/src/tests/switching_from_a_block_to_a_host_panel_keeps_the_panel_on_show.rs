use block_editor_beui::HostPanel;

use super::*;

fn panel_shown(fixture: &Fixture) -> bool {
    fixture
        .test
        .children()
        .iter()
        .any(|placement| placement.content == ChildContent::Host(HostPanel::Performance))
}

#[test]
fn switching_from_a_block_to_a_host_panel_keeps_the_panel_on_show() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(390.0, 800.0)));
    show(&mut fixture, opened, None);
    fixture.host.show_panel(HostPanel::Performance);
    fixture.settle();
    assert!(panel_shown(&fixture), "the panel is shown");

    let panel = (1u64 << 40) + 1;
    fixture.test.click("dock.switch");
    fixture.settle();
    fixture.test.click(&format!("dock.switcher.tab.{}", 2));
    fixture.settle();
    assert_eq!(fixture.shown(), vec![opened], "the block is back on show");

    fixture.test.click("dock.switch");
    fixture.settle();
    fixture.test.click(&format!("dock.switcher.tab.{panel}"));
    fixture.settle();
    fixture.settle();
    fixture.settle();
    assert!(panel_shown(&fixture), "the panel stays on show");
    assert!(fixture.shown().is_empty(), "the block is not shown under it");
}
