use super::*;
use crate::drag::{Candidate, Carry, fixed};
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{carrying, use_drop_site};

#[test]
fn a_finger_pressed_while_the_mouse_carries_neither_moves_nor_drops_its_carry() {
    let _root = RootScope::new();
    let dropped = create_signal(Vec::<&'static str>::new());
    let over = create_signal((None::<()>, None::<()>));
    let carried = create_signal(None::<u32>);
    let document = build_node(move || {
        let left = NodeRef::new();
        let right = NodeRef::new();
        let site = |node: NodeRef, name: &'static str| {
            use_drop_site(
                Func::new(|_: u32| true),
                Func::new(move |_: u32| {
                    fixed(Candidate::new(
                        (),
                        Rect::from_min_size(Pos2::ZERO, vec2(100.0, 40.0)),
                        node,
                    ))
                }),
                Callback::new(move |_: (u32, ())| dropped.update(|dropped| dropped.push(name))),
            )
        };
        let on_left = site(left, "left");
        let on_right = site(right, "right");
        create_effect(move || over.set((on_left.over.get(), on_right.over.get())));
        create_effect(move || carried.set(carrying::<u32>().first().copied()));
        view! {
            <List spacing=10.0>
                <Interactive drag={Func::new(|_: PointerPress| Some(Carry::new(7_u32)))}>
                    <Frame @test_id="item" width=100.0 height=40.0 />
                </Interactive>
                <List direction=Direction::Horizontal spacing=200.0>
                    <Frame @node_ref=&left @test_id="left" width=100.0 height=40.0 />
                    <Frame @node_ref=&right @test_id="right" width=100.0 height=40.0 />
                </List>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let grab = harness.center(harness.find("item"));
    let left = harness.center(harness.find("left"));
    let right = harness.center(harness.find("right"));
    harness.down(grab, PointerButton::Primary);
    for step in 1..=4 {
        let at = grab + (left - grab) * (step as f32 / 4.0);
        harness.frame(vec![Input::PointerMoved(at)]);
    }
    harness.settle();
    assert_eq!(
        over.get_untracked(),
        (Some(()), None),
        "the mouse carries it over the left"
    );

    harness.touch(TouchPhase::Start, right + vec2(0.0, 30.0));
    harness.touch(TouchPhase::Move, right);
    harness.settle();
    assert_eq!(
        over.get_untracked(),
        (Some(()), None),
        "a finger moving over the right does not move the mouse's carry"
    );
    harness.touch(TouchPhase::End, right);
    harness.settle();
    assert_eq!(
        carried.get_untracked(),
        Some(7),
        "and its release does not put it down"
    );

    for step in 1..=4 {
        let at = left + (right - left) * (step as f32 / 4.0);
        harness.frame(vec![Input::PointerMoved(at)]);
    }
    harness.up(right, PointerButton::Primary);
    harness.settle();
    assert_eq!(
        dropped.get_untracked(),
        ["right"],
        "the mouse moves and drops it"
    );
}
