use std::cell::RefCell;
use std::time::Duration;

use beui::primitives::*;
use beui::reactive::*;
use beui::styled;
use beui::unstyled::Scroll;
use beui_core::document::Document;
use block_editor_plugin::WheelUnit;

use super::*;

struct Shared(Rc<RefCell<Document>>);

impl App for Shared {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.0.borrow_mut().show(context, rect);
    }
}

#[test]
fn a_trackpad_scroll_lifted_against_the_start_bands_and_springs_back() {
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
        &InputEvent::PointerMoved {
            x: 50.0,
            y: 50.0,
            time_micros: 0,
        },
    );
    let frame = |surface: &mut PluginSurface, events: &[InputEvent]| {
        surface.context().advance_clock(Duration::from_millis(16));
        for event in events {
            surface.input(&shown, event);
        }
        surface.update(&shown);
    };
    let wheel = InputEvent::Wheel {
        x: 0.0,
        y: 30.0,
        unit: WheelUnit::Pixels,
    };
    for _ in 0..5 {
        frame(&mut surface, std::slice::from_ref(&wheel));
    }
    let overscroll = || document.borrow().scroll_overscroll(scroll);
    assert_eq!(
        overscroll(),
        0.0,
        "the fingers alone keep the list at its start"
    );

    frame(&mut surface, &[InputEvent::WheelEnded]);
    let mut deepest: f32 = 0.0;
    for _ in 0..120 {
        frame(&mut surface, &[]);
        deepest = deepest.min(overscroll());
    }
    assert!(
        deepest < -1.0,
        "lifting the fingers flings the list past its start, where it bands: {deepest}"
    );
    assert_eq!(overscroll(), 0.0, "the band springs back");
    assert_eq!(document.borrow().scroll_offset(scroll), 0.0);
}
