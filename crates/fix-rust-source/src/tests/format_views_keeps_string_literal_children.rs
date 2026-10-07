use super::*;

#[test]
fn format_views_keeps_string_literal_children() {
    let source = r#"fn build() -> NodeId {
    view! {
        <Text>
            "Spans give "
            <Span color={accent}>
                "color"
            </Span>
        </Text>
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! {
        <Text>
            "Spans give "
            <Span color={accent}>"color"</Span>
        </Text>
    }
}
"#
    );
}
