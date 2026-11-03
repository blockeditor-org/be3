use super::*;
use beui::dock::styled::{
    DockBar, DockBarPlacement, DockBarSurface, DockEdge, DockPane, DockTab, DockWindow, Docking,
    DockingLayout,
};
use beui::{Pos2, Rect, Vec2, pos2, vec2};

const DISPLAY: Vec2 = vec2(900.0, 600.0);

thread_local! {
    static LAYOUT: std::cell::Cell<Option<DockingLayout<u64>>> = const { std::cell::Cell::new(None) };
}

struct PoppedOverRoot;

#[component]
impl BeuiRoot for PoppedOverRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        let _ = editor;
        let layout = DockingLayout::<u64>::new();
        LAYOUT.with(|held| held.set(Some(layout)));
        view! {
            <Root>
                <Docking layout>
                    <DockBar
                        id=9u64
                        placement={DockBarPlacement::Window {
                            edge: DockEdge::Top,
                            origin: pos2(220.0, 180.0),
                        }}
                    >
                        <DockBarSurface @test_id={"bar"}>
                            <Button label="Bar" />
                        </DockBarSurface>
                    </DockBar>
                    <DockWindow
                        id="program"
                        rect={Rect::from_min_size(pos2(60.0, 60.0), vec2(520.0, 380.0))}
                    >
                        <DockPane id="program">
                            <DockTab id=4u64 title="Program">
                                <Frame @test_id={"program"} />
                            </DockTab>
                        </DockPane>
                    </DockWindow>
                </Docking>
            </Root>
        }
    }
}

fn on_top(test: &BeuiTest<PoppedOverRoot>, shown: Rect) -> bool {
    test.desk_windows()
        .last()
        .is_some_and(|top| top.contains_rect(shown))
}

#[test]
fn a_popped_out_bar_stays_above_a_window_raised_over_it_until_covered() {
    let mut test = BeuiTest::<PoppedOverRoot>::editing(Uuid::new_v4());
    test.show_on_displays(&[(Rect::from_min_size(Pos2::ZERO, DISPLAY), 1.0)]);
    test.run();
    test.run();
    let windows = test.desk_windows();
    assert_eq!(
        windows.len(),
        2,
        "the program and the popped-out bar are windows of their own: {windows:?}"
    );
    let bar = test.desk_rects_of("bar");
    let [bar] = bar[..] else {
        panic!("the bar shows in its window: {bar:?}");
    };
    let program = test.desk_rects_of("program")[0];
    assert!(
        program.intersects(bar),
        "the bar's window lies over the program's: {bar:?} {program:?}"
    );
    assert!(
        on_top(&test, bar),
        "the program window, opened after the bar, opens beneath it: {windows:?}"
    );

    let press = pos2(program.center().x, program.bottom() - 40.0);
    test.click(press);
    test.run();
    test.run();
    assert!(
        on_top(&test, bar),
        "a press raises the program window, but not over the bar: {:?}",
        test.desk_windows()
    );
    test.desk_snapshot(
        "a_popped_out_bar_stays_above_a_window_raised_over_it_until_covered",
        0,
    );

    LAYOUT
        .with(std::cell::Cell::get)
        .expect("the app made its dock")
        .set_covered(true);
    test.run();
    test.run();
    test.click(press);
    test.run();
    test.run();
    assert!(
        !on_top(&test, bar),
        "while a fullscreen window covers the bars, a raised window covers the popped-out bar too: {:?}",
        test.desk_windows()
    );
}
