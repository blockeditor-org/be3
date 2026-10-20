use super::*;
use crate::tw;

#[test]
#[should_panic(expected = "typed CSS requires")]
fn typed_styles_require_an_outlet() {
    render_fragment(|| {
        view! {
            <elements::Div css=<tw::Flex /> />
        }
    });
}
