use std::cell::Cell;

use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::dock::unstyled::{DockPane, DockTab, DockWindow, DockingLayout};

#[component]
fn HoverInto(hovered: Rc<Cell<bool>>) -> NodeId {
    view! {
        <Interactive on_hover_change={move |on: bool| hovered.set(on)} />
    }
}

fn hover(harness: &mut Harness, pos: Pos2) {
    harness.frame(vec![Input::PointerMoved(pos2(1.0, 1.0))]);
    harness.frame(vec![Input::PointerMoved(pos)]);
}

#[test]
fn a_fullscreen_dock_tab_comes_back_over_a_window_raised_above_it() {
    let _root = crate::reactive::RootScope::new();
    let program = Rc::new(Cell::new(false));
    let other = Rc::new(Cell::new(false));
    let layout = DockingLayout::<u64>::new();
    let document = {
        let (program, other, layout) = (program.clone(), other.clone(), layout);
        build_node(move || {
            let (program, other) = (program.clone(), other.clone());
            view! {
                <dock::styled::Docking layout>
                    <DockPane
                        id="desktop"
                        empty={|| view! {
                            <Frame />
                        }}
                    />
                    <DockWindow
                        id="program"
                        rect={Rect::from_min_size(pos2(40.0, 40.0), vec2(360.0, 240.0))}
                    >
                        <DockPane id="program">
                            <DockTab
                                id=1u64
                                title="Program"
                                content={move || view! {
                                    <HoverInto hovered={program.clone()} @test_id="program" />
                                }}
                            />
                        </DockPane>
                    </DockWindow>
                    <DockWindow
                        id="other"
                        rect={Rect::from_min_size(pos2(300.0, 200.0), vec2(360.0, 240.0))}
                    >
                        <DockPane id="other">
                            <DockTab
                                id=2u64
                                title="Other"
                                content={move || view! {
                                    <HoverInto hovered={other.clone()} @test_id="other" />
                                }}
                            />
                        </DockPane>
                    </DockWindow>
                </dock::styled::Docking>
            }
        })
    };
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let over_other = harness.center(harness.find("other"));

    layout.show(&1);
    layout.enter_fullscreen(&1, None);
    harness.settle();
    hover(&mut harness, over_other);
    assert!(program.get(), "the fullscreen tab covers the other window");
    assert!(!other.get());

    layout.show(&2);
    harness.settle();
    hover(&mut harness, over_other);
    assert!(
        other.get() && !program.get(),
        "a window shown after it comes in front of the fullscreen tab"
    );

    layout.show(&1);
    harness.settle();
    hover(&mut harness, over_other);
    assert_eq!(layout.focused(), Some(1));
    assert!(
        program.get() && !other.get(),
        "showing the fullscreen tab again brings it in front, not the window it left"
    );

    layout.show(&2);
    harness.settle();
    assert_eq!(layout.focused(), Some(2));
    harness.click(pos2(WIDE_VIEWPORT.x - 20.0, WIDE_VIEWPORT.y - 20.0));
    harness.settle();
    assert_eq!(
        layout.focused(),
        Some(1),
        "a click on the uncovered part of the fullscreen tab focuses it"
    );
    hover(&mut harness, over_other);
    assert!(
        program.get() && !other.get(),
        "and brings it in front of the window that covered it"
    );
    assert_eq!(
        layout.fullscreen(),
        Some(1),
        "it stays fullscreen throughout"
    );
}
