use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::dock::unstyled::{DockPane, DockTab, DockingLayout};

#[derive(Default)]
struct Heard {
    hovered: bool,
    scrolled: Vec2,
}

#[test]
fn a_fullscreen_dock_tab_covers_everything_and_returns_to_its_pane() {
    let _root = crate::reactive::RootScope::new();
    let bar = create_signal(Heard::default());
    let tab = create_signal(Heard::default());
    let layout = DockingLayout::<u64>::new();
    let document = {
        build_node(move || {
            view! {
                <List spacing=0.0>
                    <Frame height=40.0>
                        <Interactive
                            @test_id="bar"
                            on_hover_change={move |on: bool| bar.update(|bar| bar.hovered = on)}
                            on_scroll={move |scroll: ScrollGesture| {
                                bar.update(|bar| bar.scrolled += scroll.delta);
                            }}
                        />
                    </Frame>
                    <dock::styled::Docking @sizing=ItemSize::Percent(100.0) layout focus=2u64>
                        <DockPane id="tabs">
                            <DockTab id=1u64 title="Tab 1">
                                <Frame @test_id="content.1" />
                            </DockTab>
                            <DockTab
                                id=2u64
                                title="Tab 2"
                                content={move || {
                                    view! {
                                        <Interactive
                                            @test_id="content.2"
                                            on_hover_change={move |on: bool| {
                                                tab.update(|tab| tab.hovered = on);
                                            }}
                                            on_scroll={move |scroll: ScrollGesture| {
                                                tab.update(|tab| tab.scrolled += scroll.delta);
                                            }}
                                        />
                                    }
                                }}
                            />
                        </DockPane>
                    </dock::styled::Docking>
                </List>
            }
        })
    };
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let docked = harness.rect(harness.find("content.2"));
    let over_bar = harness.center(harness.find("bar"));

    layout.enter_fullscreen(&2, None);
    harness.settle();
    assert_eq!(layout.fullscreen(), Some(2));
    assert_eq!(
        harness.rect(harness.find("content.2")),
        Rect::from_min_size(Pos2::ZERO, WIDE_VIEWPORT),
        "the fullscreen tab covers the whole document, the bar above the dock as well"
    );

    harness.frame(vec![Input::PointerMoved(over_bar)]);
    harness.scroll_at(over_bar, Vec2::new(0.0, -30.0), Modifiers::NONE);
    harness.settle();
    assert!(
        !bar.with_untracked(|bar| bar.hovered),
        "what the tab covers is not hovered"
    );
    assert_eq!(
        bar.with_untracked(|bar| bar.scrolled),
        Vec2::ZERO,
        "and the wheel does not reach it"
    );
    assert!(
        tab.with_untracked(|tab| tab.hovered),
        "the fullscreen tab hears the pointer"
    );
    assert_ne!(
        tab.with_untracked(|tab| tab.scrolled),
        Vec2::ZERO,
        "and the wheel"
    );

    layout.leave_fullscreen();
    harness.settle();
    assert_eq!(layout.fullscreen(), None);
    assert_eq!(
        harness.rect(harness.find("content.2")),
        docked,
        "leaving puts the tab back where it was"
    );
    harness.frame(vec![Input::PointerMoved(over_bar)]);
    assert!(
        bar.with_untracked(|bar| bar.hovered),
        "and what it covered hears the pointer again"
    );
}
