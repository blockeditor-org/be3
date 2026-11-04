use super::*;

#[test]
fn a_native_element_keeps_its_own_semantics() {
    let field = with(Role::TextInput, |node| node.set_label("Email"));
    assert_eq!(html("input", field.clone()), "<input aria-label=\"Email\">");
    assert_eq!(
        html("div", field),
        "<div role=\"textbox\" aria-label=\"Email\"></div>"
    );
    assert_eq!(
        html("dialog", accesskit::Node::new(Role::Dialog)),
        "<dialog></dialog>"
    );
}
