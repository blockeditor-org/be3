use super::*;
use crate::reactive::{NodeRef, Text, build, create_signal, view, with_reactive_scope};
use crate::styled::Dialog;

#[test]
fn a_dialog_opens_in_the_middle_and_escape_dismisses_it() {
    let _root = crate::reactive::RootScope::new();
    let (open, set_open) = create_signal(false);
    let (dismissed, set_dismissed) = create_signal(0);
    let surface = NodeRef::new();
    let document = build({
        move || {
            view! {
                <List spacing=0.0>
                    <Dialog
                        open={open}
                        title="Event"
                        width=300.0
                        on_dismiss={move || set_dismissed.update(|dismissed| *dismissed += 1)}
                    >
                        <Text
                            string="Body"
                            font_size=14.0
                            color=Color32::WHITE
                            @node_ref={&surface}
                        />
                    </Dialog>
                </List>
            }
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert!(
        surface.try_get().is_none(),
        "a closed dialog builds nothing it holds"
    );

    with_reactive_scope(harness.document_mut(), move || set_open.set(true));
    harness.frame(Vec::new());
    let body = harness.rect(surface.get());
    assert!(body.is_positive());
    assert!((body.center().x - VIEWPORT.x / 2.0).abs() < 2.0);

    harness.key(Key::Escape, Modifiers::NONE);
    assert_eq!(dismissed.get_untracked(), 1);
}
