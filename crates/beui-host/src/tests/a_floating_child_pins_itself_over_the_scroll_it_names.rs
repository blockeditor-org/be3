use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;
use crate::unstyled::{Edge, Floating, Scroll};

#[test]
fn a_floating_child_pins_itself_over_the_scroll_it_names() {
    let _root = crate::reactive::RootScope::new();
    let pressed = create_signal(0u32);
    let scroll = NodeRef::new();
    let button = NodeRef::new();
    let rows = create_signal(0u32);
    let hovered = create_signal(0usize);
    let document = build_node(move || {
        let over = create_signal(0usize);
        create_effect(move || hovered.set(over.get()));
        view! {
            <List spacing=0.0>
                <Scroll @node_ref=&scroll @sizing=ItemSize::Percent(100.0)>
                    <ForEach keys={(0..20).collect::<Vec<i64>>()}>|row: i64|
                        <Interactive
                            on_hover_change={move |next: bool| {
                                over.update(|count| match next {
                                    true => *count += 1,
                                    false => *count -= 1,
                                });
                            }}
                        >
                            <Button on_click={move || rows.update(|rows| *rows += 1)}>
                                <Text>{format!("row {row}")}</Text>
                            </Button>
                        </Interactive>
                    </ForEach>
                </Scroll>
                <Floating anchor={scroll} edge=Edge::Bottom>
                    <Button
                        @node_ref=&button
                        on_click={move || pressed.update(|pressed| *pressed += 1)}
                    >
                        <Text>"reveal"</Text>
                    </Button>
                </Floating>
            </List>
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

    let before = rows.get_untracked();
    harness.click(floating.center());
    harness.frame(Vec::new());

    assert_eq!(
        pressed.get_untracked(),
        1,
        "the floating child answers the pointer"
    );
    assert_eq!(
        rows.get_untracked(),
        before,
        "a press the floating child took must not reach the rows under it"
    );

    harness.click(Pos2::new(viewport.center().x, viewport.top() + 4.0));
    harness.frame(Vec::new());

    assert_eq!(
        rows.get_untracked(),
        before + 1,
        "the rest of the scroll must go on answering the pointer"
    );

    harness.frame(vec![Input::PointerMoved(Pos2::new(
        viewport.center().x,
        viewport.top() + 4.0,
    ))]);
    assert_eq!(
        hovered.get_untracked(),
        1,
        "the row under the pointer is hovered"
    );

    harness.frame(vec![Input::PointerMoved(floating.center())]);

    assert_eq!(
        hovered.get_untracked(),
        0,
        "moving onto the floating child must take the hover off what it covers"
    );
}
