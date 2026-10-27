use super::*;

#[test]
fn text_inside_styles_and_scripts_is_not_entity_escaped() {
    let html = render_fragment(|| {
        view! {
            <elements::Style>"p > b::after { content: \"&\" }"</elements::Style>
            <elements::Script>"if (1 < 2 && 'a') {}"</elements::Script>
            <elements::Title>"a & b"</elements::Title>
        }
    });
    assert_eq!(
        html,
        "<style>p > b::after { content: \"&\" }</style><script>if (1 < 2 && 'a') {}</script><title>a &amp; b</title>"
    );
}
