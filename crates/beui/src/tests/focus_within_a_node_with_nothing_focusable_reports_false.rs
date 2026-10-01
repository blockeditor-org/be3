use super::*;
use crate::reactive::view;
use crate::styled::TextInput;

#[test]
fn focus_within_a_node_with_nothing_focusable_reports_false() {
    let (document, [label, input]) = toolbar_of(|| {
        [
            view! {
                <Text string="Label" />
            },
            view! {
                <TextInput value="Text" />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    with_installed(harness.document_mut(), |_| {
        assert!(!crate::focus_within(label));
        assert!(crate::focus_within(input));
    });
}
