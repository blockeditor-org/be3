use super::*;

#[test]
fn raw_text_elements_omit_hydration_markers() {
    let html = render_fragment(|| {
        view! {
            <elements::Title>
                "Page"
                <Show value={move || true}>" one"</Show>
            </elements::Title>
            <elements::Textarea>
                <Show value={move || true}>"draft"</Show>
            </elements::Textarea>
        }
    });
    assert_eq!(html, "<title>Page one</title><textarea>draft</textarea>");
}
