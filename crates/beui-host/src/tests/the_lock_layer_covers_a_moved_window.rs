use super::*;
use crate::primitives::{Frame, Interactive, List};
use crate::reactive::*;
use crate::styled::*;

#[test]
fn the_lock_layer_covers_a_moved_window() {
    let _root = crate::reactive::RootScope::new();
    let locked = create_signal(false);
    let window_pressed = create_signal(0_u32);
    let lock_pressed = create_signal(0_u32);
    let inside = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Window
                    size={vec2(200.0, 100.0)}
                    own_window=false
                    blocks=true
                    at={pos2(300.0, 200.0)}
                >|_: WindowHandle|
                    <Interactive
                        @node_ref=&inside
                        on_press={move |_: PointerPress| window_pressed.update(|n| *n += 1)}
                    >
                        <Frame width=200.0 height=100.0 />
                    </Interactive>
                </Window>
                <Modal open={locked} scrim=Scrim::Opaque level=LayerLevel::Lock>||
                    <Interactive on_press={move |_: PointerPress| lock_pressed.update(|n| *n += 1)}>
                        <Frame />
                    </Interactive>
                </Modal>
            </List>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let at = harness.center(inside.get());
    harness.click(at);
    harness.frame(Vec::new());
    assert_eq!(window_pressed.get_untracked(), 1, "the moved window takes a press");

    locked.set(true);
    harness.frame(Vec::new());
    harness.click(at);
    harness.frame(Vec::new());
    assert_eq!(
        (window_pressed.get_untracked(), lock_pressed.get_untracked()),
        (1, 1),
        "once locked, the lock layer takes the press where the window is"
    );
}
