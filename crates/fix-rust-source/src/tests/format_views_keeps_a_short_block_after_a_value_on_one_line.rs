use super::*;

#[test]
fn format_views_keeps_a_short_block_after_a_value_on_one_line() {
    let source = r#"fn build() -> NodeId {
    view! {
        <Dynamic value={shape}>
            |shape| {
                render(shape)
            }
        </Dynamic>
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! {
        <Dynamic value={shape}>|shape| { render(shape) }</Dynamic>
    }
}
"#
    );
}
