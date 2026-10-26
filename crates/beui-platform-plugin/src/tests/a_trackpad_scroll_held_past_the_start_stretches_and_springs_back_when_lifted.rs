use std::time::Duration;

use beui::styled::*;
use beui::unstyled::Scroll;
use block_editor_plugin::{ScrollDelta, TimedInput};

use super::*;

#[test]
fn a_trackpad_scroll_held_past_the_start_stretches_and_springs_back_when_lifted() {
    let _root = RootScope::new();
    let scroll = NodeRef::new();
    let document = Rc::default();
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |_| {
            view! {
                <List spacing=0.0>
                    <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <ForEach keys={(0..40).collect::<Vec<usize>>()}>|_: usize|
                            <Frame padding={Sides::symmetric(0.0, 20.0)}>
                                <Spacer />
                            </Frame>
                        </ForEach>
                    </Scroll>
                </List>
            }
        })
        .sharing(&document),
    );
    let shown = whole(100.0, 100.0);
    let start = surface.context().now();
    surface.context().set_clock(start);
    one(&mut surface, &shown);
    let scroll = scroll.get();
    surface.input(
        &shown,
        &TimedInput::from(Input::PointerMoved(pos2(50.0, 50.0))),
        None,
    );
    let frame = |surface: &mut ModuleRunner, events: &[TimedInput]| {
        surface.context().advance_clock(Duration::from_millis(16));
        for event in events {
            surface.input(&shown, event, None);
        }
        one(surface, &shown);
    };
    let wheel = TimedInput::from(Input::Scroll(ScrollDelta::Pixels(vec2(0.0, 30.0))));
    let overscroll = || {
        document
            .borrow()
            .as_ref()
            .map_or(f32::NAN, |document: &Document| {
                document.scroll_overscroll(scroll)
            })
    };
    for _ in 0..3 {
        frame(&mut surface, std::slice::from_ref(&wheel));
    }
    assert_eq!(overscroll(), 0.0, "a mouse wheel stops at the start");

    frame(&mut surface, &[TimedInput::from(Input::ScrollStarted)]);
    for _ in 0..5 {
        frame(&mut surface, std::slice::from_ref(&wheel));
    }
    let stretched = overscroll();
    assert!(
        stretched < -1.0,
        "fingers on the trackpad pull the list past its start: {stretched}"
    );
    for _ in 0..30 {
        frame(&mut surface, &[]);
    }
    assert_eq!(
        overscroll(),
        stretched,
        "the list stays stretched while the fingers rest"
    );

    frame(&mut surface, &[TimedInput::from(Input::ScrollEnded)]);
    for _ in 0..120 {
        frame(&mut surface, &[]);
    }
    assert_eq!(
        overscroll(),
        0.0,
        "lifting the fingers springs the band back"
    );
    assert_eq!(
        document
            .borrow()
            .as_ref()
            .map(|document: &Document| document.scroll_offset(scroll)),
        Some(0.0)
    );
}
