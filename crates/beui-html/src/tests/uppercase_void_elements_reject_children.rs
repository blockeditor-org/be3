use super::*;

#[test]
#[should_panic(expected = "void HTML elements cannot have children")]
fn uppercase_void_elements_reject_children() {
    render_fragment(|| {
        view! {
            <Element tag="INPUT">"bad"</Element>
        }
    });
}
