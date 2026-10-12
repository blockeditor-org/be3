use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::geometry::vec2;
use crate::reactive::{
    ForEach, List, PlacementBuilder, ReadSignal, build, component, create_effect, view,
};
use crate::unstyled::Scroll;

#[test]
fn a_placement_built_inside_a_scroll_follows_it_without_laying_its_content_out_again() {
    let seen = Rc::new(Cell::new(Rect::NOTHING));
    let document = build({
        let seen = Rc::clone(&seen);
        move || {
            view! {
                <List spacing=0.0>
                    <Frame height=200.0>
                        <Scroll @test_id="scroll">
                            <ForEach keys={(0..20).collect::<Vec<usize>>()}>
                                {move |row: usize| {
                                    let seen = Rc::clone(&seen);
                                    view! {
                                        <Row watched={row == 2} seen />
                                    }
                                }}
                            </ForEach>
                        </Scroll>
                    </Frame>
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(seen.get().top(), 80.0);
    let scroll = harness.find("scroll");
    harness.frame(vec![Event::PointerMoved(harness.rect(scroll).center())]);

    harness.frame(vec![Event::Scroll(vec2(0.0, -30.0))]);
    assert_eq!(harness.document().scroll_offset(scroll), 30.0);
    assert_eq!(
        seen.get().top(),
        50.0,
        "a row built from its placement hears where its scroll moved it"
    );
    let latest = &harness.document().performance().latest;
    assert_eq!(latest.layout_passes, 1, "following the scroll takes no second layout");
    assert!(
        latest.work.placed < 20,
        "the rows under the placements are not laid out again, but {} nodes were",
        latest.work.placed
    );
}

#[component]
fn Row(watched: bool, seen: Rc<Cell<Rect>>) -> NodeId {
    view! {
        <PlacementBuilder>
            {move |rect: ReadSignal<Rect>| {
                create_effect(move || {
                    let placed = rect.get();
                    if watched {
                        seen.set(placed);
                    }
                });
                view! {
                    <Frame height=40.0 color=Color32::WHITE radius=0 />
                }
            }}
        </PlacementBuilder>
    }
}
