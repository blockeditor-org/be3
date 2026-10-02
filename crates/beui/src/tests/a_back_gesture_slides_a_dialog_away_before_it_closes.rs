use super::*;
use crate::input::{BackEdge, BackGesture};
use crate::reactive::{NodeRef, Text, build, create_signal, view, with_reactive_scope};
use crate::styled::Dialog;

#[test]
fn a_back_gesture_slides_a_dialog_away_before_it_closes() {
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

    let opening = set_open.clone();
    with_reactive_scope(harness.document_mut(), move || opening.set(true));
    assert!(harness.frame(Vec::new()).handles_back);
    harness.frame(Vec::new());
    let resting = harness.rect(body.get());

    harness.frame(vec![
        Event::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Event::Back(BackGesture::Progressed(1.0)),
    ]);
    let pulled = harness.rect(body.get());
    assert!(
        pulled.left() > resting.left() + 100.0,
        "held all the way, the dialog has moved well out of place: {pulled:?} from {resting:?}"
    );
    assert_eq!(pulled.top(), resting.top());

    harness.frame(vec![Event::Back(BackGesture::Cancelled)]);
    harness.frame(Vec::new());
    let returning = harness.rect(body.get()).left();
    assert!(
        returning > resting.left() && returning < pulled.left(),
        "let go, it eases back rather than jumping, and is at {returning}"
    );
    harness.settle();
    assert_eq!(harness.rect(body.get()), resting);
    assert_eq!(dismissed.get(), 0);

    harness.frame(vec![
        Event::Back(BackGesture::Started {
            edge: BackEdge::Right,
        }),
        Event::Back(BackGesture::Progressed(0.5)),
    ]);
    let pulled = harness.rect(body.get());
    assert!(pulled.left() < resting.left() - 40.0);

    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
    harness.frame(Vec::new());
    let leaving = harness.rect(body.get());
    assert!(
        leaving.left() < pulled.left(),
        "released, it carries on the way it was going: {leaving:?} after {pulled:?}"
    );
    assert_eq!(dismissed.get(), 0, "and closes only once it is gone");
    harness.settle();
    assert_eq!(dismissed.get(), 1);
    assert!(!harness.frame(Vec::new()).handles_back);

    let closing = set_open.clone();
    with_reactive_scope(harness.document_mut(), move || closing.set(false));
    harness.frame(Vec::new());
    with_reactive_scope(harness.document_mut(), move || set_open.set(true));
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(body.get()),
        resting,
        "opened again, it is back in place"
    );
}
