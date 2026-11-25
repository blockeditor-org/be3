use super::*;
use crate::primitives::{Frame, List, Text};
use crate::styled::*;

fn opened(harness: &mut Harness) -> Vec<(WindowId, WindowLevel)> {
    harness
        .take_window_requests()
        .into_iter()
        .filter_map(|request| match request {
            WindowRequest::Open { window, level, .. } => Some((window, level)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_hosted_dialog_is_a_window_of_its_own_a_sheet_when_narrow_and_closes_while_locked() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let narrow = create_signal(false);
    let session = create_signal(false);
    let locked = create_signal(false);
    let level = create_memo(move || match session.get() {
        true => LayerLevel::Raised,
        false => LayerLevel::Normal,
    });
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame width=600.0 height=400.0 />
                <DialogFrame open={open} narrow={narrow} level={level} on_dismiss={move || open.set(false)}>||
                    <Text color=Color32::WHITE>"Body"</Text>
                </DialogFrame>
                <Modal open={locked} scrim=Scrim::Opaque level=LayerLevel::Lock>||
                    <Frame />
                </Modal>
            </List>
        }
    });
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    open.set(true);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let windows = opened(&mut harness);
    assert!(
        matches!(windows[..], [(_, WindowLevel::Normal)]),
        "where windows are hosted it is a window of its own, at the normal level: {windows:?}"
    );
    let (window, _) = windows[0];

    locked.set(true);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        harness
            .take_window_requests()
            .contains(&WindowRequest::Close(window)),
        "the lock layer does not paint over it, so its window closes while locked"
    );
    locked.set(false);
    open.set(false);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    harness.take_window_requests();

    session.set(true);
    open.set(true);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let windows = opened(&mut harness);
    assert!(
        matches!(windows[..], [(_, WindowLevel::Above)]),
        "one about the whole session stays above the other windows: {windows:?}"
    );
    open.set(false);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    harness.take_window_requests();

    narrow.set(true);
    open.set(true);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        opened(&mut harness),
        [],
        "narrow, it is a sheet over its window and opens no window"
    );
}
