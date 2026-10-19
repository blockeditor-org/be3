use super::*;

#[test]
fn boolean_attributes_follow_signals() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let (disabled, set_disabled) = create_signal(true);
    let node = scope.run(|| {
        view! {
            <Element tag="input" @attr={("name", "email")} @flag={("disabled", disabled)} />
        }
    });
    assert_eq!(node.to_html(), "<input name=\"email\" disabled=\"\">");
    set_disabled.set(false);
    assert_eq!(node.to_html(), "<input name=\"email\">");
    set_disabled.set(true);
    assert_eq!(node.to_html(), "<input name=\"email\" disabled=\"\">");
}
