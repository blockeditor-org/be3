use super::*;

#[test]
fn format_views_breaks_a_block_after_a_value_that_does_not_fit() {
    let source = r#"fn build() -> NodeId {
    view! {
        <Container>
            || {
                    let narrow = narrower_than(BREAKPOINT);
                    view! { <Stack narrow /> }
            }
        </Container>
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! {
        <Container>|| {
            let narrow = narrower_than(BREAKPOINT);
            view! {
                <Stack narrow />
            }
        }</Container>
    }
}
"#
    );
}
