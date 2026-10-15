use super::*;

#[test]
fn a_checkout_waits_for_its_version_status() {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut editor = BeuiTest::<CheckoutApp>::new(OwnedEditor::new(host, Uuid::new_v4()));
    editor.hold(None, CheckoutContent::new(&Checkout::default()));
    editor.run();

    assert!(editor.shown("editor.loading"));

    editor.set_version_status(VersionStatus::default());
    editor.run();

    assert!(!editor.shown("editor.loading"));
}
