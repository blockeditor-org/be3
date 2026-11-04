use beui::styled::*;

use super::*;

const KEY_COMPOSE: u32 = 127;

#[component]
fn InMenu(windows: Windows) -> NodeId {
    let ids = create_memo(clone_list(&windows));
    let items = view! {
        <beui::unstyled::MenuItem row_test_id={"test.menu.hide".to_owned()} label="Hide" />
    };
    view! {
        <ContextMenu items>
            <List spacing=Space::None>
                <Button @test_id={"test.button"} label="Button" />
                <ForEach keys={ids}>|id: WindowId|
                    <Frame width={Size::Custom(SHOWN.x)} height={Size::Custom(SHOWN.y)}>
                        <WindowView windows id />
                    </Frame>
                </ForEach>
            </List>
        </ContextMenu>
    }
}

#[test]
fn the_menu_key_reaches_a_focused_window_inside_a_context_menu() {
    let mut harness = Harness::showing(None, |windows| {
        view! {
            <InMenu windows />
        }
    });
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    let menu_open = |harness: &Harness| {
        harness
            .output
            .as_ref()
            .and_then(|output| output.test_id_rect("test.menu.hide"))
            .is_some()
    };

    harness.click(&format!("wayland.window.{}", id.0));
    harness.frame(vec![
        stroke(Some(Key::ContextMenu), KEY_COMPOSE, true),
        stroke(Some(Key::ContextMenu), KEY_COMPOSE, false),
    ]);
    harness.settle();
    assert_eq!(
        harness.client.received.keys,
        [(KEY_COMPOSE, true), (KEY_COMPOSE, false)],
        "the focused window gets the Menu key"
    );
    assert!(
        !menu_open(&harness),
        "the menu around the window leaves the window its key"
    );

    harness.click("test.button");
    harness.frame(vec![
        stroke(Some(Key::ContextMenu), KEY_COMPOSE, true),
        stroke(Some(Key::ContextMenu), KEY_COMPOSE, false),
    ]);
    harness.settle();
    assert!(
        menu_open(&harness),
        "with the focus on a control the same key opens the menu"
    );
    assert_eq!(harness.client.received.keys.len(), 2);
}
