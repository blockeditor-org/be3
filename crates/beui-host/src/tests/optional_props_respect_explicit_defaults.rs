use super::*;
use crate::reactive::*;

#[component]
fn OptionalLabel(#[prop(default = Some("Hosted"))] label: Option<&'static str>) -> NodeId {
    view! {
        <Text>{label.unwrap_or("None")}</Text>
    }
}

#[test]
fn optional_props_respect_explicit_defaults() {
    let _root = RootScope::new();
    let default = NodeRef::new();
    let none = NodeRef::new();
    let custom = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <OptionalLabel @node_ref=&default />
                <OptionalLabel @node_ref=&none label={None::<&'static str>} />
                <OptionalLabel @node_ref=&custom label={Some("Custom")} />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(text_of(harness.document(), default.get()), "Hosted");
    assert_eq!(text_of(harness.document(), none.get()), "None");
    assert_eq!(text_of(harness.document(), custom.get()), "Custom");
}
