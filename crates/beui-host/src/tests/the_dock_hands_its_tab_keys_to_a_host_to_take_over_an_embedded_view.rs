use super::*;
use crate::dock::{DockPane, DockTab, DockingLayout};
use crate::dock_host::styled::DockTabKeys;
use crate::dock_host::unstyled::DockApi;
use crate::reactive::*;

#[test]
fn the_dock_hands_its_tab_keys_to_a_host_to_take_over_an_embedded_view() {
    let _root = crate::reactive::RootScope::new();
    let dock = NodeRef::new();
    let keys = create_signal(DockTabKeys::CTRL);
    let document = build_node(move || {
        let layout = DockingLayout::<u64>::new();
        view! {
            <dock_host::styled::DockHost tab_keys={keys} @node_ref=&dock>
                <dock::Docking layout>
                    <DockPane id="tabs">
                        <ForEach value={vec![1u64, 2, 3]}>|id: u64|
                            <DockTab
                                id
                                title={format!("Tab {id}")}
                                content={move || {
                                    let typed = create_signal(String::new());
                                    view! {
                                        <styled::TextInput
                                            @test_id={format!("input.{id}")}
                                            value={typed}
                                            label="Notes"
                                        />
                                    }
                                }}
                            />
                        </ForEach>
                    </DockPane>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let dock = dock.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let input = harness.find("input.1");
    harness.click(harness.center(input));
    let intercepted = harness
        .frame(Vec::new())
        .main()
        .window
        .requests
        .intercepted_keys
        .clone();
    let ctrl = DockTabKeys::CTRL;
    assert!(
        intercepted.contains(&ctrl.next.key_chord())
            && intercepted.contains(&ctrl.previous.key_chord()),
        "a host is asked to take Ctrl+Tab and Ctrl+Shift+Tab ahead of an embedded view: {intercepted:?}"
    );

    let active = |harness: &Harness| {
        let state = harness.document().api::<DockApi>(dock).state();
        let leaf = state.leaves(state.main())[0];
        state.active_tab(leaf)
    };
    let before = active(&harness);
    let tab = |pressed| Input::InterceptedKey {
        key: Key::Tab,
        pressed,
        repeat: false,
    };
    harness.frame(vec![
        Input::Modifiers(Modifiers::CTRL),
        tab(true),
        tab(false),
        Input::Modifiers(Modifiers::NONE),
    ]);
    assert_ne!(
        active(&harness),
        before,
        "Ctrl+Tab the host took for the dock shows the next tab"
    );

    keys.set(DockTabKeys::SUPER);
    let intercepted = harness
        .frame(Vec::new())
        .main()
        .window
        .requests
        .intercepted_keys
        .clone();
    assert!(
        intercepted.contains(&DockTabKeys::SUPER.next.key_chord())
            && !intercepted.contains(&ctrl.next.key_chord()),
        "bound to Super+Tab, the dock leaves Ctrl+Tab to the view: {intercepted:?}"
    );
    let shown = active(&harness);
    harness.key(Key::Tab, Modifiers::CTRL);
    harness.frame(Vec::new());
    assert_eq!(active(&harness), shown, "Ctrl+Tab no longer walks the tabs");
    harness.key(Key::Tab, Modifiers::LOGO);
    harness.frame(Vec::new());
    assert_ne!(active(&harness), shown, "Super+Tab does");
    let shown = active(&harness);
    assert!(
        run_dock_action(&mut harness, "dock.tab.previous"),
        "the palette offers Previous tab"
    );
    harness.frame(Vec::new());
    assert_ne!(active(&harness), shown, "and running it walks back");
}
