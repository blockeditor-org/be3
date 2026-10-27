use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::Text;
use crate::styled::*;

#[test]
fn a_style_provider_sizes_and_colours_the_text_inside_it() {
    let _root = crate::reactive::RootScope::new();
    let (inherited, overridden, outside) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=4.0>
                <TextStyleProvider
                    style={TextStyle::new().size(TextSize::Sm).color(TextColor::Muted)}
                >
                    <List spacing=4.0>
                        <Text @node_ref=&inherited>"Inherited"</Text>
                        <Text @node_ref=&overridden color=TextColor::Danger>"Danger"</Text>
                    </List>
                </TextStyleProvider>
                <Text @node_ref=&outside>"Outside"</Text>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let property = |node: NodeRef, name: &str| {
        harness
            .document()
            .node_properties(node.get())
            .into_iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
            .expect("a text reports the property")
    };

    assert_eq!(property(inherited, "font size"), "12");
    assert_eq!(
        property(inherited, "color"),
        "--color-text_muted"
    );
    assert_eq!(property(overridden, "font size"), "12");
    assert_eq!(
        property(overridden, "color"),
        "--color-danger"
    );
    assert_eq!(property(outside, "font size"), "14");
    assert_eq!(
        property(outside, "color"),
        "--color-text"
    );
}
