use super::*;

#[test]
#[should_panic(expected = "closing style tag")]
fn text_inside_a_style_cannot_close_it() {
    render_fragment(|| {
        view! {
            <elements::Style>"</StYlE><p>injected"</elements::Style>
        }
    });
}
