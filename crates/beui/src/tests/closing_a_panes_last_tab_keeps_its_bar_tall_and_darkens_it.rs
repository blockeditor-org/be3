use super::*;
use crate::reactive::{Show, clone};
use crate::styled::Theme;

#[test]
fn closing_a_panes_last_tab_keeps_its_bar_tall_and_darkens_it() {
    let document = build(move || {
        let layout = unstyled::DockingLayout::new();
        let (open, set_open) = create_signal(true);
        view! {
            <styled::Docking layout>
                <unstyled::DockPane
                    id="tabs"
                    empty={move || view! {
                        <Frame @test_id={"nothing_open"} />
                    }}
                >
                    <Show condition={open}>
                        <unstyled::DockTab
                            id=1u64
                            title="Tab 1"
                            on_close={clone!(set_open -> move || set_open.set(false))}
                        >
                            <Frame @test_id={"content.1"} />
                        </unstyled::DockTab>
                    </Show>
                </unstyled::DockPane>
            </styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    let tabbed = harness.frame(Vec::new());
    let top = harness.rect(harness.find("content.1")).min.y;
    assert!(
        surface_fills_above(&tabbed, top) > 0,
        "a bar with tabs is the surface color"
    );

    let close = harness.center(harness.find("dock.tab.1.close"));
    harness.click(close);
    harness.frame(Vec::new());
    let vacant = harness.frame(Vec::new());

    let body = harness.rect(harness.find("nothing_open"));
    assert_eq!(body.min.y, top, "the bar keeps its height with no tabs");
    assert_eq!(
        surface_fills_above(&vacant, top),
        0,
        "a bar with no tabs is not the surface color"
    );
}

fn surface_fills_above(output: &crate::FrameOutput, top: f32) -> usize {
    output
        .shapes()
        .iter()
        .filter(|shape| {
            matches!(
                shape,
                crate::painter::Shape::Rect {
                    rect,
                    stroke_width,
                    color,
                    ..
                } if *stroke_width == 0.0
                    && *color == Theme::DARK.surface
                    && rect.max.y <= top
            )
        })
        .count()
}
