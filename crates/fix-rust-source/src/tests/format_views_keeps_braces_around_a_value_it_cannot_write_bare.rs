use super::*;

#[test]
fn format_views_keeps_braces_around_a_value_it_cannot_write_bare() {
    let source = r#"fn build() -> NodeId {
    view! {
        <Frame width={a + b} style={Style { pad: 1 }} name={item.name} on_click={move || go()} />
        <Text string={format!("{a}")} unit={()} cast={size as f32} />
    }
}
"#;

    assert_eq!(formatted(source), source);
}
