use std::time::Duration;

use block_editor_beui::beui::{pos2, vec2};
use block_editor_beui::{DisplayAnswer, DisplayConfirmation, PendingDisplayChange};

use super::*;

fn ask(fixture: &mut Fixture, round: u64) {
    fixture
        .test
        .set_host_value::<DisplayConfirmation>(&Some(PendingDisplayChange {
            round,
            timeout: Duration::from_secs(15),
        }));
    fixture.settle();
}

fn settle_answer(fixture: &mut Fixture) {
    fixture.test.set_host_value::<DisplayConfirmation>(&None);
    fixture.settle();
}

#[test]
fn a_display_change_asks_on_every_monitor_to_be_kept() {
    let left = Rect::from_min_size(pos2(0.0, 0.0), vec2(500.0, 600.0));
    let right = Rect::from_min_size(pos2(500.0, 0.0), vec2(300.0, 400.0));
    let mut fixture = Fixture::new();
    fixture
        .test
        .set_monitors(vec![("left", left), ("right", right)]);
    fixture.settle();
    assert!(!fixture.test.shown("display.keep.0"));

    ask(&mut fixture, 4);
    assert!(left.contains_rect(fixture.test.rect_of("display.keep.0")));
    assert!(right.contains_rect(fixture.test.rect_of("display.keep.1")));
    assert!(fixture.says("Reverting in 15 seconds."));
    fixture.test.snapshot("the_display_prompt_on_two_monitors");

    fixture.test.click("display.keep.0.keep");
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<DisplayAnswer>(),
        vec![DisplayAnswer::Keep(4)],
        "the answer names the change it answers"
    );
    settle_answer(&mut fixture);
    assert!(!fixture.test.shown("display.keep.0"));

    ask(&mut fixture, 5);
    fixture.test.click("display.keep.1.revert");
    fixture.settle();
    assert_eq!(
        fixture.test.take_actions::<DisplayAnswer>(),
        vec![DisplayAnswer::Revert(5)]
    );
    settle_answer(&mut fixture);

    ask(&mut fixture, 6);
    fixture.test.advance(Duration::from_secs(5));
    assert!(fixture.says("Reverting in 10 seconds."));
    fixture.test.advance(Duration::from_secs(10));
    assert_eq!(
        fixture.test.take_actions::<DisplayAnswer>(),
        vec![DisplayAnswer::Revert(6)],
        "the desktop reverts once its countdown runs out, as the host does on its own"
    );
}
