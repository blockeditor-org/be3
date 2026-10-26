use beui::dock::styled::Docking;
use beui::styled::*;
use beui::dock::unstyled::{DockPane, DockTab, DockingLayout};

use super::*;

const KEY_A: u32 = 30;
const KEY_B: u32 = 48;
const KEY_C: u32 = 46;

thread_local! {
    static LAYOUT: std::cell::Cell<Option<DockingLayout<u64>>> = const { std::cell::Cell::new(None) };
}

#[component]
fn Tabbed(windows: Windows) -> NodeId {
    let ids = create_memo(clone_list(&windows));
    let layout = DockingLayout::new();
    LAYOUT.with(|held| held.set(Some(layout)));
    view! {
        <Docking layout>
            <DockPane id="tabs">
                <DockTab
                    id=1u64
                    title="Program"
                    content={move || view! {
                        <Layers>
                            <ForEach keys={ids}>|id: WindowId|
                                <WindowView windows id />
                            </ForEach>
                        </Layers>
                    }}
                />
                <DockTab
                    id=2u64
                    title="Text"
                    content={|| view! {
                        <TextInput @test_id={"test.input"} value="" />
                    }}
                />
            </DockPane>
        </Docking>
    }
}

fn show(harness: &mut Harness, tab: u64) {
    let layout = LAYOUT
        .with(std::cell::Cell::get)
        .expect("the dock was built");
    layout.show(&tab);
    harness.settle();
}

#[test]
fn hiding_the_tab_of_the_window_with_the_keyboard_takes_the_keyboard_from_it() {
    let mut harness = Harness::showing(None, |windows| {
        view! {
            <Tabbed windows />
        }
    });
    let _keyboard = harness.client.keyboard();
    let _pointer = harness.client.pointer();
    let (_window, id) = harness.open();
    harness.click(&format!("wayland.window.{}", id.0));
    assert!(harness.client.received.keyboard_entered);
    harness.key(KEY_A);

    show(&mut harness, 2);
    assert_eq!(
        harness.compositor().windows().focused(),
        None,
        "the window whose tab was hidden no longer has the focus"
    );
    assert!(
        !harness.client.received.keyboard_entered,
        "the program whose tab was hidden loses the keyboard"
    );
    harness.key(KEY_B);

    show(&mut harness, 1);
    assert_eq!(
        harness.compositor().windows().focused(),
        Some(id),
        "showing the window's tab again gives it the keyboard back"
    );
    harness.key(KEY_C);
    assert_eq!(
        harness.client.received.keys,
        [(KEY_A, true), (KEY_A, false), (KEY_C, true), (KEY_C, false)],
        "no key reaches the program while its tab is hidden"
    );
}
