use super::*;

#[test]
fn format_views_preserves_a_multi_line_string_literal() {
    let source = "fn build() -> NodeId {\n    view! {\n        <Paragraph content=\"first line \\\n             second line\" align=TextAlign::Start color={muted} wrap=true />\n    }\n}\n";

    assert_eq!(
        formatted(source),
        "fn build() -> NodeId {\n    view! {\n        <Paragraph\n            content=\"first line \\\n             second line\"\n            align=TextAlign::Start\n            color={muted}\n            wrap=true\n        />\n    }\n}\n"
    );
}
