use super::*;
use crate::drag::{Candidate, Carry, fixed};
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{carrying, use_drop_site};

#[test]
fn a_finger_held_on_a_source_lifts_its_carry_and_drops_it_where_it_moves() {
    let _root = RootScope::new();
    let dropped = create_signal(0_u32);
    let carried = create_signal(None::<u32>);
    let document = build_node(move || {
        let basket = NodeRef::new();
        use_drop_site(
            Func::new(|_: u32| true),
            Func::new(move |_: u32| {
                fixed(Candidate::new(
                    (),
                    Rect::from_min_size(Pos2::ZERO, vec2(100.0, 40.0)),
                    basket,
                ))
            }),
            Callback::new(move |_: (u32, ())| dropped.update(|dropped| *dropped += 1)),
        );
        create_effect(move || carried.set(carrying::<u32>()));
        view! {
            <List spacing=60.0 align=Align::Start>
                <Interactive
                    lifts_on_long_press=true
                    drag={Func::new(|_: PointerPress| Some(Carry::new(7_u32)))}
                >
                    <Frame @test_id="item" width=100.0 height=40.0 />
                </Interactive>
                <Frame @node_ref=&basket @test_id="basket" width=100.0 height=40.0 />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let grab = harness.center(harness.find("item"));
    let basket = harness.center(harness.find("basket"));
    harness.touch(TouchPhase::Start, grab);
    harness.advance(crate::input::LONG_PRESS_DELAY);
    harness.frame(Vec::new());
    assert_eq!(
        carried.get_untracked(),
        Some(7),
        "holding a finger still lifts it"
    );
    harness.touch(TouchPhase::Move, basket);
    harness.frame(Vec::new());
    harness.touch(TouchPhase::End, basket);
    harness.settle();
    assert_eq!(
        dropped.get_untracked(),
        1,
        "and it drops where the finger lets go"
    );
    assert_eq!(carried.get_untracked(), None);
}
