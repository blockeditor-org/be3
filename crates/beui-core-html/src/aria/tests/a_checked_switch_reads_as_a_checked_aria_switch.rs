use super::*;

#[test]
fn a_checked_switch_reads_as_a_checked_aria_switch() {
    let shown = html(
        "div",
        with(Role::Switch, |node| {
            node.set_label("Wi-Fi");
            node.set_toggled(Toggled::True);
            node.set_disabled();
        }),
    );
    assert_eq!(
        shown,
        "<div role=\"switch\" aria-label=\"Wi-Fi\" aria-checked=\"true\" aria-disabled=\"true\"></div>"
    );
}
