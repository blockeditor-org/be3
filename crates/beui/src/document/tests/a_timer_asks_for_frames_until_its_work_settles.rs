use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::reactive::{create_timer, view};

#[test]
fn a_timer_asks_for_frames_until_its_work_settles() {
    let runs = Rc::new(Cell::new(0));
    let document = build({
        let runs = Rc::clone(&runs);
        move || {
            let timer = create_timer(move || {
                runs.set(runs.get() + 1);
                (runs.get() < 3).then_some(Duration::ZERO)
            });
            timer.start(Duration::ZERO);
            view! {
                <List spacing=0.0>
                    <Frame height=10.0 />
                </List>
            }
        }
    });
    let mut harness = Harness::new(document);

    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::ZERO);
    assert_eq!(runs.get(), 1);
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::ZERO);
    assert_eq!(runs.get(), 2);
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);
    assert_eq!(runs.get(), 3);
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);
    assert_eq!(runs.get(), 3, "a timer whose work settled runs no more");
}
