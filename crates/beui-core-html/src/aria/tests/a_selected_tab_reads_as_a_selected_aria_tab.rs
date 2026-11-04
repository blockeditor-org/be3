use super::*;

#[test]
fn a_selected_tab_reads_as_a_selected_aria_tab() {
    let shown = html(
        "div",
        with(Role::Tab, |node| {
            node.set_selected(true);
            node.set_expanded(false);
        }),
    );
    assert_eq!(
        shown,
        "<div role=\"tab\" aria-selected=\"true\" aria-expanded=\"false\"></div>"
    );
}
