use super::*;

#[test]
fn format_views_keeps_the_macro_body_on_its_own_lines() {
    let source = r#"fn build() -> NodeId {
    view! { <Text string=label /> }
}

fn nested() -> NodeId {
    view! {
        <Tree row={|handle| view! { <Row handle /> }} />
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! {
        <Text string=label />
    }
}

fn nested() -> NodeId {
    view! {
        <Tree
            row={|handle| view! {
                <Row handle />
            }}
        />
    }
}
"#
    );
}
