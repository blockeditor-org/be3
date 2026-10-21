use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[test]
fn a_reactive_tree_can_nest_builder_calls_without_threading_the_document() {
    let _root = crate::reactive::RootScope::new();
    let value = create_signal(None);
    let increment = create_signal(None);

    let document = build_node(move || {
        let count = create_signal(0i64);
        let value_node = view! {
            <Text>{create_memo(move || count.get().to_string())}</Text>
        };
        let increment_node = view! {
            <Button
                on_click={move || {
                    count.update(|count| *count += 1)
                }}
            >
                <Text>"+"</Text>
            </Button>
        };
        value.set(Some(value_node));
        increment.set(Some(increment_node));
        view! {
            <List spacing=8.0>
                <List direction=Direction::Horizontal spacing=8.0>
                    {increment_node}
                    {value_node}
                </List>
            </List>
        }
    });

    let value = value.get_untracked().expect("text node was created");
    let increment = increment.get_untracked().expect("button node was created");

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(text_of(harness.document(), value), "0");

    harness.click(harness.center(increment));
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), value), "1");
}
