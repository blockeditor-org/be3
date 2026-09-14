use super::*;

#[test]
fn format_views_leaves_a_view_it_cannot_parse_alone() {
    let source = r#"fn build() -> NodeId {
    view! {
        <Column spacing=0.0>
                <Widget />
        </Row>
    }
}
"#;

    assert_eq!(formatted(source), source);
}
