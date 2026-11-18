use super::*;
use crate::drag::{Carry, free};
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{carrying, use_drop_site};

#[test]
fn a_mouse_press_lifts_a_source_that_lifts_on_press_and_a_finger_lifts_past_its_threshold() {
    let _root = RootScope::new();
    let over = create_signal(None::<Pos2>);
    let dropped = create_signal(None::<Pos2>);
    let carried = create_signal(None::<u32>);
    let document = build_node(move || {
        let track = NodeRef::new();
        let site = use_drop_site(
            Func::new(|_: u32| true),
            Func::new(move |_: u32| free(track, |rect: Rect| rect.min).unbounded()),
            Callback::new(move |(_, at): (u32, Pos2)| dropped.set(Some(at))),
        );
        create_effect(move || over.set(site.over.get()));
        create_effect(move || carried.set(carrying::<u32>()));
        view! {
            <Frame @node_ref=&track width=300.0 height=40.0>
                <Interactive
                    lifts_on_press=true
                    touch_drags={TouchDrags::Along(Direction::Horizontal)}
                    drag={Func::new(|_: PointerPress| {
                        Some(Carry::new(7_u32).drawn_by_site().scrolls_nothing())
                    })}
                >
                    <Frame @test_id="thumb" width=20.0 height=40.0 />
                </Interactive>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let grab = harness.center(harness.find("thumb"));

    harness.down(grab, PointerButton::Primary);
    assert_eq!(carried.get_untracked(), Some(7), "a mouse's press lifts it");
    assert_eq!(over.get_untracked(), Some(pos2(0.0, 0.0)), "where it is");
    harness.frame(vec![Input::PointerMoved(grab + vec2(1.0, 0.0))]);
    assert_eq!(
        over.get_untracked(),
        Some(pos2(1.0, 0.0)),
        "and it follows the pointer from the first point, short of the threshold"
    );
    harness.up(grab + vec2(1.0, 0.0), PointerButton::Primary);
    harness.settle();
    assert_eq!(dropped.get_untracked(), Some(pos2(1.0, 0.0)), "it drops there");
    assert_eq!(carried.get_untracked(), None);

    let grab = harness.center(harness.find("thumb"));
    harness.touch(TouchPhase::Start, grab);
    harness.frame(Vec::new());
    assert_eq!(carried.get_untracked(), None, "a finger's press does not");
    harness.touch(TouchPhase::Move, grab + vec2(2.0, 0.0));
    harness.frame(Vec::new());
    assert_eq!(carried.get_untracked(), None, "nor its move short of the threshold");
    harness.touch(TouchPhase::Move, grab + vec2(40.0, 0.0));
    harness.frame(Vec::new());
    assert_eq!(carried.get_untracked(), Some(7), "a drag past it lifts it");
    harness.touch(TouchPhase::End, grab + vec2(40.0, 0.0));
    harness.settle();
    assert_eq!(carried.get_untracked(), None);
}
