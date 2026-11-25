use super::*;
use crate::primitives::{Frame, Interactive, List};
use crate::reactive::*;
use crate::styled::*;

#[test]
fn the_lock_layer_covers_a_window_placed_away_from_its_owner_and_takes_its_keys() {
    let _root = crate::reactive::RootScope::new();
    let locked = create_signal(false);
    let window_pressed = create_signal(0_u32);
    let lock_pressed = create_signal(0_u32);
    let dismissed = create_signal(0_u32);
    let (owner, inside, unlock) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let owned = create_signal(None::<NodeId>);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame @node_ref=&owner width=100.0 height=60.0 />
                <Window
                    size={vec2(200.0, 100.0)}
                    own_window=false
                    owner={owned}
                    blocks=true
                    at={pos2(400.0, 300.0)}
                    on_dismiss={move |_: Dismiss| dismissed.update(|n| *n += 1)}
                >|_: WindowHandle|
                    <List spacing=0.0>
                        <Interactive
                            on_press={move |_: PointerPress| window_pressed.update(|n| *n += 1)}
                        >
                            <Frame width=200.0 height=60.0 />
                        </Interactive>
                        <Button @node_ref=&inside label="Inside" focused=true />
                    </List>
                </Window>
                <Modal open={locked} scrim=Scrim::Opaque level=LayerLevel::Lock>||
                    <List spacing=0.0>
                        <Interactive on_press={move |_: PointerPress| lock_pressed.update(|n| *n += 1)}>
                            <Frame height=400.0 />
                        </Interactive>
                        <Button @node_ref=&unlock label="Unlock" />
                    </List>
                </Modal>
            </List>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    owned.set(Some(owner.get()));
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused,
        Some(inside.get()),
        "the window placed away from its owner holds the focus"
    );

    locked.set(true);
    harness.frame(Vec::new());
    harness.key(Key::Tab, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        harness.document().focused,
        Some(unlock.get()),
        "once locked, Tab moves within the lock layer, not the window's trap"
    );
    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(dismissed.get_untracked(), 0, "Escape never reaches the window");

    let at = harness.rect(inside.get()).center() - vec2(0.0, 40.0);
    harness.click(at);
    harness.frame(Vec::new());
    assert_eq!(
        (window_pressed.get_untracked(), lock_pressed.get_untracked()),
        (0, 1),
        "and the lock layer takes a press where the window is"
    );
}
