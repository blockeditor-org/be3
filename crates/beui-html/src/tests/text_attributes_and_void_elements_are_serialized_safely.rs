use super::*;

#[test]
fn text_attributes_and_void_elements_are_serialized_safely() {
    let html = render_fragment(|| {
        view! {
            <Element tag="label" @attr={("data-value", "\"<&'")}>
                "<&\"'>"
                <Element tag="input" @flag={("required", true)} @flag={("disabled", false)} />
            </Element>
        }
    });
    assert_eq!(
        html,
        "<label data-value=\"&quot;&lt;&amp;&#39;\">&lt;&amp;&quot;&#39;&gt;<input required=\"\"></label>"
    );
}
