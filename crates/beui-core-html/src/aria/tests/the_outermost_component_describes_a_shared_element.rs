use super::*;

#[test]
fn the_outermost_component_describes_a_shared_element() {
    let made = create("Test", "div");
    let named = |label: &str| with(Role::Group, |node| node.set_label(label));
    set_accessibility(made, named("Inner"), 1);
    set_accessibility(made, named("Outer"), 2);
    set_accessibility(made, named("Inner again"), 1);
    assert_eq!(
        element(made).expect("made above").to_html(),
        r#"<div role="group" aria-label="Outer"></div>"#
    );
}
