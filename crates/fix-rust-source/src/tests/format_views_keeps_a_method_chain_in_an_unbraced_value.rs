use super::*;

#[test]
fn format_views_keeps_a_method_chain_in_an_unbraced_value() {
    let source = r#"fn build() -> NodeId {
    view! {
        <List spacing=0.0>
            <Frame   @sizing=ItemSize::Percent(100.0).max(300.0) />
        </List>
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! {
        <List spacing=0.0>
            <Frame @sizing=ItemSize::Percent(100.0).max(300.0) />
        </List>
    }
}
"#
    );
}
