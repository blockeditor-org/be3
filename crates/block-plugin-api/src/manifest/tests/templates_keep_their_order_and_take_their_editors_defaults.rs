use super::*;

#[test]
fn templates_keep_their_order_and_take_their_editors_defaults() {
    let manifest = manifest_from_json(DOCUMENT).expect("the document is valid");
    let editor = &manifest.editors[0];
    let ids: Vec<_> = editor
        .templates
        .iter()
        .map(|template| template.id.as_str())
        .collect();
    assert_eq!(ids, ["main", "zero", "another"]);
    let main = editor.template("main").expect("main is declared");
    assert_eq!(main.name, "Counter");
    assert_eq!(main.icon, "\u{eb8d}");
    assert_eq!(main.category, TemplateCategory::Debug);
    assert_eq!(main.block_type, editor.block_type);
    assert!(!main.dialog);
    let zero = editor.template("zero").expect("zero is declared");
    assert_eq!(zero.name, "Zero");
    assert_eq!(zero.icon, "\u{e3c6}");
    assert_eq!(zero.category, TemplateCategory::Template);
    let another = editor.template("another").expect("another is declared");
    assert!(another.dialog);
    assert_eq!(
        another.block_type,
        Uuid::parse_str("00007072-6573-656e-7461-74696f6e0001")
            .expect("a uuid")
            .into_bytes()
    );
}
