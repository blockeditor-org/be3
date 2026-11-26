use super::*;

#[test]
#[should_panic(expected = "void HTML elements cannot have children")]
fn dynamic_children_cannot_hide_inside_void_elements() {
    render(|| {
        view! {
            <Element tag="input">
                <Show value={move || false}>"hidden"</Show>
            </Element>
        }
    });
}
