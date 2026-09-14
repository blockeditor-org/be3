use super::*;

#[test]
fn format_views_breaks_a_tag_that_does_not_fit() {
    let source = r#"fn build() -> NodeId {
    view! {
        <Frame color={surface} outline={border} outline_width=BORDER_WIDTH radius=CARD_RADIUS padding=PADDING>
            {children}
        </Frame>
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! {
        <Frame
            color={surface}
            outline={border}
            outline_width=BORDER_WIDTH
            radius=CARD_RADIUS
            padding=PADDING
        >
            {children}
        </Frame>
    }
}
"#
    );
}
