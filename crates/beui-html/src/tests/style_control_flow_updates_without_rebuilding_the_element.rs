use super::*;
use crate::tw;

#[test]
fn style_control_flow_updates_without_rebuilding_the_element() {
    let _root = RootScope::new();
    let scope = Scope::new();
    let (shown, set_shown) = create_signal(true);
    let (items, set_items) = create_signal(vec![1, 2]);
    let node = scope.run(|| {
        view! {
            <elements::Div
                css=[
                    <tw::Flex />
                    <Show condition={shown}>
                        <tw::Hidden />
                    </Show>
                    <ForEach keys={items}>|_item: i32|
                        <tw::Underline />
                    </ForEach>
                ]
            >
                <tw::Styles />
                "Kept"
            </elements::Div>
        }
    });
    let before = node.to_html();
    assert!(before.contains("display:none;"));
    assert_eq!(before.matches("text-decoration:underline;").count(), 2);
    set_shown.set(false);
    set_items.set(vec![2]);
    let after = node.to_html();
    assert!(!after.contains("display:none;"));
    assert!(after.contains("display:flex;"));
    assert_eq!(after.matches("text-decoration:underline;").count(), 1);
    assert!(after.ends_with("Kept</div>"));
}
