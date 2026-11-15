use super::*;
use crate::drag::{Candidate, Carry, closest};
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::use_drop_site;

fn said(harness: &Harness) -> Option<String> {
    harness
        .accessible()
        .into_iter()
        .find(|node| node.role() == accesskit::Role::Status && node.label().is_some())
        .and_then(|node| node.label().map(str::to_owned))
}

/// The baskets of a `closest(..).thrown()` site, the far one chosen only while the pointer is
/// in a zone beside its rest, as a dock's edge band is apart from the half pane it rests
/// in. A keyboard step jumps a basket's width at once, which no throw may read as speed, and
/// reaches the zoned basket by aiming at its zone.
#[test]
fn a_keyboard_carry_drops_on_the_place_it_said_thrown_or_zoned() {
    let _root = RootScope::new();
    let dropped = create_signal(Vec::<&'static str>::new());
    let document = build_node(move || {
        let baskets = [NodeRef::new(), NodeRef::new(), NodeRef::new(), NodeRef::new()];
        let names = ["left", "middle", "right", "far"];
        use_drop_site(
            Func::new(|_: u32| true),
            Func::new(move |_: u32| {
                closest(baskets.iter().zip(names).map(|(basket, name)| {
                    let place = Candidate::new(
                        name,
                        Rect::from_min_size(Pos2::ZERO, vec2(60.0, 40.0)),
                        *basket,
                    )
                    .labelled(format!("the {name} basket"));
                    match name {
                        "far" => place.zoned(Rect::from_min_size(pos2(70.0, 0.0), vec2(60.0, 40.0))),
                        _ => place,
                    }
                }))
                .thrown()
            }),
            Callback::new(move |(_, name): (u32, &'static str)| {
                dropped.update(|dropped| dropped.push(name))
            }),
        );
        view! {
            <List spacing=20.0 align=Align::Start>
                <Interactive
                    focusable=true
                    drag={Func::new(|_: PointerPress| Some(Carry::new(7_u32)))}
                >
                    <Frame width=60.0 height=40.0 />
                </Interactive>
                <List direction=Direction::Horizontal spacing=30.0>
                    <Frame @node_ref={&baskets[0]} width=60.0 height=40.0 />
                    <Frame @node_ref={&baskets[1]} width=60.0 height=40.0 />
                    <Frame @node_ref={&baskets[2]} width=60.0 height=40.0 />
                    <Frame @node_ref={&baskets[3]} width=60.0 height=40.0 />
                </List>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    harness.key(Key::Tab, Modifiers::NONE);

    harness.key(Key::Space, Modifiers::NONE);
    // A frame between the lift and the step, so the step's jump has a speed a throw could
    // read, and Enter at once after it.
    harness.frame(Vec::new());
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(said(&harness).as_deref(), Some("Over the middle basket, place 2 of 4"));
    harness.key(Key::Enter, Modifiers::NONE);
    assert_eq!(
        dropped.get_untracked(),
        ["middle"],
        "a drop at once after a step lands where it said, not thrown on"
    );

    harness.key(Key::Space, Modifiers::NONE);
    for _ in 0..3 {
        harness.key(Key::ArrowRight, Modifiers::NONE);
    }
    assert_eq!(said(&harness).as_deref(), Some("Over the far basket, place 4 of 4"));
    harness.key(Key::Space, Modifiers::NONE);
    assert_eq!(
        dropped.get_untracked(),
        ["middle", "far"],
        "a zoned place an arrow reaches is the one dropped on"
    );
}
