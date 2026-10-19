use super::*;

#[test]
fn typed_elements_bind_optional_numeric_and_boolean_attributes() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let (name, set_name) = create_signal(Some("<&\"".to_string()));
    let (disabled, set_disabled) = create_signal(true);
    let (length, set_length) = create_signal(Some(200_u32));
    let node = scope.run(|| {
        view! {
            <elements::Input
                r#type="text"
                id="entry"
                name={name}
                maxlength={length}
                disabled={disabled}
                @data={("test-id", "field")}
                @aria={("invalid", "false")}
            />
        }
    });
    assert!(node.to_html().contains("name=\"&lt;&amp;&quot;\""));
    assert!(node.to_html().contains("maxlength=\"200\""));
    assert!(node.to_html().contains("disabled=\"\""));
    assert!(
        node.to_html()
            .contains("data-test-id=\"field\" aria-invalid=\"false\"")
    );
    set_name.set(None);
    set_length.set(None);
    set_disabled.set(false);
    assert_eq!(
        node.to_html(),
        "<input id=\"entry\" type=\"text\" data-test-id=\"field\" aria-invalid=\"false\">"
    );
    set_name.set(Some("restored".to_string()));
    set_length.set(Some(14));
    set_disabled.set(true);
    assert!(
        node.to_html()
            .contains("name=\"restored\" maxlength=\"14\" disabled=\"\"")
    );
}
