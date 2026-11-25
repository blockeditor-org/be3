use super::*;
use crate::dock::{DockDialog, DockPane, DockTab};
use crate::styled::{Modal, Scrim};
use crate::{LayerLevel, WindowId, WindowPlacement, WindowRequest};

struct Dock {
    harness: Harness,
    window: WindowId,
    at: Pos2,
    pressed: Signal<u32>,
    open: Signal<bool>,
    locked: Signal<bool>,
}

fn hosted_dock(size: Vec2) -> Dock {
    let pressed = create_signal(0_u32);
    let open = create_signal(true);
    let locked = create_signal(false);
    let document = build_node(move || {
        let dialogs = create_memo(move || match open.get() {
            true => vec![2u64],
            false => Vec::new(),
        });
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
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
                            <ForEach value={dialogs}>|id: u64|
                                <DockDialog
                                    id
                                    parent=1u64
                                    title="Dialog"
                                    modal=true
                                    size={vec2(240.0, 80.0)}
                                    on_close={move || open.set(false)}
                                >
                                    <Button @test_id="dialog.button" label="Inside" />
                                </DockDialog>
                            </ForEach>
                        </dock::Docking>
                    </dock_host::styled::DockHost>
                </ListItem>
                <Modal open={locked} scrim=Scrim::Opaque level=LayerLevel::Lock>||
                    <Frame />
                </Modal>
            </List>
        }
    });
    let mut harness = Harness::hosting_windows(document, size);
    for _ in 0..4 {
        harness.frame(Vec::new());
    }
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
    Dock {
        harness,
        window,
        at,
        pressed,
        open,
        locked,
    }
}

#[test]
fn a_hosted_dock_dialog_is_a_window_of_its_own_beside_its_tab_that_its_host_moves() {
    let _root = crate::reactive::RootScope::new();
    let Dock {
        mut harness,
        window,
        at,
        pressed,
        ..
    } = hosted_dock(WIDE_VIEWPORT);
    let root = harness.document().root().expect("the dock built a root");
    let tab = harness.rect(dock_tab(harness.document(), root, "Page"));
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

#[test]
fn a_hosted_modal_dock_dialog_keeps_the_focus_and_gives_it_back_to_its_tab() {
    let _root = crate::reactive::RootScope::new();
    let Dock {
        mut harness, open, ..
    } = hosted_dock(WIDE_VIEWPORT);
    open.set(false);
    harness.frame(Vec::new());
    let under = harness.find("page.button");
    harness.click(harness.rect(under).center());
    harness.frame(Vec::new());
    assert_eq!(harness.document().focused, Some(under));

    open.set(true);
    for _ in 0..3 {
        harness.frame(Vec::new());
    }
    let inside = harness.find("dialog.button");
    for _ in 0..4 {
        harness.key(Key::Tab, Modifiers::NONE);
        harness.frame(Vec::new());
        let focused = harness.document().focused;
        assert_ne!(focused, Some(under), "Tab never reaches the blocked panel");
        assert!(
            focused.is_some_and(|node| harness.document().window_of(node)
                == harness.document().window_of(inside)),
            "it stays in the dialog's window: {focused:?}"
        );
    }

    open.set(false);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused,
        Some(under),
        "closed, it gives the focus back to its tab's control in the main window"
    );
}

#[test]
fn a_hosted_dock_dialog_closes_its_window_while_locked() {
    let _root = crate::reactive::RootScope::new();
    let Dock {
        mut harness,
        window,
        locked,
        ..
    } = hosted_dock(WIDE_VIEWPORT);
    locked.set(true);
    harness.frame(Vec::new());
    assert!(
        harness
            .take_window_requests()
            .contains(&WindowRequest::Close(window)),
        "the lock layer does not paint over it, so its window closes"
    );
}

#[test]
fn a_hosted_dock_dialog_turned_into_a_sheet_opens_no_window() {
    let _root = crate::reactive::RootScope::new();
    let Dock {
        mut harness, window, ..
    } = hosted_dock(WIDE_VIEWPORT);
    harness.resize(vec2(390.0, 844.0));
    for _ in 0..4 {
        harness.frame(Vec::new());
    }
    let requests = harness.take_window_requests();
    assert!(
        requests.contains(&WindowRequest::Close(window)),
        "on a phone its window closes: {requests:?}"
    );
    assert!(
        !requests
            .iter()
            .any(|request| matches!(request, WindowRequest::Open { .. })),
        "and no other window opens for its sheet: {requests:?}"
    );
}
