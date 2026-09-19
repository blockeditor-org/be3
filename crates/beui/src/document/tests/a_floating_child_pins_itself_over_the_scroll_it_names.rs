use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::reactive::{Button, ForEach, List, NodeRef, Scroll, Text, build, view};
use crate::unstyled::{Edge, Floating};

#[test]
fn a_floating_child_pins_itself_over_the_scroll_it_names() {
    let pressed = Rc::new(Cell::new(0u32));
    let scroll = NodeRef::new();
    let button = NodeRef::new();
    let rows = Rc::new(Cell::new(0u32));
    let document = build({
        let (pressed, scroll, button, rows) = (
            Rc::clone(&pressed),
            scroll.clone(),
            button.clone(),
            Rc::clone(&rows),
        );
        move || {
            view! {
                <List spacing=0.0>
                    <Scroll @node_ref=&scroll @sizing=ItemSize::Percent(100.0)>
                        <ForEach keys={(0..20).collect::<Vec<i64>>()}>
                            {move |row: i64| {
                                let rows = Rc::clone(&rows);
                                view! {
                                    <Button on_click={move || rows.set(rows.get() + 1)}>
                                        <Text string={format!("row {row}")} />
                                    </Button>
                                }
                            }}
                        </ForEach>
                    </Scroll>
                    <Floating anchor={scroll.clone()} edge=Edge::Bottom>
                        <Button @node_ref=&button on_click={move || pressed.set(pressed.get() + 1)}>
                            <Text string="reveal" />
                        </Button>
                    </Floating>
                </List>
            }
        }
    });

    let (scroll, button) = (scroll.get(), button.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let viewport = harness.rect(scroll);
    let floating = harness.rect(button);
    assert!(
        viewport.height() > floating.height() * 2.0,
        "the floating child must not take the scroll's height from it"
    );
    assert!(
        (floating.bottom() - viewport.bottom()).abs() < 1.0,
        "the floating child must sit on the edge it named"
    );
    assert!(viewport.contains_rect(floating));

    let before = rows.get();
    harness.click(floating.center());
    harness.frame(Vec::new());

    assert_eq!(pressed.get(), 1, "the floating child answers the pointer");
    assert_eq!(
        rows.get(),
        before,
        "a press the floating child took must not reach the rows under it"
    );

    harness.click(Pos2::new(viewport.center().x, viewport.top() + 4.0));
    harness.frame(Vec::new());

    assert_eq!(
        rows.get(),
        before + 1,
        "the rest of the scroll must go on answering the pointer"
    );
}
