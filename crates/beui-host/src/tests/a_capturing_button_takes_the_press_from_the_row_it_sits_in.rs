use super::*;
use crate::primitives::Spacer;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_capturing_button_takes_the_press_from_the_row_it_sits_in() {
    let _root = crate::reactive::RootScope::new();
    let row_clicks = create_signal(0u32);
    let inner_clicks = create_signal(0u32);
    let inner = NodeRef::new();
    let row = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <unstyled::Button
                    @node_ref=&row
                    on_click={move || row_clicks.update(|row_clicks| *row_clicks += 1)}
                >|_: unstyled::ButtonHandle|
                    <List direction=Direction::Horizontal align=Align::Center spacing=0.0>
                        <Text>"a row that opens something"</Text>
                        <Spacer @sizing=ItemSize::Percent(100.0) />
                        <unstyled::Button
                            @node_ref=&inner
                            tab_stop=false
                            capture_presses=true
                            on_click={move || {
                                inner_clicks.update(|inner_clicks| *inner_clicks += 1)
                            }}
                            content={move |_: unstyled::ButtonHandle| view! {
                                <Text>"add"</Text>
                            }}
                        />
                    </List>
                </unstyled::Button>
            </List>
        }
    });

    let (inner, row) = (inner.get(), row.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(inner));
    harness.frame(Vec::new());

    assert_eq!(
        inner_clicks.get_untracked(),
        1,
        "the captured button answers"
    );
    assert_eq!(
        row_clicks.get_untracked(),
        0,
        "a press the captured button took must not reach the row around it"
    );

    let rect = harness.rect(row);
    harness.click(Pos2::new(rect.left() + 4.0, rect.center().y));
    harness.frame(Vec::new());

    assert_eq!(
        row_clicks.get_untracked(),
        1,
        "the rest of the row still answers"
    );
    assert_eq!(inner_clicks.get_untracked(), 1);
}
