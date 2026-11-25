use super::*;
use crate::primitives::List;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use beui::input::{BackEdge, BackGesture};

#[test]
fn a_back_gesture_slides_a_dialog_away_once_its_owner_closes_it() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let dismissed = create_signal(0);
    let body = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dialog
                    open={open}
                    width={Size::Custom(300.0)}
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
    assert_eq!(
        harness.rect(body.get()),
        resting,
        "a dialog stays in place while a back gesture is held"
    );
    harness.frame(vec![Input::Back(BackGesture::Cancelled)]);
    harness.settle();
    assert_eq!(harness.rect(body.get()), resting);
    assert_eq!(dismissed.get_untracked(), 0, "a cancelled back asks nothing");

    harness.frame(vec![
        Input::Back(BackGesture::Started {
            edge: BackEdge::Right,
        }),
        Input::Back(BackGesture::Progressed(0.5)),
    ]);
    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    assert_eq!(
        dismissed.get_untracked(),
        1,
        "the owner is asked as it is let go"
    );
    assert!(
        !harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .handles_back,
        "and once it agrees the dialog is gone at once"
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
