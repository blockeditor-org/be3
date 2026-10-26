use super::*;
use crate::input::{BackEdge, BackGesture};
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;
use crate::primitives::List;

#[test]
fn a_back_gesture_lowers_a_sheet_and_it_slides_away_from_there() {
    let _root = crate::reactive::RootScope::new();
    let sheet = NodeRef::new();
    let open = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame @sizing=ItemSize::Percent(100.0) />
                <ModalSheet
                    @node_ref=&sheet
                    open={open}
                    fit=true
                    on_close={move || open.set(false)}
                >
                    <Frame height=200.0 />
                </ModalSheet>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    open.set(true);
    harness.settle();
    let handle = harness.find("sheet.handle");
    let resting = harness.rect(handle).top();

    harness.frame(vec![
        Input::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Input::Back(BackGesture::Progressed(1.0)),
    ]);
    let lowered = harness.rect(handle).top();
    assert!(
        lowered > resting + 40.0,
        "held, the sheet sinks with the gesture, to {lowered} from {resting}"
    );

    harness.frame(vec![Input::Back(BackGesture::Cancelled)]);
    harness.frame(Vec::new());
    let rising = harness.rect(handle).top();
    assert!(
        rising > resting && rising < lowered,
        "let go, it eases back up rather than jumping, and is at {rising}"
    );
    harness.settle();
    assert_eq!(harness.rect(handle).top(), resting);

    harness.frame(vec![
        Input::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Input::Back(BackGesture::Progressed(1.0)),
    ]);
    let lowered = harness.rect(handle).top();
    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    assert!(!open.get_untracked());
    harness.frame(Vec::new());
    let leaving = harness.rect(handle).top();
    assert!(
        leaving > lowered && leaving < 600.0,
        "released, it slides on down from where it was, to {leaving} after {lowered}"
    );
    harness.settle();
    assert!(
        !harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .handles_back
    );
}
