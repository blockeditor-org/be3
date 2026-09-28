use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::reactive::{Align, Direction, List, NodeRef, Spacer, Text, build, view};

#[test]
fn a_capturing_button_takes_the_press_from_the_row_it_sits_in() {
    let row_clicks = Rc::new(Cell::new(0u32));
    let inner_clicks = Rc::new(Cell::new(0u32));
    let inner = NodeRef::new();
    let row = NodeRef::new();
    let document = build({
        let (row_clicks, inner_clicks, inner, row) = (
            Rc::clone(&row_clicks),
            Rc::clone(&inner_clicks),
            inner.clone(),
            row.clone(),
        );
        move || {
            view! {
                <List spacing=0.0>
                    <unstyled::Button
                        @node_ref=&row
                        on_click={move || row_clicks.set(row_clicks.get() + 1)}
                        content={move |_: unstyled::ButtonHandle| view! {
                            <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                                <Text string="a row that opens something" />
                                <Spacer @sizing=ItemSize::Percent(100.0) />
                                <unstyled::Button
                                    @node_ref=&inner
                                    tab_stop=false
                                    capture_presses=true
                                    on_click={move || {
                                        inner_clicks.set(inner_clicks.get() + 1)
                                    }}
                                    content={move |_: unstyled::ButtonHandle| view! {
                                        <Text string="add" />
                                    }}
                                />
                            </List>
                        }}
                    />
                </List>
            }
        }
    });

    let (inner, row) = (inner.get(), row.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(inner));
    harness.frame(Vec::new());

    assert_eq!(inner_clicks.get(), 1, "the captured button answers");
    assert_eq!(
        row_clicks.get(),
        0,
        "a press the captured button took must not reach the row around it"
    );

    let rect = harness.rect(row);
    harness.click(Pos2::new(rect.left() + 4.0, rect.center().y));
    harness.frame(Vec::new());

    assert_eq!(row_clicks.get(), 1, "the rest of the row still answers");
    assert_eq!(inner_clicks.get(), 1);
}
