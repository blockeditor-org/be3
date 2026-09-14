use super::*;

#[test]
fn format_views_collapses_a_view_that_fits_on_one_line() {
    let source = r#"fn build() -> NodeId {
    view! {
        <Text
                string={label}
            font_size=FONT_BODY
        />
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! { <Text string={label} font_size=FONT_BODY /> }
}
"#
    );
}
