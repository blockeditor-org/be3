use super::*;
use crate::reactive::{Interactive, with_reactive_scope};
use crate::unstyled::{DockPane, DockTab, DockTabControl, DockingLayout, use_dock_tab};

#[test]
fn a_dock_tab_made_fullscreen_from_inside_covers_only_the_area_it_names() {
    let hovered = Rc::new(Cell::new(false));
    let control: Rc<RefCell<Option<DockTabControl>>> = Rc::default();
    let layout = DockingLayout::<u64>::new();
    let document = {
        let (hovered, control, layout) = (hovered.clone(), control.clone(), layout.clone());
        build(move || {
            view! {
                <List spacing=0.0>
                    <Frame height=40.0>
                        <Interactive
                            @test_id="bar"
                            on_hover_change={move |on: bool| hovered.set(on)}
                        />
                    </Frame>
                    <styled::Docking @sizing=ItemSize::Percent(100.0) layout>
                        <DockPane id="tabs">
                            <DockTab
                                id=1u64
                                title="Tab 1"
                                content={move || {
                                    *control.borrow_mut() = use_dock_tab();
                                    view! {
                                        <Frame @test_id="content" />
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
    let control = control.borrow().clone().expect("the tab's content has its control");
    let left = Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDE_VIEWPORT.x / 2.0, WIDE_VIEWPORT.y));
    let bar = harness.rect(harness.find("bar"));

    let entering = control.clone();
    with_reactive_scope(harness.document_mut(), move || entering.enter_fullscreen(Some(left)));
    harness.settle();
    assert!(control.fullscreen());
    assert_eq!(layout.fullscreen(), Some(1));
    assert_eq!(
        harness.rect(harness.find("content")),
        left,
        "the tab covers the area it asked for"
    );

    harness.frame(vec![Event::PointerMoved(pos2(bar.right() - 10.0, bar.center().y))]);
    assert!(hovered.get(), "outside that area the document still hears the pointer");
    harness.frame(vec![Event::PointerMoved(pos2(10.0, bar.center().y))]);
    assert!(!hovered.get(), "inside it, nothing under the tab does");

    let leaving = control.clone();
    with_reactive_scope(harness.document_mut(), move || leaving.leave_fullscreen());
    harness.settle();
    assert!(!control.fullscreen());
    assert_eq!(layout.fullscreen(), None);
}
