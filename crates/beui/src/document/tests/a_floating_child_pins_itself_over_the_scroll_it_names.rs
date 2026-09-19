use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::reactive::create_effect;
use crate::reactive::{
    Button, ClickCatcher, ForEach, List, NodeRef, Scroll, Text, build, create_signal, view,
};
use crate::unstyled::{Edge, Floating};

#[test]
fn a_floating_child_pins_itself_over_the_scroll_it_names() {
    let pressed = Rc::new(Cell::new(0u32));
    let scroll = NodeRef::new();
    let button = NodeRef::new();
    let rows = Rc::new(Cell::new(0u32));
    let hovered = Rc::new(Cell::new(0usize));
    let document = build({
        let (pressed, scroll, button, rows, hovered) = (
            Rc::clone(&pressed),
            scroll.clone(),
            button.clone(),
            Rc::clone(&rows),
            Rc::clone(&hovered),
        );
        move || {
            let (over, set_over) = create_signal(0usize);
            create_effect(move || hovered.set(over.get()));
            view! {
                <List spacing=0.0>
                    <Scroll @node_ref=&scroll @sizing=ItemSize::Percent(100.0)>
                        <ForEach keys={(0..20).collect::<Vec<i64>>()}>
                            {move |row: i64| {
                                let rows = Rc::clone(&rows);
                                let set_over = set_over.clone();
                                view! {
                                    <ClickCatcher
                                        on_hover_change={move |over: bool| {
                                            set_over.update(|count| match over {
                                                true => *count += 1,
                                                false => *count -= 1,
                                            });
                                        }}
                                    >
                                        <Button on_click={move || rows.set(rows.get() + 1)}>
                                            <Text string={format!("row {row}")} />
                                        </Button>
                                    </ClickCatcher>
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

    harness.frame(vec![Event::PointerMoved(Pos2::new(
        viewport.center().x,
        viewport.top() + 4.0,
    ))]);
    assert_eq!(hovered.get(), 1, "the row under the pointer is hovered");

    harness.frame(vec![Event::PointerMoved(floating.center())]);

    assert_eq!(
        hovered.get(),
        0,
        "moving onto the floating child must take the hover off what it covers"
    );
}
