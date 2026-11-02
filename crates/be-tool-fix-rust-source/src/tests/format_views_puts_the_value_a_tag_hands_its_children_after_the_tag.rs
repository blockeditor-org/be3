use super::*;

#[test]
fn format_views_puts_the_value_a_tag_hands_its_children_after_the_tag() {
    let source = r#"fn build() -> NodeId {
    view! {
        <ForEach keys={keys}>
            |key:   Uuid| <Row key /> <Row key />
        </ForEach>
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! {
        <ForEach keys={keys}>|key: Uuid|
            <Row key />
            <Row key />
        </ForEach>
    }
}
"#
    );
}
