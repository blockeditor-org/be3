use super::*;

#[test]
fn text_attributes_and_void_elements_are_serialized_safely() {
    let html = Node::element("label")
        .attr("data-value", "\"<&'")
        .child("<&\"'>")
        .child(
            Node::element("input")
                .flag("required", true)
                .flag("disabled", false),
        );
    assert_eq!(
        html.to_html(),
        "<label data-value=\"&quot;&lt;&amp;&#39;\">&lt;&amp;&quot;&#39;&gt;<input required=\"\"></label>"
    );
}
