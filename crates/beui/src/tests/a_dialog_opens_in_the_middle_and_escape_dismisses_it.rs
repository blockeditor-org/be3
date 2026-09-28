use super::*;
use crate::reactive::{NodeRef, Text, build, create_signal, view, with_reactive_scope};
use crate::styled::Dialog;

#[test]
fn a_dialog_opens_in_the_middle_and_escape_dismisses_it() {
    let (open, set_open) = create_signal(false);
    let dismissed = Rc::new(Cell::new(0));
    let reports = dismissed.clone();
    let surface = NodeRef::new();
    let body_ref = surface.clone();
    let document = build({
        let open = open.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Dialog
                        open={open}
                        title="Event"
                        width=300.0
                        on_dismiss={move || reports.set(reports.get() + 1)}
                    >
                        <Text
                            string="Body"
                            font_size=14.0
                            color=Color32::WHITE
                            @node_ref={&body_ref}
                        />
                    </Dialog>
                </List>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(harness.document().node_rect(surface.get()).is_none());

    with_reactive_scope(harness.document_mut(), move || set_open.set(true));
    harness.frame(Vec::new());
    let body = harness.rect(surface.get());
    assert!(body.is_positive());
    assert!((body.center().x - VIEWPORT.x / 2.0).abs() < 2.0);

    harness.key(Key::Escape, Modifiers::NONE);
    assert_eq!(dismissed.get(), 1);
}
