use super::*;
use crate::drag::{Candidate, Carry, closest};
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{carrying, use_drop_site};

fn said(harness: &Harness) -> Option<String> {
    harness
        .accessible()
        .into_iter()
        .find(|node| node.role() == accesskit::Role::Status && node.label().is_some())
        .and_then(|node| node.label().map(str::to_owned))
}

#[test]
fn a_carry_lifted_by_the_keyboard_steps_between_its_places_and_says_where_it_is() {
    let _root = RootScope::new();
    let dropped = create_signal(Vec::<&'static str>::new());
    let carried = create_signal(None::<u32>);
    let document = build_node(move || {
        let baskets = [NodeRef::new(), NodeRef::new(), NodeRef::new()];
        let names = ["left", "middle", "right"];
        use_drop_site(
            Func::new(|_: u32| true),
            Func::new(move |_: u32| {
                closest(baskets.iter().zip(names).map(|(basket, name)| {
                    Candidate::new(name, Rect::from_min_size(Pos2::ZERO, vec2(100.0, 40.0)), *basket)
                        .labelled(format!("the {name} basket"))
                }))
            }),
            Callback::new(move |(_, name): (u32, &'static str)| {
                dropped.update(|dropped| dropped.push(name))
            }),
        );
        create_effect(move || carried.set(carrying::<u32>()));
        view! {
            <List spacing=20.0 align=Align::Start>
                <Interactive
                    focusable=true
                    drag={Func::new(|_: PointerPress| Some(Carry::new(7_u32)))}
                >
                    <Frame @test_id="item" width=100.0 height=40.0 />
                </Interactive>
                <List direction=Direction::Horizontal spacing=50.0>
                    <Frame @node_ref={&baskets[0]} width=100.0 height=40.0 />
                    <Frame @node_ref={&baskets[1]} width=100.0 height=40.0 />
                    <Frame @node_ref={&baskets[2]} width=100.0 height=40.0 />
                </List>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    harness.key(Key::Tab, Modifiers::NONE);

    harness.key(Key::Space, Modifiers::NONE);
    assert_eq!(carried.get_untracked(), Some(7), "Space lifts it");
    assert_eq!(
        said(&harness).as_deref(),
        Some("Lifted, over the left basket, place 1 of 3"),
        "and says where it is"
    );
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(said(&harness).as_deref(), Some("Over the middle basket, place 2 of 3"));
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(said(&harness).as_deref(), Some("Over the right basket, place 3 of 3"));
    harness.key(Key::Enter, Modifiers::NONE);
    assert_eq!(dropped.get_untracked(), ["right"], "Enter drops it where it is");
    assert_eq!(said(&harness).as_deref(), Some("Dropped"));

    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(carried.get_untracked(), None, "Tab puts it down");
    assert_eq!(dropped.get_untracked(), ["right"], "dropping nothing");
    assert_eq!(said(&harness).as_deref(), Some("Put down where it was"));
    harness.key(Key::Space, Modifiers::NONE);
    assert_eq!(
        carried.get_untracked(),
        Some(7),
        "and focus stays on the item, which Space lifts again"
    );
    harness.key(Key::Escape, Modifiers::NONE);
    assert_eq!(carried.get_untracked(), None, "Escape puts it down too");
}
