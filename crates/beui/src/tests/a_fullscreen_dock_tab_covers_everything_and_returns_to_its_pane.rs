use super::*;
use crate::reactive::{Interactive, with_reactive_scope};
use crate::unstyled::{DockPane, DockTab, DockingLayout};

#[derive(Default)]
struct Heard {
    hovered: bool,
    scrolled: Vec2,
}

#[test]
fn a_fullscreen_dock_tab_covers_everything_and_returns_to_its_pane() {
    let bar = Rc::new(RefCell::new(Heard::default()));
    let tab = Rc::new(RefCell::new(Heard::default()));
    let layout = DockingLayout::<u64>::new();
    let document = {
        let (bar, tab, layout) = (bar.clone(), tab.clone(), layout.clone());
        build(move || {
            let hovered = bar.clone();
            view! {
                <List spacing=0.0>
                    <Frame height=40.0>
                        <Interactive
                            @test_id="bar"
                            on_hover_change={move |on: bool| hovered.borrow_mut().hovered = on}
                            on_scroll={move |scroll: ScrollGesture| {
                                bar.borrow_mut().scrolled += scroll.delta;
                            }}
                        />
                    </Frame>
                    <styled::Docking @sizing=ItemSize::Percent(100.0) layout focus=2u64>
                        <DockPane id="tabs">
                            <DockTab id=1u64 title="Tab 1">
                                <Frame @test_id="content.1" />
                            </DockTab>
                            <DockTab
                                id=2u64
                                title="Tab 2"
                                content={move || {
                                    let hovered = tab.clone();
                                    let scrolled = tab.clone();
                                    view! {
                                        <Interactive
                                            @test_id="content.2"
                                            on_hover_change={move |on: bool| {
                                                hovered.borrow_mut().hovered = on;
                                            }}
                                            on_scroll={move |scroll: ScrollGesture| {
                                                scrolled.borrow_mut().scrolled += scroll.delta;
                                            }}
                                        />
                                    }
                                }}
                            />
                        </DockPane>
                    </styled::Docking>
                </List>
            }
        })
    };
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let docked = harness.rect(harness.find("content.2"));
    let over_bar = harness.center(harness.find("bar"));

    let entering = layout.clone();
    with_reactive_scope(harness.document_mut(), move || entering.enter_fullscreen(&2, None));
    harness.settle();
    assert_eq!(layout.fullscreen(), Some(2));
    assert_eq!(
        harness.rect(harness.find("content.2")),
        Rect::from_min_size(Pos2::ZERO, WIDE_VIEWPORT),
        "the fullscreen tab covers the whole document, the bar above the dock as well"
    );

    harness.frame(vec![Event::PointerMoved(over_bar)]);
    harness.scroll(over_bar, Vec2::new(0.0, -30.0), Modifiers::NONE);
    harness.settle();
    assert!(!bar.borrow().hovered, "what the tab covers is not hovered");
    assert_eq!(
        bar.borrow().scrolled,
        Vec2::ZERO,
        "and the wheel does not reach it"
    );
    assert!(tab.borrow().hovered, "the fullscreen tab hears the pointer");
    assert_ne!(tab.borrow().scrolled, Vec2::ZERO, "and the wheel");

    let leaving = layout.clone();
    with_reactive_scope(harness.document_mut(), move || leaving.leave_fullscreen());
    harness.settle();
    assert_eq!(layout.fullscreen(), None);
    assert_eq!(
        harness.rect(harness.find("content.2")),
        docked,
        "leaving puts the tab back where it was"
    );
    harness.frame(vec![Event::PointerMoved(over_bar)]);
    assert!(bar.borrow().hovered, "and what it covered hears the pointer again");
}
