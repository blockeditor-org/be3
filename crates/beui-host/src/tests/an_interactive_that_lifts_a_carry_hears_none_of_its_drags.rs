use super::*;
use crate::drag::Carry;
use crate::primitives::*;
use crate::reactive::*;

/// One `Interactive` that both carries pieces and pans, as a game board does: a press its
/// `drag` lifts belongs to the board, and only a press it declines pans.
#[test]
fn an_interactive_that_lifts_a_carry_hears_none_of_its_drags() {
    let _root = RootScope::new();
    let lifts = create_signal(true);
    let panned = create_signal(0_u32);
    let document = build_node(move || {
        crate::unstyled::drag_board();
        view! {
            <Interactive
                drag={Func::new(move |_: PointerPress| lifts.get_untracked().then(|| Carry::new(1_u32)))}
                on_drag={move |_: PointerPress| panned.update(|panned| *panned += 1)}
            >
                <Frame @test_id="board" width=200.0 height=200.0 />
            </Interactive>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let grab = harness.center(harness.find("board"));
    let drag = |harness: &mut Harness| {
        harness.down(grab, PointerButton::Primary);
        for step in 1..=5 {
            harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 10.0 * step as f32))]);
        }
        harness.up(grab + vec2(0.0, 50.0), PointerButton::Primary);
        harness.settle();
    };

    drag(&mut harness);
    assert_eq!(panned.get_untracked(), 0, "a lifted press never pans");

    lifts.set(false);
    drag(&mut harness);
    assert!(panned.get_untracked() > 0, "a press drag declines pans");
}
