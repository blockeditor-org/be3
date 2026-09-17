use super::*;

#[test]
fn format_views_collapses_an_element_whose_children_are_expressions() {
    let source = r#"fn build() -> NodeId {
    view! {
        <Frame visible=open>
            {children}
        </Frame>
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! {
        <Frame visible=open>{children}</Frame>
    }
}
"#
    );
}
