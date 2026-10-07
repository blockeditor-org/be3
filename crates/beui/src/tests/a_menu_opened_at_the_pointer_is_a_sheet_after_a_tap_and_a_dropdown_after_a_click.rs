use super::*;
use crate::reactive::{Frame, Interactive, view};
use crate::styled::ContextMenu;
use crate::unstyled::MenuItem;

fn sheet_shown(harness: &Harness) -> bool {
    harness
        .document()
        .find_test_id("sheet.handle")
        .and_then(|node| harness.document().node_rect(node))
        .is_some()
}

#[test]
fn a_menu_opened_at_the_pointer_is_a_sheet_after_a_tap_and_a_dropdown_after_a_click() {
    let (menu, target) = (NodeRef::new(), NodeRef::new());
    let document = crate::reactive::build({
        let (menu, target) = (menu.clone(), target.clone());
        move || {
            let (open, set_open) = create_signal(false);
            let closing = set_open.clone();
            view! {
                <List spacing=0.0>
                    <Interactive @node_ref=&target on_click={move || set_open.set(true)}>
                        <Frame width=200.0 height=100.0 />
                    </Interactive>
                    <ContextMenu
                        @node_ref=&menu
                        disabled=true
                        open_at_pointer={open}
                        on_close={move || closing.set(false)}
                        items={view! {
                            <MenuItem label="Settings" />
                        }}
                    >
                        <Frame />
                    </ContextMenu>
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.settle();
    let overlay = unstyled::context_menu_overlay(harness.document(), menu.get());
    let at = harness.center(target.get());

    harness.click(at);
    harness.settle();
    assert!(
        harness.document().is_overlay_open(overlay),
        "a click opens the menu as a dropdown"
    );
    assert!(!sheet_shown(&harness));
    let panel = harness
        .document()
        .overlay_content(kind_of::<crate::base::overlay::OverlayNode>(
            harness.document(),
            overlay,
        ))
        .expect("the open menu has content");
    let panel = harness.rect(panel);
    assert!(
        (panel.left() - at.x).abs() < 0.5 && (panel.top() - at.y).abs() < 0.5,
        "the dropdown opens where the pointer clicked"
    );
    harness.key(Key::Escape, Modifiers::NONE);
    harness.settle();
    assert!(!harness.document().is_overlay_open(overlay));

    harness.touch(TouchPhase::Start, at);
    harness.touch(TouchPhase::End, at);
    harness.settle();
    assert!(sheet_shown(&harness), "a tap opens the menu as a sheet");
    assert!(!harness.document().is_overlay_open(overlay));
}
