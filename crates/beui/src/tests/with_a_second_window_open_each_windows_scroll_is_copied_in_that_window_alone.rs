use super::*;
use crate::geometry::vec2;
use crate::primitives::*;
use crate::primitives::{Window, WindowHandle};
use crate::reactive::*;
use crate::unstyled::Scroll;
use crate::{ScrollDelta, WindowId};

#[test]
fn with_a_second_window_open_each_windows_scroll_is_copied_in_that_window_alone() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame height=60.0 fill=Color32::WHITE radius=0.0 />
                <Frame height=150.0>
                    <Scroll @test_id="main scroll">
                        <Bands count=20 />
                    </Scroll>
                </Frame>
                <Window id={window} size={vec2(160.0, 120.0)}>|_: WindowHandle|
                    <Scroll @test_id="window scroll">
                        <Bands count=20 />
                    </Scroll>
                </Window>
            </List>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(harness.document().open_windows().contains(&window));

    let main_scroll = harness.rect(harness.find("main scroll"));
    harness.frame(vec![Input::PointerMoved(main_scroll.center())]);
    harness.frame(Vec::new());
    let output = harness.frame(vec![Input::Scroll(ScrollDelta::Pixels(vec2(0.0, -20.0)))]);
    let main = output.main();
    let moved = main.moved.expect("the main window's scroll is copied");
    assert_eq!(moved.by, vec2(0.0, -20.0));
    assert!(
        main_scroll.contains_rect(moved.to()),
        "within the scroll {main_scroll:?}: {moved:?}"
    );
    let other = output.window(window).expect("the second window is shown");
    assert!(
        !other.changed && other.damage.is_empty() && other.moved.is_none(),
        "the second window is untouched: {:?} {:?}",
        other.damage.rects(),
        other.moved
    );

    let window_scroll = harness.rect(harness.find("window scroll"));
    harness.frame_in(window, vec![Input::PointerMoved(window_scroll.center())]);
    harness.frame(Vec::new());
    let output = harness.frame_in(
        window,
        vec![Input::Scroll(ScrollDelta::Pixels(vec2(0.0, -30.0)))],
    );
    let other = output.window(window).expect("the second window is shown");
    let moved = other
        .moved
        .expect("the second window's scroll is copied in it");
    assert_eq!(moved.by, vec2(0.0, -30.0));
    assert!(
        window_scroll.contains_rect(moved.to()),
        "within the window's scroll {window_scroll:?}: {moved:?}"
    );
    let main = output.main();
    assert!(
        main.moved.is_none() && main.damage.is_empty(),
        "the main window is untouched: {:?} {:?}",
        main.damage.rects(),
        main.moved
    );
}

#[component]
fn Bands(count: usize) -> DynamicSegment<ListChild> {
    view! {
        <ForEach keys={indices(count)}>|index: usize|
            <Frame height=40.0 fill={Color32::from_gray((index * 10) as u8)} radius=0.0 />
        </ForEach>
    }
}
