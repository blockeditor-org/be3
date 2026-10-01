use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::*;
use crate::geometry::vec2;
use crate::reactive::{ForEach, Frame, Interactive, ItemSize, List, NodeRef, build, view};
use crate::unstyled::Scroll;

#[test]
fn a_scroll_keeps_the_pointer_off_the_part_of_a_row_it_clips() {
    let hovers: Rc<RefCell<Vec<bool>>> = Rc::default();
    let clicks = Rc::new(Cell::new(0));
    let scroll = NodeRef::new();
    let document = build({
        let hovers = hovers.clone();
        let clicks = clicks.clone();
        let scroll = scroll.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Frame height=100.0 />
                    <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <Interactive
                            on_hover_change={move |hovered: bool| hovers.borrow_mut().push(hovered)}
                            on_click={move || clicks.set(clicks.get() + 1)}
                        >
                            <Frame height=100.0 />
                        </Interactive>
                        <ForEach keys={indices(20)}>
                            {|_: usize| view! {
                                <Frame height=100.0 />
                            }}
                        </ForEach>
                    </Scroll>
                </List>
            }
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let x = harness.rect(scroll).center().x;

    harness.scroll(pos2(x, 299.0), vec2(0.0, -60.0), Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(harness.document().scroll_offset(scroll), 60.0);

    harness.frame(vec![Event::PointerMoved(pos2(x, 50.0))]);
    assert_eq!(
        *hovers.borrow(),
        Vec::<bool>::new(),
        "the part of the row scrolled above the scroll is not hovered"
    );
    harness.click(pos2(x, 50.0));
    assert_eq!(clicks.get(), 0, "nor can it be clicked");

    harness.frame(vec![Event::PointerMoved(pos2(x, 120.0))]);
    assert_eq!(*hovers.borrow(), vec![true]);

    harness.frame(vec![Event::PointerMoved(pos2(x, 50.0))]);
    assert_eq!(*hovers.borrow(), vec![true, false]);
}
