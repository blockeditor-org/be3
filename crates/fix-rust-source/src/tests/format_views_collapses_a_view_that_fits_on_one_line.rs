use super::*;

#[test]
fn format_views_collapses_a_view_that_fits_on_one_line() {
    let source = r#"fn build() -> NodeId {
    view! {
        <Column spacing=0.0>
                <Widget />
        </Column>
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! { <Column spacing=0.0><Widget /></Column> }
}
"#
    );
}
