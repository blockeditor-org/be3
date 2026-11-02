use crate::beui_rules::beui_violations;
use ra_ap_syntax::SourceFile;

#[test]
fn a_component_method_needs_a_component_block() {
    let source = r#"
#[component]
trait Marked { #[component] fn View(editor: Editor) -> Node; }
trait Unmarked { #[component] fn View(editor: Editor) -> Node; }
#[component]
impl Marked for App { #[component] fn View(editor: Editor) -> Node { view! { <Shell /> } } }
impl Unmarked for App { #[component] fn View(editor: Editor) -> Node { view! { <Shell /> } } }
impl App { #[component] fn Row() -> Node { view! { <Shell /> } } }
"#;
    let tree = SourceFile::parse(source, ra_ap_syntax::Edition::CURRENT).tree();
    let violations = beui_violations(&tree, source, "crates/html/src/lib.rs");
    let lines: Vec<&str> = violations
        .iter()
        .filter(|v| v.starts_with("component method outside a component block:"))
        .map(|v| v.split(' ').nth(6).unwrap_or_default())
        .collect();
    assert_eq!(
        lines,
        [
            "crates/html/src/lib.rs:4",
            "crates/html/src/lib.rs:7",
            "crates/html/src/lib.rs:8"
        ]
    );
}
