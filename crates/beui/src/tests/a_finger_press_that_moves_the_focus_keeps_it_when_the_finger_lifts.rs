use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_finger_press_that_moves_the_focus_keeps_it_when_the_finger_lifts() {
    let _root = crate::reactive::RootScope::new();
    let wanted = create_signal(false);
    let pressed = NodeRef::new();
    let field = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Interactive
                    @node_ref={&pressed}
                    focusable=true
                    on_press={move |_: PointerPress| wanted.set(true)}
                >
                    <Frame height=100.0 />
                </Interactive>
                <Interactive @node_ref={&field} focusable=true focused={wanted}>
                    <Frame height=100.0 />
                </Interactive>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let at = harness.rect(pressed.get()).center();
    harness.touch(TouchPhase::Start, at);
    harness.touch(TouchPhase::End, at);
    assert_eq!(
        harness.document().focused_node(),
        Some(field.get()),
        "the focus a press handed on is not taken back when the finger lifts"
    );
}
