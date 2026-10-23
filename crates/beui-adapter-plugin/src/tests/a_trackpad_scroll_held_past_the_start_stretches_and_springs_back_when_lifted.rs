use std::cell::RefCell;
use std::time::Duration;

use beui::reactive::*;
use beui::styled;
use beui::styled::*;
use beui::unstyled::Scroll;
use beui_core::document::Document;
use block_editor_plugin::{ScrollDelta, TimedInput};

use super::*;

struct Shared(Rc<RefCell<Document>>);

impl App for Shared {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.0.borrow_mut().show(context, rect);
    }
}

#[test]
fn a_trackpad_scroll_held_past_the_start_stretches_and_springs_back_when_lifted() {
    let _root = RootScope::new();
    let scroll = NodeRef::new();
    let document = Rc::new(RefCell::new(build(move || {
        view! {
            <styled::Root>
                <List spacing=0.0>
                    <Scroll @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <ForEach keys={(0..40).collect::<Vec<usize>>()}>|_: usize|
                            <Frame padding={Sides::symmetric(0.0, 20.0)}>
                                <Spacer />
                            </Frame>
                        </ForEach>
                    </Scroll>
                </List>
            </styled::Root>
        }
    })));
    let scroll = scroll.get();
    let mut surface = PluginSurface::launch(
        EditorHost::default(),
        beui::context(),
        Shared(Rc::clone(&document)),
    );
    let shown = region(
        Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)),
        [100, 100],
    );
    let start = surface.context().now();
    surface.context().set_clock(start);
    surface.update(&shown);
    surface.input(
        &shown,
        &TimedInput::from(Input::PointerMoved(pos2(50.0, 50.0))),
    );
    let frame = |surface: &mut PluginSurface, events: &[TimedInput]| {
        surface.context().advance_clock(Duration::from_millis(16));
        for event in events {
            surface.input(&shown, event);
        }
        surface.update(&shown);
    };
    let wheel = TimedInput::from(Input::Scroll(ScrollDelta::Pixels(vec2(0.0, 30.0))));
    let overscroll = || document.borrow().scroll_overscroll(scroll);
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
    assert_eq!(document.borrow().scroll_offset(scroll), 0.0);
}
