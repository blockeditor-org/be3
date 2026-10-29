use super::*;
use crate::primitives::Text;
use crate::styled::*;

#[test]
fn focus_within_a_node_with_nothing_focusable_reports_false() {
    let (document, [label, input]) = toolbar_of(|| {
        [
            view! {
                <Text>"Label"</Text>
            },
            view! {
                <TextInput value="Text" />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    with_installed(harness.document_mut(), |_| {
        assert!(!crate::native::focus_within(label));
        assert!(crate::native::focus_within(input));
    });
}
