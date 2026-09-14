use super::*;

#[test]
fn format_views_always_breaks_children_onto_their_own_lines() {
    let source = r#"fn build() -> NodeId {
    view! {
        <Column spacing=0.0><Widget /><Frame visible={open}>{children}</Frame><Row></Row></Column>
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! {
        <Column spacing=0.0>
            <Widget />
            <Frame visible={open}>
                {children}
            </Frame>
            <Row></Row>
        </Column>
    }
}
"#
    );
}
