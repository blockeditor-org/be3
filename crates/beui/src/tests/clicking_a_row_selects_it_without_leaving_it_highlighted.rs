use super::*;

#[test]
fn clicking_a_row_selects_it_without_leaving_it_highlighted() {
    let HelloColumn { document, text, .. } = hello_column();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.click(harness.row_center(2));
    harness.frame(vec![Event::PointerMoved(pos2(4.0, 4.0))]);

    assert_eq!(harness.inspector().state.selected.get(), Some(text));
    assert_eq!(harness.inspector().highlighted(), None);
}
