use super::*;
use crate::dock::unstyled::{DockPane, DockTab, DockTabControl, DockingLayout, use_dock_tab};
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_dock_tab_made_fullscreen_from_inside_covers_only_the_area_it_names() {
    let _root = crate::reactive::RootScope::new();
    let hovered = create_signal(false);
    let control = create_signal::<Option<DockTabControl>>(None);
    let layout = DockingLayout::<u64>::new();
    let document = {
        build_node(move || {
            view! {
                <List spacing=0.0>
                    <Frame height=40.0>
                        <Interactive @test_id="bar" on_hover_change={hovered} />
                    </Frame>
                    <dock::styled::Docking @sizing=ItemSize::Percent(100.0) layout>
                        <DockPane id="tabs">
                            <DockTab
                                id=1u64
                                title="Tab 1"
                                content={move || {
                                    let tab = use_dock_tab();
                                    control.update(|control| *control = tab);
                                    view! {
                                        <Frame @test_id="content" />
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
    let control = control
        .get_untracked()
        .expect("the tab's content has its control");
    let left = Rect::from_min_size(
        Pos2::ZERO,
        Vec2::new(WIDE_VIEWPORT.x / 2.0, WIDE_VIEWPORT.y),
    );
    let bar = harness.rect(harness.find("bar"));

    control.enter_fullscreen(Some(left));
    harness.settle();
    assert!(control.fullscreen());
    assert_eq!(layout.fullscreen(), Some(1));
    assert_eq!(
        harness.rect(harness.find("content")),
        left,
        "the tab covers the area it asked for"
    );

    harness.frame(vec![Input::PointerMoved(pos2(
        bar.right() - 10.0,
        bar.center().y,
    ))]);
    assert!(
        hovered.get_untracked(),
        "outside that area the document still hears the pointer"
    );
    harness.frame(vec![Input::PointerMoved(pos2(10.0, bar.center().y))]);
    assert!(
        !hovered.get_untracked(),
        "inside it, nothing under the tab does"
    );

    control.leave_fullscreen();
    harness.settle();
    assert!(!control.fullscreen());
    assert_eq!(layout.fullscreen(), None);
}
