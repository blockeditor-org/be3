use crate::beui_rules::beui_violations;
use ra_ap_syntax::SourceFile;

#[test]
fn view_helpers_require_components_independently_of_return_type() {
    let source = r#"
fn shell_node() -> Node { view! { <Element /> } }
fn group() -> Children<Node> { view! { <Element /><Element /> } }
fn wrapped() -> Result<Node, Error> { Ok(view! { <Element /> }) }
#[component]
fn Shell() -> Node { view! { <Element /> } }
fn page() -> String { beui_html::render(|| view! { <Shell /> }) }
fn snippet() -> String { render_fragment(|| view! { <Shell /> }) }
fn application() -> App { build(|| view! { <Shell /> }) }
fn deferred() -> RenderFn<()> { RenderFn::new(|_| view! { <Shell /> }) }
fn stored() -> ChildView { store(Box::new(|_| view! { <Shell /> })) }
impl Widget for App { fn view() -> Node { view! { <Shell /> } } }
#[component]
impl Widget for Marked { #[component] fn View() -> Node { view! { <Shell /> } } }
fn mapped() -> Vec<Node> { values.map(|_| view! { <Element /> }).collect() }
fn nested() { #[component] fn Good() -> Node { view! { <Shell /> } } }
fn mixed() -> String { let stray = view! { <Element /> }; render(|| view! { <Shell /> }) }
"#;
    let tree = SourceFile::parse(source, ra_ap_syntax::Edition::CURRENT).tree();
    let violations = beui_violations(&tree, source, "crates/html/src/lib.rs");
    assert_eq!(
        violations
            .iter()
            .filter(|v| v.starts_with("view outside component:"))
            .count(),
        6
    );
    assert!(beui_violations(&tree, source, "crates/html/src/tests/views.rs").is_empty());
}
