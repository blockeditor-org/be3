use super::*;
use crate::primitives::{Frame, List};
use crate::reactive::*;

/// A sheet at its top stop (540 of a 600-point screen) over 2000 points of content, scrolled 300
/// into it: its grip lowers it at once, what it holds moving with it, and let go back at the top
/// the scroll is where it was.
#[test]
fn a_scrolled_sheets_grip_lowers_it_and_leaves_what_it_holds_where_it_was() {
    let _root = RootScope::new();
    let sheet = NodeRef::new();
    let row = NodeRef::new();
    let closed = create_signal(0);
    let document = build_node(move || {
        view! {
            <ModalSheet
                @node_ref=&sheet
                open=true
                rest=0.9
                on_close={move || closed.update(|n| *n += 1)}
            >
                <List spacing=0.0>
                    <Frame height=500.0 />
                    <Frame @node_ref=&row height=40.0 />
                    <Frame height=1460.0 />
                </List>
            </ModalSheet>
        }
    });
    let mut harness = Harness::sized(document, vec2(400.0, 600.0));
    harness.settle();
    let offset = |harness: &Harness| harness.document().scroll_offset(sheet.get());
    let wheel = |harness: &mut Harness, by: f32| {
        harness.scroll_at(pos2(200.0, 400.0), vec2(0.0, by), Modifiers::NONE);
        harness.settle();
    };
    assert_eq!(offset(&harness), 540.0, "up at its top stop");
    wheel(&mut harness, -300.0);
    assert_eq!(offset(&harness), 840.0, "what it holds scrolled 300 in");

    let handle = harness.find("sheet.handle");
    let top = harness.rect(handle).top();
    let under = |harness: &Harness| harness.rect(row.get()).top() - harness.rect(handle).top();
    let held = under(&harness);
    let from = harness.center(handle);

    harness.touch(TouchPhase::Start, from);
    harness.touch(TouchPhase::Move, from + vec2(0.0, 60.0));
    harness.touch(TouchPhase::Move, from + vec2(0.0, 120.0));
    assert!(
        harness.rect(handle).top() > top + 60.0,
        "the grip lowers the sheet at once"
    );
    assert_eq!(under(&harness), held, "and what it holds moves with it, not scrolled back");
    harness.touch(TouchPhase::Move, from + vec2(0.0, 60.0));
    harness.touch(TouchPhase::Move, from + vec2(0.0, 30.0));
    harness.touch(TouchPhase::End, from + vec2(0.0, 30.0));
    harness.settle();
    assert_eq!(
        harness.rect(handle).top(),
        top,
        "dragged most of the way back up and let go, it springs to its top"
    );
    assert_eq!(offset(&harness), 840.0, "with the scroll where it was");
    assert_eq!(closed.get_untracked(), 0);
    wheel(&mut harness, -40.0);
    assert_eq!(offset(&harness), 880.0, "and a wheel scrolls on from there by its step");

    let from = harness.center(handle);
    harness.touch(TouchPhase::Start, from);
    harness.touch(TouchPhase::Move, from + vec2(0.0, 40.0));
    harness.touch(TouchPhase::Move, from + vec2(0.0, 80.0));
    assert!(harness.rect(handle).top() > top + 20.0);
    harness.touch(TouchPhase::Cancel, from + vec2(0.0, 80.0));
    harness.settle();
    assert_eq!(harness.rect(handle).top(), top, "a cancelled grip touch lets it go back up");
    assert_eq!(offset(&harness), 880.0, "with the scroll where it was");
    assert_eq!(closed.get_untracked(), 0, "and nothing closed it");
    wheel(&mut harness, -40.0);
    assert_eq!(offset(&harness), 920.0);
}
