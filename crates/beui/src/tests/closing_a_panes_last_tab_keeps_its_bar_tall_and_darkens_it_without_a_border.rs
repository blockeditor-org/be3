use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn closing_a_panes_last_tab_keeps_its_bar_tall_and_darkens_it_without_a_border() {
    let document = build_node(move || {
        let layout = dock::unstyled::DockingLayout::new();
        let open = create_signal(true);
        view! {
            <dock::styled::Docking layout>
                <dock::unstyled::DockPane
                    id="tabs"
                    empty={move || view! {
                        <Frame @test_id={"nothing_open"} />
                    }}
                >
                    <Show condition={open}>
                        <dock::unstyled::DockTab id=1u64 title="Tab 1" on_close={move || open.set(false)}>
                            <Frame @test_id={"content.1"} />
                        </dock::unstyled::DockTab>
                    </Show>
                </dock::unstyled::DockPane>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    let tabbed = harness.frame(Vec::new());
    let top = harness.rect(harness.find("content.1")).min.y;
    assert!(
        fills_above(&tabbed, top, Theme::DARK.surface) > 0,
        "a bar with tabs is the surface color"
    );
    assert!(
        fills_above(&tabbed, top, Theme::DARK.border) > 0,
        "a bar with tabs has a border under it"
    );

    let close = harness.center(harness.find("dock.tab.1.close"));
    harness.click(close);
    harness.frame(Vec::new());
    let vacant = harness.frame(Vec::new());

    let body = harness.rect(harness.find("nothing_open"));
    assert_eq!(body.min.y, top, "the bar keeps its height with no tabs");
    assert_eq!(
        fills_above(&vacant, top, Theme::DARK.surface),
        0,
        "a bar with no tabs is not the surface color"
    );
    assert_eq!(
        fills_above(&vacant, top, Theme::DARK.border),
        0,
        "a bar with no tabs has no border under it"
    );
}

fn fills_above(output: &crate::FrameOutput, top: f32, fill: crate::color::Color32) -> usize {
    output
        .main()
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
                    && *color == fill
                    && rect.max.y <= top
            )
        })
        .count()
}
