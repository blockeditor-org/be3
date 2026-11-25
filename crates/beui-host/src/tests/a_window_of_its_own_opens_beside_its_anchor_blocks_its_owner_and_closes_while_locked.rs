use super::*;
use crate::primitives::{Frame, Interactive, List};
use crate::reactive::*;
use crate::styled::*;
use crate::{WindowAnchor, WindowId, WindowPlacement, WindowRequest};

const SCRIM: Color32 = Color32::from_rgba_unmultiplied(0, 0, 0, 100);

#[test]
fn a_window_of_its_own_opens_beside_its_anchor_blocks_its_owner_and_closes_while_locked() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let anchor = Rect::from_min_size(pos2(300.0, 100.0), vec2(80.0, 20.0));
    let size = vec2(200.0, 120.0);
    let owner = NodeRef::new();
    let owned = create_signal(None::<NodeId>);
    let owner_pressed = create_signal(0_u32);
    let dismissed = create_signal(0_u32);
    let locked = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Interactive
                    @node_ref=&owner
                    on_press={move |_: PointerPress| owner_pressed.update(|n| *n += 1)}
                >
                    <Frame width=600.0 height=400.0 />
                </Interactive>
                <Window
                    id={window}
                    size={size}
                    owner={owned}
                    anchor={Some(WindowAnchor::Beside(anchor))}
                    blocks=true
                    scrim={SCRIM}
                    on_dismiss={move |_: Dismiss| dismissed.update(|n| *n += 1)}
                >|_: WindowHandle|
                    <Frame />
                </Window>
                <Modal open={locked} scrim=Scrim::Opaque level=LayerLevel::Lock>||
                    <Frame />
                </Modal>
            </List>
        }
    });
    let mut harness = Harness::hosting_windows(document, WIDE_VIEWPORT);
    owned.set(Some(owner.get()));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let opened = harness.take_window_requests();
    let at = opened.iter().find_map(|request| match request {
        WindowRequest::Open {
            window: opened,
            placement: WindowPlacement::Over { window: over, at },
            ..
        } if *opened == window && *over == WindowId::MAIN => Some(*at),
        _ => None,
    });
    let at = at.unwrap_or_else(|| panic!("it opens over the main window: {opened:?}"));
    assert!(
        at.y >= anchor.bottom() && (at.x + size.x / 2.0 - anchor.center().x).abs() < 1.0,
        "below the centre of its anchor, as a card: {at:?} for {anchor:?}"
    );

    let output = harness.frame(Vec::new());
    let dimmed = output.main().shapes().iter().any(|shape| {
        matches!(shape, Shape::Rect { rect, color, .. } if *color == SCRIM && rect.contains(pos2(50.0, 300.0)))
    });
    assert!(dimmed, "the main window dims its owner under it");

    harness.click(pos2(50.0, 300.0));
    harness.frame(Vec::new());
    assert_eq!(
        (owner_pressed.get_untracked(), dismissed.get_untracked()),
        (0, 1),
        "a press on its owner, in another window, is shut out and asks it to close"
    );

    locked.set(true);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(
        harness
            .take_window_requests()
            .contains(&WindowRequest::Close(window)),
        "while locked, the window of its own closes"
    );
}
