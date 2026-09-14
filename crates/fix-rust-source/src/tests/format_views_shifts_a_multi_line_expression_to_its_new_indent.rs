use super::*;

#[test]
fn format_views_shifts_a_multi_line_expression_to_its_new_indent() {
    let source = r#"fn build() -> NodeId {
    view! {
            <Button label="Reset" on_click={move || {
                    reset_count.set(0);
                }}>
            <Text string="reset" />
        </Button>
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! {
        <Button
            label="Reset"
            on_click={move || {
                reset_count.set(0);
            }}
        >
            <Text string="reset" />
        </Button>
    }
}
"#
    );
}
