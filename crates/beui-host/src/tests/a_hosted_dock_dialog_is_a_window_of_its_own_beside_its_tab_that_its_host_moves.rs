use super::*;
use crate::dock::{DockDialog, DockPane, DockTab};
use crate::{WindowId, WindowPlacement, WindowRequest};

#[test]
fn a_hosted_dock_dialog_is_a_window_of_its_own_beside_its_tab_that_its_host_moves() {
    let _root = crate::reactive::RootScope::new();
    let pressed = create_signal(0_u32);
    let document = build_node(move || {
        view! {
            <dock_host::styled::DockHost>
                <dock::Docking>
                    <DockPane id="main">
                        <DockTab id=1u64 title="Page">
                            <List spacing=0.0>
                                <Button @test_id="page.button" label="Under the dialog" />
                                <ListItem grow=1.0>
                                    <Interactive
                                        @test_id="page"
                                        on_press={move |_: PointerPress| pressed.update(|n| *n += 1)}
                                    >
                                        <Frame />
                                    </Interactive>
                                </ListItem>
                            </List>
                        </DockTab>
                    </DockPane>
                    <DockDialog
                        id=2u64
                        parent=1u64
                        title="Dialog"
                        modal=true
                        size={vec2(240.0, 80.0)}
                        on_close={|| {}}
                    >
                        <Button @test_id="dialog.button" label="Inside" />
                    </DockDialog>
                </dock::Docking>
            </dock_host::styled::DockHost>
        }
    });
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    for _ in 0..4 {
        harness.frame(Vec::new());
    }
    let root = harness.document().root().expect("the dock built a root");
    let tab = harness.rect(dock_tab(harness.document(), root, "Page"));
    let opened = harness.take_window_requests();
    let (window, at) = opened
        .iter()
        .find_map(|request| match request {
            WindowRequest::Open {
                window,
                placement: WindowPlacement::Over { window: over, at },
                ..
            } if *over == WindowId::MAIN => Some((*window, *at)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("the dialog opens a window of its own: {opened:?}"));
    assert!(
        at.y >= tab.bottom(),
        "below the tab it hangs from: {at:?} under {tab:?}"
    );

    let page = harness.rect(harness.find("page")).center();
    harness.click(page);
    harness.frame(Vec::new());
    assert_eq!(
        pressed.get_untracked(),
        0,
        "a modal dialog in a window of its own blocks its tab's panel in the main window"
    );

    let under = harness.find("page.button");
    for _ in 0..4 {
        harness.key(Key::Tab, Modifiers::NONE);
        harness.frame(Vec::new());
        assert_ne!(
            harness.document().focused,
            Some(under),
            "Tab never takes the focus back to the blocked panel in the main window"
        );
    }

    let header = harness.rect(harness.find("dock.dialog.2.header")).center();
    press_in(&mut harness, window, header, true);
    harness.frame(Vec::new());
    assert!(
        harness
            .take_window_requests()
            .contains(&WindowRequest::BeginMove(window)),
        "its bar asks its host to move it"
    );
    press_in(&mut harness, window, header, false);
}
