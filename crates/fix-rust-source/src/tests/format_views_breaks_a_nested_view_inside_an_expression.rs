use super::*;

#[test]
fn format_views_breaks_a_nested_view_inside_an_expression() {
    let source = r#"fn build() -> NodeId {
    view! {
        <Toggle checked={on}>
            {move |handle: ToggleHandle| view! { <SwitchTrack handle label="a very long label goes here" outline=true /> }}
        </Toggle>
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! {
        <Toggle checked={on}>
            {move |handle: ToggleHandle| view! {
                <SwitchTrack handle label="a very long label goes here" outline=true />
            }}
        </Toggle>
    }
}
"#
    );
}
