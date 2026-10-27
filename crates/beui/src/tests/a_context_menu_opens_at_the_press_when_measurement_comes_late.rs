use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::ContextMenu;
use crate::unstyled::MenuItem;

#[test]
fn a_context_menu_opens_at_the_press_when_measurement_comes_late() {
    let (prompt, late) = prompt_and_late(|| {
        let _root = crate::reactive::RootScope::new();
        let document = build_node(move || {
            view! {
                <ContextMenu
                    items={view! {
                        <MenuItem label="Copy" />
                        <MenuItem label="Paste" />
                    }}
                >
                    <Frame width=300.0 height=300.0 />
                </ContextMenu>
            }
        });
        let mut harness = Harness::new(document);
        harness.settle();
        harness.click_with(pos2(120.0, 90.0), PointerButton::Secondary);
        harness.settle();
        text_rects(&harness)
    });
    assert!(
        prompt
            .iter()
            .any(|(text, rect)| text == "Copy" && rect.is_some()),
        "the menu opened: {prompt:?}"
    );
    assert_eq!(
        late, prompt,
        "late measurement settles where prompt measurement does"
    );
}
