use super::*;

#[test]
fn a_beside_overlay_takes_the_css_position_area_of_its_side_and_alignment() {
    let styled = |placement: Placement, property: &str| {
        anchored_styles(placement, Some("--anchor"))
            .into_iter()
            .find(|(name, _)| *name == property)
            .map(|(_, value)| value)
    };
    let cases = [
        (Placement::ABOVE_START, "top span-right", "flip-block, flip-inline, flip-block flip-inline"),
        (Placement::ABOVE_CENTER, "top", "flip-block"),
        (Placement::ABOVE_END, "top span-left", "flip-block, flip-inline, flip-block flip-inline"),
        (Placement::BELOW_START, "bottom span-right", "flip-block, flip-inline, flip-block flip-inline"),
        (Placement::BELOW_CENTER, "bottom", "flip-block"),
        (Placement::BELOW_END, "bottom span-left", "flip-block, flip-inline, flip-block flip-inline"),
        (Placement::LEFT_START, "left span-bottom", "flip-inline, flip-block, flip-inline flip-block"),
        (Placement::LEFT_CENTER, "left", "flip-inline"),
        (Placement::LEFT_END, "left span-top", "flip-inline, flip-block, flip-inline flip-block"),
        (Placement::RIGHT_START, "right span-bottom", "flip-inline, flip-block, flip-inline flip-block"),
        (Placement::RIGHT_CENTER, "right", "flip-inline"),
        (Placement::RIGHT_END, "right span-top", "flip-inline, flip-block, flip-inline flip-block"),
    ];
    for (placement, area, fallbacks) in cases {
        assert_eq!(styled(placement, "position-area").as_deref(), Some(area), "{placement:?}");
        assert_eq!(
            styled(placement, "position-try-fallbacks").as_deref(),
            Some(fallbacks),
            "{placement:?} flips across its anchor first"
        );
    }
    let facing = [
        (Placement::ABOVE_START, "margin-bottom"),
        (Placement::BELOW_START, "margin-top"),
        (Placement::LEFT_START, "margin-right"),
        (Placement::RIGHT_START, "margin-left"),
    ];
    for (placement, margin) in facing {
        assert_eq!(
            styled(placement.gap(6), margin).as_deref(),
            Some("6px"),
            "{placement:?} keeps its gap on the side facing its anchor"
        );
        assert_eq!(styled(placement, margin), None, "no gap, no margin");
    }
    assert_eq!(
        styled(Placement::BELOW_CENTER.margin(8), "max-height").as_deref(),
        Some("calc(100vh - 16px)"),
        "a margin keeps it off the window's edges"
    );
}
