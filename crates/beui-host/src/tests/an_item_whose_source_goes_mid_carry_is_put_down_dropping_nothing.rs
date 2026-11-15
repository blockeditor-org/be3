use super::*;
use crate::drag::{Candidate, Carry, fixed};
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{carrying, use_drop_site};

#[test]
fn an_item_whose_source_goes_mid_carry_is_put_down_dropping_nothing() {
    let _root = RootScope::new();
    let shown = create_signal(true);
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
            <List spacing=10.0>
                <Show value={shown}>
                    <Interactive drag={Func::new(|_: PointerPress| Some(Carry::new(7_u32)))}>
                        <Frame @test_id="item" width=100.0 height=40.0 />
                    </Interactive>
                </Show>
                <Frame @node_ref=&basket @test_id="basket" width=100.0 height=40.0 />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let grab = harness.center(harness.find("item"));
    harness.down(grab, PointerButton::Primary);
    for step in 1..=4 {
        harness.frame(vec![Input::PointerMoved(grab + vec2(0.0, 12.5 * step as f32))]);
    }
    harness.settle();
    assert_eq!(carried.get_untracked(), Some(7));

    shown.set(false);
    harness.settle();
    assert_eq!(carried.get_untracked(), None, "the source gone, the carry is put down");
    harness.up(grab + vec2(0.0, 50.0), PointerButton::Primary);
    harness.settle();
    assert_eq!(dropped.get_untracked(), 0, "and nothing is dropped");
}
