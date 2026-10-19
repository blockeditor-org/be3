use super::*;

#[test]
#[should_panic(expected = "void HTML elements cannot have children")]
fn void_elements_reject_children() {
    render_fragment(|| {
        view! {
            <Element tag="input">"bad"</Element>
        }
    });
}
