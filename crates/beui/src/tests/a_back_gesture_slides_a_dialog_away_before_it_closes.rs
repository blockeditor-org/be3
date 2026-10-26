use super::*;
use crate::input::{BackEdge, BackGesture};
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn a_back_gesture_slides_a_dialog_away_before_it_closes() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let dismissed = create_signal(0);
    let body = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dialog
                    open={open}
                    width=300.0
                    on_dismiss={move || {
                        dismissed.update(|dismissed| *dismissed += 1);
                        open.set(false);
                    }}
                >
                    <Text color=Color32::WHITE @node_ref={&body}>"Body"</Text>
                </Dialog>
            </List>
        }
    });

    let mut harness = Harness::new(document);
    assert!(
        !harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .handles_back
    );

    open.set(true);
    assert!(
        harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .handles_back
    );
    harness.frame(Vec::new());
    let resting = harness.rect(body.get());

    harness.frame(vec![
        Input::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Input::Back(BackGesture::Progressed(1.0)),
    ]);
    let pulled = harness.rect(body.get());
    assert!(
        pulled.left() > resting.left() + 100.0,
        "held all the way, the dialog has moved well out of place: {pulled:?} from {resting:?}"
    );
    assert_eq!(pulled.top(), resting.top());

    harness.frame(vec![Input::Back(BackGesture::Cancelled)]);
    harness.frame(Vec::new());
    let returning = harness.rect(body.get()).left();
    assert!(
        returning > resting.left() && returning < pulled.left(),
        "let go, it eases back rather than jumping, and is at {returning}"
    );
    harness.settle();
    assert_eq!(harness.rect(body.get()), resting);
    assert_eq!(dismissed.get_untracked(), 0);

    harness.frame(vec![
        Input::Back(BackGesture::Started {
            edge: BackEdge::Right,
        }),
        Input::Back(BackGesture::Progressed(0.5)),
    ]);
    let pulled = harness.rect(body.get());
    assert!(pulled.left() < resting.left() - 40.0);

    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    harness.frame(Vec::new());
    let leaving = harness.rect(body.get());
    assert!(
        leaving.left() < pulled.left(),
        "released, it carries on the way it was going: {leaving:?} after {pulled:?}"
    );
    assert_eq!(
        dismissed.get_untracked(),
        0,
        "and closes only once it is gone"
    );
    harness.settle();
    assert_eq!(dismissed.get_untracked(), 1);
    assert!(
        !harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .handles_back
    );

    open.set(true);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        harness.rect(body.get()),
        resting,
        "opened again, it is back in place"
    );
}
