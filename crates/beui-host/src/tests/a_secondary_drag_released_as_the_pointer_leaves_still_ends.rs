use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui::SecondaryDrag;

#[test]
fn a_secondary_drag_released_as_the_pointer_leaves_still_ends() {
    let _root = crate::reactive::RootScope::new();
    let drags = create_signal(Vec::<SecondaryDrag>::new());
    let document = build_node(move || {
        view! {
            <Interactive
                on_pointer={crate::unstyled::gestures::secondary_drag(move |drag: SecondaryDrag| drags.update(|drags| drags.push(drag)))}
            >
                <Frame />
            </Interactive>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (from, to) = (pos2(50.0, 50.0), pos2(120.0, 90.0));
    harness.frame(vec![Input::PointerMoved(from)]);
    harness.frame(vec![Input::PointerButton {
        pos: from,
        button: PointerButton::Secondary,
        pressed: true,
    }]);
    harness.frame(vec![Input::PointerMoved(to)]);
    harness.frame(vec![
        Input::PointerButton {
            pos: to,
            button: PointerButton::Secondary,
            pressed: false,
        },
        Input::PointerLeft,
    ]);

    let last = *drags.get_untracked().last().expect("the drag was heard");
    assert!(last.ended, "the release ended the drag");
    assert!(
        !last.cancelled,
        "the pointer leaving after the release does not take the drag back"
    );
    assert_eq!(last.pos, to);
}
