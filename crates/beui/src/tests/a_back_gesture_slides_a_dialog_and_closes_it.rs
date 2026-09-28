use super::*;
use crate::input::{BackEdge, BackGesture};
use crate::reactive::{NodeRef, Text, build, create_signal, view, with_reactive_scope};
use crate::styled::Dialog;

#[test]
fn a_back_gesture_slides_a_dialog_and_closes_it() {
    let (open, set_open) = create_signal(false);
    let dismissed = Rc::new(Cell::new(0));
    let reports = dismissed.clone();
    let body = NodeRef::new();
    let body_ref = body.clone();
    let document = build({
        let open = open.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Dialog
                        open={open}
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
    assert!(!harness.frame(Vec::new()).handles_back);

    with_reactive_scope(harness.document_mut(), move || set_open.set(true));
    assert!(harness.frame(Vec::new()).handles_back);
    let resting = harness.rect(body.get());

    harness.frame(vec![
        Event::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Event::Back(BackGesture::Progressed(0.5)),
    ]);
    let pulled = harness.rect(body.get());
    assert!(pulled.left() > resting.left() + 10.0);
    assert_eq!(pulled.top(), resting.top());

    harness.frame(vec![Event::Back(BackGesture::Cancelled)]);
    assert_eq!(harness.rect(body.get()), resting);
    assert_eq!(dismissed.get(), 0);

    harness.frame(vec![
        Event::Back(BackGesture::Started {
            edge: BackEdge::Right,
        }),
        Event::Back(BackGesture::Progressed(0.5)),
    ]);
    assert!(harness.rect(body.get()).left() < resting.left() - 10.0);

    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
    assert_eq!(dismissed.get(), 1);
    assert!(!harness.frame(Vec::new()).handles_back);
}
