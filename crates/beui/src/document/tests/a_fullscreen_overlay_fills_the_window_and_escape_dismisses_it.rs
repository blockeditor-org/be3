use super::*;
use crate::reactive::{Frame, NodeRef, build, create_signal, view, with_reactive_scope};
use crate::styled::Fullscreen;

#[test]
fn a_fullscreen_overlay_fills_the_window_and_escape_dismisses_it() {
    let (open, set_open) = create_signal(false);
    let dismissed = Rc::new(Cell::new(0));
    let reports = dismissed.clone();
    let content = NodeRef::new();
    let content_ref = content.clone();
    let document = build({
        let open = open.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Fullscreen open={open} on_dismiss={move || reports.set(reports.get() + 1)}>
                        <Frame color=Color32::WHITE @node_ref={&content_ref} />
                    </Fullscreen>
                </List>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(harness.document().node_rect(content.get()).is_none());

    with_reactive_scope(harness.document_mut(), move || set_open.set(true));
    harness.frame(Vec::new());
    let filled = harness.rect(content.get());
    assert_eq!(filled.size(), VIEWPORT);

    harness.key(Key::Escape, Modifiers::NONE);
    assert_eq!(dismissed.get(), 1);
}
