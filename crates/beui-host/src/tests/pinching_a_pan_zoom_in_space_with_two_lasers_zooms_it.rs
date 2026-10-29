use beui_runner::{Controller, Hand};

use super::*;
use crate::unstyled::PanZoomApi;

#[test]
fn pinching_a_pan_zoom_in_space_with_two_lasers_zooms_it() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        view! {
            <PanZoomStage />
        }
    });
    let mut space = InSpace::new(document);
    space.settle();
    let toward = |hand, x: f32| Controller::Toward {
        hand,
        window: crate::WindowId::MAIN,
        at: pos2(x, 300.0),
    };
    space.controllers([toward(Hand::Left, 300.0), toward(Hand::Right, 500.0)]);
    space.controllers([
        Controller::Trigger {
            hand: Hand::Left,
            value: 1.0,
        },
        Controller::Trigger {
            hand: Hand::Right,
            value: 1.0,
        },
    ]);
    space.controllers([toward(Hand::Left, 100.0), toward(Hand::Right, 700.0)]);
    space.settle();

    let scale = space
        .document
        .borrow()
        .api::<PanZoomApi>(space.document.borrow().find_test_id("stage").unwrap())
        .view()
        .get()
        .scale;
    assert!(
        (scale - 3.0).abs() < 0.05,
        "two lasers spreading pinch it: {scale}"
    );
}
