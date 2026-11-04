use super::*;
use beui_host::native::testing::Target;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);
const LANDSCAPE_PHONE: Vec2 = Vec2::new(844.0, 390.0);

#[test]
fn a_stage_gathers_the_panes_beside_it_on_a_phone() {
    let mut test = demo(PHONE);
    open(&mut test, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    let stage = test.rect_of("bars.stage");
    let gathered = test.rect_of("dock.compact");
    assert!(
        stage.bottom() <= gathered.top() + 1.0 && gathered.width() > PHONE.x - 1.0,
        "the page's side pane is gathered below its stage, {stage:?} {gathered:?}"
    );
    test.snapshot("a_stage_with_its_panes_gathered_below_it_on_a_phone");

    test.click(Target::text("Tab \"Layers\""));
    test.frame(Vec::new());
    test.settle();
    let folded = test.rect_of("dock.compact");
    assert!(
        folded.height() < gathered.height() / 2.0,
        "tapping the shown tab folds the gathered pane to its tabs, {folded:?}"
    );
    test.snapshot("a_stage_with_its_gathered_panes_folded_on_a_phone");

    let mut test = alone(LANDSCAPE_PHONE, Page::DockBars);
    test.frame(Vec::new());
    test.settle();
    let stage = test.rect_of("bars.stage");
    let gathered = test.rect_of("dock.compact");
    assert!(
        stage.right() <= gathered.left() + 1.0,
        "on a phone turned on its side the panes are gathered right of the stage, {stage:?} {gathered:?}"
    );
    test.snapshot("a_stage_with_its_panes_gathered_beside_it_on_a_phone_on_its_side");
}
