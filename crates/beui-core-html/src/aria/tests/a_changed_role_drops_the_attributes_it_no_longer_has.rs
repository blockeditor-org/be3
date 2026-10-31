use super::*;

#[test]
fn a_changed_role_drops_the_attributes_it_no_longer_has() {
    let made = create("Test", "div");
    set_accessibility(
        made,
        with(Role::CheckBox, |node| {
            node.set_label("Done");
            node.set_toggled(Toggled::Mixed);
        }),
        1,
    );
    set_accessibility(made, accesskit::Node::new(Role::Label), 1);
    assert_eq!(element(made).expect("made above").to_html(), "<div></div>");
}
