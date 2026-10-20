use super::*;

#[test]
fn rendering_flushes_bindings_before_serializing_and_disposes_scope() {
    let cleaned = Rc::new(std::cell::Cell::new(false));
    let cleanup = cleaned.clone();
    let html = render(|| {
        on_cleanup(move || cleanup.set(true));
        view! {
            <Element tag="p" @attr={("class", "message")}>
                <Text value="ready" />
            </Element>
        }
    });
    assert_eq!(html, "<!doctype html><p class=\"message\">ready</p>");
    assert!(cleaned.get());
}
