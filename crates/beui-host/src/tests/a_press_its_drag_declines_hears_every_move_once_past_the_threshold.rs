use super::*;
use crate::primitives::*;
use crate::reactive::*;

/// A press `drag` declines is the source's own drag once past the threshold, and stays one
/// however near its start it comes back: a pan returned to its origin ends there.
#[test]
fn a_press_its_drag_declines_hears_every_move_once_past_the_threshold() {
    let _root = RootScope::new();
    let heard = create_signal(Vec::<f32>::new());
    let document = build_node(move || {
        crate::unstyled::drag_board();
        view! {
            <Interactive
                drag={Func::new(|_: PointerPress| None)}
                on_drag={move |press: PointerPress| heard.update(|heard| heard.push(press.local.y))}
            >
                <Frame @test_id="board" width=200.0 height=200.0 />
            </Interactive>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let grab = harness.center(harness.find("board"));
    harness.down(grab, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 2.0))]);
    assert!(
        heard.get_untracked().is_empty(),
        "a move short of the threshold is not yet a drag"
    );
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 20.0))]);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 1.0))]);
    harness.up(grab + vec2(0.0, 1.0), PointerButton::Primary);
    let heard = heard.get_untracked();
    assert_eq!(
        heard.last().copied(),
        Some(101.0),
        "back within the threshold of its start, it still hears the move: {heard:?}"
    );
}
