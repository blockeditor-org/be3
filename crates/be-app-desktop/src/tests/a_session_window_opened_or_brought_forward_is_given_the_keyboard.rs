use be_sdk_beui::ChildId;

use super::*;

fn child_of(fixture: &Fixture, session: Uuid) -> ChildId {
    fixture
        .test
        .children()
        .iter()
        .find_map(|placement| match placement.content {
            ChildContent::Block { block_id, .. } if Uuid::from_bytes(block_id) == session => {
                Some(placement.child)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("the session {session} is placed"))
}

#[test]
fn a_session_window_opened_or_brought_forward_is_given_the_keyboard() {
    let (mut fixture, sessions) =
        Fixture::new().with_sessions(&[("Laptop", WORKSPACE_EDITOR), ("Desk", WORKSPACE_EDITOR)]);
    fixture.settle();
    assert_eq!(fixture.test.take_child_focus(), Vec::new());

    for session in &sessions {
        fixture.test.click("desktop.apps");
        fixture.settle();
        fixture
            .test
            .click(&format!("desktop.apps.sessions.{session}"));
        fixture.settle();
        let child = child_of(&fixture, *session);
        assert_eq!(
            fixture.test.take_child_focus().last(),
            Some(&child),
            "the session just opened is given the keyboard"
        );
    }

    fixture.test.click("desktop.windows.block.2");
    fixture.settle();
    assert_eq!(
        fixture.test.take_child_focus().last(),
        Some(&child_of(&fixture, sessions[0])),
        "the window whose chip was clicked is given the keyboard"
    );

    fixture.close_a_tab();
    let left = fixture.placed_blocks();
    let [(block, _, _)] = left[..] else {
        panic!("one session is left: {left:?}");
    };
    assert_eq!(
        fixture.test.take_child_focus().last(),
        Some(&child_of(&fixture, block)),
        "closing a window gives the keyboard to the window left in front"
    );
}
