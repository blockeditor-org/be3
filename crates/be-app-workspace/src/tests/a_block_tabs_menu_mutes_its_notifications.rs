use super::*;

use be_sdk_beui::{BlockNotifyMode, Failure, NotifyMode, NotifyModesWatch};
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

fn menu_says(fixture: &mut Fixture, words: &str) -> bool {
    fixture.test.click("dock.editors.menu");
    fixture.settle();
    let said = says(fixture, words);
    fixture
        .test
        .key(be_sdk_beui::beui::Key::Escape, be_sdk_beui::beui::Modifiers::default());
    fixture.settle();
    said
}

#[test]
fn a_block_tabs_menu_mutes_its_notifications() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1000.0, 600.0)));
    show(&mut fixture, opened, None);
    assert!(
        !menu_says(&mut fixture, "Notifications"),
        "the menu offers no mode before the host has said what it is"
    );
    fixture
        .test
        .set_watched::<NotifyModesWatch>(&Some(Vec::new()));
    fixture.settle();

    fixture.test.click("dock.editors.menu");
    fixture.settle();
    choose(&mut fixture, "Notifications: All");
    assert!(says(&fixture, "Mentions only") && says(&fixture, "Off"));
    fixture
        .test
        .snapshot("a_block_tabs_menu_mutes_its_notifications");
    choose(&mut fixture, "Off");
    assert!(
        menu_says(&mut fixture, "Notifications: Off"),
        "the menu shows the mode chosen while the host sets it"
    );

    let set = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::SetNotifyMode { reply, block, mode }
                if block == opened.into_bytes() && mode == NotifyMode::Off =>
            {
                Some(reply)
            }
            _ => None,
        })
        .expect("choosing Off asks the host to set it");
    fixture.test.answer(set, Err(Failure::Denied));
    fixture.settle();
    assert!(
        menu_says(&mut fixture, "Notifications: All"),
        "a change the host refused goes back to the mode it publishes"
    );

    fixture.test.set_watched::<NotifyModesWatch>(&Some(vec![BlockNotifyMode {
        block: opened.into_bytes(),
        mode: NotifyMode::Mentions,
    }]));
    fixture.settle();
    assert!(
        menu_says(&mut fixture, "Notifications: Mentions only"),
        "a mode set on another device shows as the host publishes it"
    );
}
