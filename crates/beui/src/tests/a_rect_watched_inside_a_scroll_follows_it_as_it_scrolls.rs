use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::geometry::vec2;
use crate::reactive::{ForEach, List, build, component, component_rect, create_effect, view};
use crate::unstyled::Scroll;

#[test]
fn a_rect_watched_inside_a_scroll_follows_it_as_it_scrolls() {
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
        "a row that moves with its scroll reports where it moved to, \
         though it was not laid out again"
    );
}

#[component]
fn Row(watched: bool, seen: Rc<Cell<Rect>>) -> NodeId {
    let rect = component_rect();
    create_effect(move || {
        let placed = rect.get();
        if watched {
            seen.set(placed);
        }
    });
    view! {
        <Frame height=40.0 color=Color32::WHITE radius=0 />
    }
}
