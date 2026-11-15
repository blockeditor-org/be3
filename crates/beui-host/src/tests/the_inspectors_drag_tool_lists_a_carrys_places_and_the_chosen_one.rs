use super::*;
use crate::drag::{Candidate, Carry, DragPoint, closest};
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::use_drop_site;

#[test]
fn the_inspectors_drag_tool_lists_a_carrys_places_and_the_chosen_one() {
    let _root = RootScope::new();
    let board = std::rc::Rc::new(std::cell::RefCell::new(None));
    let held = std::rc::Rc::clone(&board);
    let document = build_node(move || {
        *held.borrow_mut() = Some(crate::unstyled::drag_board());
        let baskets = [NodeRef::new(), NodeRef::new()];
        let names = ["left", "right"];
        use_drop_site(
            Func::new(|_: u32| true),
            Func::new(move |_: u32| {
                closest(baskets.iter().zip(names).map(|(basket, name)| {
                    Candidate::new(name, Rect::from_min_size(Pos2::ZERO, vec2(60.0, 40.0)), *basket)
                        .labelled(format!("the {name} basket"))
                }))
            }),
            Callback::new(|_: (u32, &'static str)| {}),
        );
        view! {
            <List direction=Direction::Horizontal spacing=40.0>
                <Frame @node_ref={&baskets[0]} width=60.0 height=40.0 />
                <Frame @node_ref={&baskets[1]} width=60.0 height=40.0 />
            </List>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.settle();
    harness.toggle_inspector();
    harness.show_inspector_tab(crate::inspector::InspectorTab::Drags);
    assert!(harness.inspector_open(), "the Drag tab is clicked, not a pin beside it");
    let read = |harness: &mut Harness| {
        harness.frame(Vec::new());
        harness.inspector().drag_rows()
    };
    let first = read(&mut harness);
    assert!(first.contains(&"Nothing is carried".to_owned()), "{first:?}");

    let board = board.borrow().clone().expect("the program made its board");
    board.lift(
        Carry::new(1_u32).item(Rect::from_min_size(pos2(-30.0, -20.0), vec2(60.0, 40.0))),
        DragPoint::at(pos2(30.0, 60.0), Modifiers::NONE),
    );
    let texts = read(&mut harness);
    for expected in [
        "Carried, on a place",
        "the left basket: 0,40 60x40 (chosen)",
        "the right basket: 100,40 60x40",
    ] {
        assert!(
            texts.iter().any(|text| text == expected),
            "{expected:?} is missing from {texts:?}"
        );
    }
}
