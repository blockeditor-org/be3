use beui::dock::styled::Docking;
use beui::dock::styled::{DockPane, DockTab, DockWindow, DockingLayout};
use beui::styled::*;

use super::*;

const KEY_A: u32 = 30;
const KEY_B: u32 = 48;

#[component]
fn Floating(windows: Windows) -> NodeId {
    let ids = create_memo(clone_list(&windows));
    let layout = DockingLayout::new();
    let window = Rect::from_min_size(pos2(450.0, 100.0), Vec2::new(450.0, 400.0));
    view! {
        <Docking layout>
            <DockPane id="main">
                <DockTab
                    id=1u64
                    title="Text"
                    content={|| view! {
                        <TextInput @test_id={"test.input"} value="" />
                    }}
                />
            </DockPane>
            <DockWindow id="window" rect={window}>
                <DockPane id="program">
                    <DockTab
                        id=2u64
                        title="Program"
                        content={move || view! {
                            <Layers>
                                <ForEach keys={ids}>|id: WindowId|
                                    <WindowView windows id />
                                </ForEach>
                            </Layers>
                        }}
                    />
                </DockPane>
            </DockWindow>
        </Docking>
    }
}

#[test]
fn pressing_a_windows_bar_gives_it_the_keyboard() {
    let mut harness = Harness::showing(None, |windows| {
        view! {
            <Floating windows />
        }
    });
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    assert!(
        harness.client.received.keyboard_entered,
        "a window gets the keyboard when it is first shown"
    );

    let input = harness.rect("test.input");
    harness.click_at(input.min + Vec2::new(40.0, 40.0));
    assert!(!harness.client.received.keyboard_entered);
    let shown = harness.window(id);
    let bar = pos2(shown.center().x + 100.0, shown.min.y - 12.0);
    let moved = bar + Vec2::new(-40.0, 30.0);
    harness.frame(vec![
        Input::PointerMoved(bar),
        Input::PointerButton {
            pos: bar,
            button: PointerButton::Primary,
            pressed: true,
        },
    ]);
    harness.frame(vec![Input::PointerMoved(moved)]);
    harness.frame(vec![Input::PointerButton {
        pos: moved,
        button: PointerButton::Primary,
        pressed: false,
    }]);
    harness.settle();
    assert_ne!(harness.window(id), shown, "the bar moved the window");
    assert!(
        harness.client.received.keyboard_entered,
        "pressing the window's bar gives the program the keyboard"
    );
    harness.key(KEY_A);

    let shown = harness.window(id);
    harness.click_at(pos2(shown.center().x + 100.0, shown.min.y - 12.0));
    assert!(
        harness.client.received.keyboard_entered,
        "pressing the bar of the window with the keyboard leaves it there"
    );
    harness.key(KEY_B);
    assert_eq!(
        harness.client.received.keys,
        [(KEY_A, true), (KEY_A, false), (KEY_B, true), (KEY_B, false)],
        "the program gets the keys typed after its window was moved"
    );
}
