use beui::{BackEdge, BackGesture};

use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);
const FRAME_LIMIT: usize = 60;
/// Frames the finger rests at the end of its swipe before letting go: past the speed's
/// window, so it lets go at rest.
const RESTING_FRAMES: usize = 12;
const RECORDED_FRAMES: usize = 5;

/// Swipes back on the Buttons page over a few frames, rests `resting` frames, lets go and
/// counts the frames until the catalog shows, recording the first few when `record`.
fn swipe_back(resting: usize, record: bool) -> (DocumentTest, usize) {
    let mut test = demo(PHONE);
    open(&mut test, Page::Buttons);
    test.frame(vec![Input::Back(BackGesture::Started {
        edge: BackEdge::Left,
    })]);
    for progress in [0.2, 0.5, 0.8] {
        test.frame(vec![Input::Back(BackGesture::Progressed(progress))]);
    }
    for _ in 0..resting {
        test.frame(vec![Input::Back(BackGesture::Progressed(0.8))]);
    }
    test.frame(vec![Input::Back(BackGesture::Invoked)]);
    let mut frames = 0;
    while test.document().find_test_id("dock.back").is_some() && frames < FRAME_LIMIT {
        if record && frames < RECORDED_FRAMES {
            test.record();
        }
        test.frame(Vec::new());
        frames += 1;
    }
    assert!(
        test.document().find_test_id("dock.back").is_none(),
        "back reaches the catalog"
    );
    (test, frames)
}

#[test]
fn a_back_swipe_let_go_fast_carries_the_page_off_at_its_speed() {
    let (mut thrown, fast) = swipe_back(0, true);
    thrown.snapshot("a_back_swipe_let_go_fast_carries_the_page_off");
    let (_, slow) = swipe_back(RESTING_FRAMES, false);
    assert!(
        fast < slow,
        "a page let go while the swipe still moves carries on at its speed, so it is off \
         sooner than one let go at rest from the same place: {fast} frames against {slow}"
    );
}
