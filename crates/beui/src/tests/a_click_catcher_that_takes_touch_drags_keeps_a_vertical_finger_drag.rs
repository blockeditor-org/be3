use super::*;
use crate::input::PointerPress;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_click_catcher_that_takes_touch_drags_keeps_a_vertical_finger_drag() {
    let _root = crate::reactive::RootScope::new();
    let drags = create_signal::<Vec<Pos2>>(Vec::new());
    let cancelled = create_signal(false);
    let document = build_node(move || {
        view! {
            <Interactive
                touch_drags=true
                on_drag={move |press: PointerPress| drags.update(|drags| drags.push(press.pos))}
                on_cancel={move || cancelled.set(true)}
            >
                <Frame width=300.0 height=300.0 />
            </Interactive>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.touch(TouchPhase::Start, pos2(100.0, 50.0));
    harness.touch(TouchPhase::Move, pos2(100.0, 120.0));
    harness.touch(TouchPhase::Move, pos2(102.0, 200.0));
    harness.touch(TouchPhase::End, pos2(102.0, 200.0));

    assert!(
        !cancelled.get_untracked(),
        "a vertical drag is not taken for a scroll"
    );
    assert_eq!(drags.get_untracked().last(), Some(&pos2(102.0, 200.0)));
}
