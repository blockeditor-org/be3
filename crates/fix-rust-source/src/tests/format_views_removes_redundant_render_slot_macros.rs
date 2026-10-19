use super::*;

#[test]
fn format_views_removes_redundant_render_slot_macros() {
    let source = r#"fn example() {
    view! {
        <ForEach keys={keys}>|key: Key| { view! { <Row key /><Row key /> } }</ForEach>
        <Show condition={shown}>|| { view! { <Row /> } }</Show>
        <ForEach keys={keys}>|key: Key| { let value = key.name(); view! { <Row value /> } }</ForEach>
    }
}
"#;
    let output = formatted(source);
    assert!(output.contains("<ForEach keys={keys}>|key: Key|\n            <Row key />\n            <Row key />\n        </ForEach>"));
    assert!(output.contains("<Show condition={shown}>||\n            <Row />\n        </Show>"));
    assert!(output.contains("let value = key.name();"));
    assert_eq!(output.matches("view!").count(), 2);
    assert_eq!(formatted(&output), output);
}
