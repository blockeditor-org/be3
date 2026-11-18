use super::*;
use crate::drag::{Carry, free};
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::use_drop_site;

#[component]
fn Thumb(name: u32, over: Signal<Vec<(u32, f32)>>, dropped: Signal<Vec<(u32, f32)>>) -> NodeId {
    let track = NodeRef::new();
    let site = use_drop_site(
        Func::new(move |carried: u32| carried == name),
        Func::new(move |_: u32| free(track, |rect: Rect| rect.min.x).unbounded()),
        Callback::new(move |(_, at): (u32, f32)| {
            dropped.update(|dropped| dropped.push((name, at)))
        }),
    );
    create_effect(move || {
        if let Some(at) = site.over.get() {
            over.update(|over| over.push((name, at)));
        }
    });
    view! {
        <Frame @node_ref=&track width=300.0 height=40.0>
            <Interactive
                lifts_on_press=true
                touch_drags={TouchDrags::Along(Direction::Horizontal)}
                drag={Func::new(move |_: PointerPress| Some(Carry::new(name).drawn_by_site()))}
            >
                <Frame @test_id={format!("thumb{name}")} width=20.0 height=40.0 />
            </Interactive>
        </Frame>
    }
}

#[test]
fn the_mouse_and_a_finger_each_carry_their_own_source_at_once() {
    let _root = RootScope::new();
    let over = create_signal(Vec::<(u32, f32)>::new());
    let dropped = create_signal(Vec::<(u32, f32)>::new());
    let document = build_node(move || {
        view! {
            <List spacing=20.0 align=Align::Start>
                <Thumb name=1 over dropped />
                <Thumb name=2 over dropped />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let one = harness.center(harness.find("thumb1"));
    let two = harness.center(harness.find("thumb2"));
    let last = |name: u32| {
        over.with_untracked(|over| {
            over.iter()
                .rev()
                .find(|(of, _)| *of == name)
                .map(|(_, at)| *at)
        })
    };

    harness.down(one, PointerButton::Primary);
    harness.touch(TouchPhase::Start, two);
    harness.touch(TouchPhase::Move, two + vec2(60.0, 0.0));
    harness.frame(vec![Input::PointerMoved(one + vec2(40.0, 0.0))]);
    assert_eq!(last(1), Some(40.0), "the mouse moves the first thumb");
    assert_eq!(last(2), Some(60.0), "while the finger moves the second");
    harness.touch(TouchPhase::End, two + vec2(60.0, 0.0));
    harness.frame(vec![Input::PointerMoved(one + vec2(50.0, 0.0))]);
    harness.up(one + vec2(50.0, 0.0), PointerButton::Primary);
    harness.settle();
    assert_eq!(
        dropped.get_untracked(),
        vec![(2, 60.0), (1, 50.0)],
        "each drops where its own pointer lets go"
    );
}
