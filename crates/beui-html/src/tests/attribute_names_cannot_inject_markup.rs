use super::*;

#[test]
#[should_panic(expected = "invalid HTML attribute name")]
fn attribute_names_cannot_inject_markup() {
    render_fragment(|| {
        view! {
            <Element tag="div" @attr={("x onclick=", "bad")} />
        }
    });
}
