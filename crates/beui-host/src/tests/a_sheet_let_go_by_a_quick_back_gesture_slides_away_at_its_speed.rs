use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;
use beui::input::{BackEdge, BackGesture};

/// A back gesture that lowers a sheet quickly and is let go carries that speed into the
/// sheet's slide away, rather than the sheet starting again from rest.
#[test]
fn a_sheet_let_go_by_a_quick_back_gesture_slides_away_at_its_speed() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(true);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Frame />
                </ListItem>
                <ModalSheet open={open} fit=true on_close={move || open.set(false)}>
                    <Frame height=200.0 />
                </ModalSheet>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.settle();
    let handle = harness.find("sheet.handle");
    let resting = harness.rect(handle).top();

    harness.frame(vec![Input::Back(BackGesture::Started {
        edge: BackEdge::Left,
    })]);
    for progress in [0.25, 0.5, 0.75, 1.0] {
        harness.frame(vec![Input::Back(BackGesture::Progressed(progress))]);
    }
    let held = harness.rect(handle).top();
    let step = (held - resting) / 4.0;
    assert!(step > 5.0, "the gesture lowers the sheet, to {held}");

    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    assert!(!open.get_untracked());
    harness.frame(Vec::new());
    let leaving = harness.rect(handle).top();
    assert!(
        leaving - held > step * 0.5,
        "let go, it goes on down at about the speed the gesture lowered it, \
         {} a frame after {step}",
        leaving - held
    );
    harness.settle();
}
