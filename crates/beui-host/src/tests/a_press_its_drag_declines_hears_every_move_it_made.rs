use super::*;
use crate::primitives::*;
use crate::reactive::*;

/// A press `drag` declines is the source's own drag once past the threshold: it hears the
/// moves held short of it first (a stroke keeps its first points), and stays a drag however
/// near its start it comes back, so a pan returned to its origin ends there. A press that
/// ends short of the threshold hears its moves as it ends, after its click.
#[test]
fn a_press_its_drag_declines_hears_every_move_it_made() {
    let _root = RootScope::new();
    let heard = create_signal(Vec::<f32>::new());
    let document = build_node(move || {
        crate::unstyled::drag_board();
        view! {
            <Interactive
                drag={Func::new(|_: PointerPress| None)}
                on_drag={move |press: PointerPress| heard.update(|heard| heard.push(press.local.y))}
                on_click={move || heard.update(|heard| heard.push(-1.0))}
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
    let past = heard.get_untracked();
    // The first heard is the press's own point.
    let start = past[0];
    assert_eq!(
        past[1..],
        [start + 2.0, start + 20.0],
        "past it, the move held short of it comes first: {past:?}"
    );
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 1.0))]);
    harness.up(grab + vec2(0.0, 1.0), PointerButton::Primary);
    let pan: Vec<f32> = heard
        .get_untracked()
        .into_iter()
        .filter(|heard| *heard >= 0.0)
        .collect();
    assert_eq!(
        pan.last().copied(),
        Some(start + 1.0),
        "back within the threshold of its start, it still hears the move: {pan:?}"
    );

    heard.set(Vec::new());
    harness.down(grab, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 2.0))]);
    harness.up(grab + vec2(0.0, 2.0), PointerButton::Primary);
    let short = heard.get_untracked();
    assert_eq!(
        short,
        [-1.0, start, start + 2.0],
        "a press that ends short of the threshold hears its click, then its moves: {short:?}"
    );
}
