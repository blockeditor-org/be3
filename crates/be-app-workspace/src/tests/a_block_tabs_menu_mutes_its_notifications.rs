use super::*;

use be_sdk_beui::NotifyMode;
use be_sdk_test::HostCall;

fn choose(fixture: &mut Fixture, words: &str) {
    let document = fixture.test.document();
    let root = document.root().expect("the workspace has a root");
    let row =
        text_within(&document, root, words).unwrap_or_else(|| panic!("the menu lists {words}"));
    let at = document
        .node_rect(row)
        .expect("the row is laid out")
        .center();
    drop(document);
    fixture.test.click(at);
    fixture.settle();
}

fn says(fixture: &Fixture, words: &str) -> bool {
    let document = fixture.test.document();
    let root = document.root().expect("the workspace has a root");
    text_within(&document, root, words).is_some_and(|node| document.node_rect(node).is_some())
}

#[test]
fn a_block_tabs_menu_mutes_its_notifications() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1000.0, 600.0)));
    show(&mut fixture, opened, None);
    let asked = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::NotifyModeOf { reply, block } if block == opened.into_bytes() => Some(reply),
            _ => None,
        })
        .expect("showing a block asks how its account is notified about it");
    fixture.test.answer(asked, Ok(NotifyMode::All));
    fixture.settle();

    fixture.test.click("dock.editors.menu");
    fixture.settle();
    choose(&mut fixture, "Notifications: All");
    assert!(says(&fixture, "Mentions only") && says(&fixture, "Off"));
    fixture
        .test
        .snapshot("a_block_tabs_menu_mutes_its_notifications");
    choose(&mut fixture, "Off");

    let set: Vec<_> = fixture
        .test
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::SetNotifyMode { block, mode, .. } => Some((block, mode)),
            _ => None,
        })
        .collect();
    assert_eq!(set, [(opened.into_bytes(), NotifyMode::Off)]);
    fixture.test.click("dock.editors.menu");
    fixture.settle();
    assert!(
        says(&fixture, "Notifications: Off"),
        "the menu shows the mode chosen without waiting for the host"
    );
}
