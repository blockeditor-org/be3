use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_fullscreen_overlay_fills_the_window_and_escape_dismisses_it() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let dismissed = create_signal(0);
    let content = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Fullscreen
                    open={open}
                    on_dismiss={move || dismissed.update(|dismissed| *dismissed += 1)}
                >
                    <Frame fill=Color32::WHITE @node_ref={&content} />
                </Fullscreen>
            </List>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(
        content.try_get().is_none(),
        "a closed fullscreen overlay has built nothing"
    );

    open.set(true);
    harness.frame(Vec::new());
    let filled = harness.rect(content.get());
    assert_eq!(filled.size(), VIEWPORT);

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        dismissed.get_untracked(),
        0,
        "Escape plays the slide-away a back gesture plays"
    );
    harness.settle();
    assert_eq!(
        dismissed.get_untracked(),
        1,
        "and dismisses once it is gone"
    );
}
