use super::*;

#[test]
fn format_views_removes_redundant_render_slot_macros() {
    let source = r#"fn example() {
    view! {
        <ForEach value={keys}>|key: Key| { view! { <Row key /><Row key /> } }</ForEach>
        <Show value={shown}>|| { view! { <Row /> } }</Show>
        <ForEach value={keys}>|key: Key| { let value = key.name(); view! { <Row value /> } }</ForEach>
    }
}
"#;
    let output = formatted(source);
    assert!(output.contains("<ForEach value={keys}>|key: Key|\n            <Row key />\n            <Row key />\n        </ForEach>"));
    assert!(output.contains("<Show value={shown}>||\n            <Row />\n        </Show>"));
    assert!(output.contains("let value = key.name();"));
    assert_eq!(output.matches("view!").count(), 2);
    assert_eq!(formatted(&output), output);
}
