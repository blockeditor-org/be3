use super::*;
use crate::FontId;
use crate::reactive::*;

const PUNNED: &str = "Punned";

#[test]
fn view_attributes_can_pun_a_bare_name_as_its_own_value() {
    let _root = crate::reactive::RootScope::new();
    let column = NodeRef::new();
    let label = NodeRef::new();
    let document = build_node(move || {
        let string = PUNNED.to_string();
        let font_size = 18.0f32;
        let spacing = 0.0f32;
        let children = view! {
            <Text @node_ref=&label font={FontId::proportional(font_size)}>{string}</Text>
        };
        view! {
            <List @node_ref=&column spacing children />
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), label.get()), PUNNED);
    assert_eq!(harness.document().children(column.get()), vec![label.get()]);
}
