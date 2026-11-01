use block_editor_beui::beui::dock::styled::{
    DockAlign, DockBar, DockBarGrip, DockBarPlacement, DockBarSurface, DockEdge, DockPane,
    DockSplit, DockTab, DockWindow, Docking, DockingLayout,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::{Button, ButtonSize, ButtonVariant, Frame, Root};
use block_editor_beui::beui::{pos2, vec2};
use block_editor_beui::{BeuiApp, Editor};

use super::*;

const SIZE: Vec2 = vec2(900.0, 600.0);

thread_local! {
    static LAYOUT: std::cell::Cell<Option<DockingLayout<u64>>> = const { std::cell::Cell::new(None) };
}

struct BandedApp;

#[component]
impl BeuiApp for BandedApp {
    #[component]
    fn View(editor: Editor) -> Root {
        let _ = editor;
        let layout = DockingLayout::<u64>::new();
        LAYOUT.with(|held| held.set(Some(layout)));
        view! {
            <Root>
                <Docking layout>
                    <DockSplit id="main">
                        <DockPane id="left">
                            <DockTab id=1u64 title="Left">
                                <Frame @test_id="banded.left" />
                            </DockTab>
                        </DockPane>
                        <DockPane id="right">
                            <DockTab id=2u64 title="Right">
                                <Frame @test_id="banded.right" />
                            </DockTab>
                        </DockPane>
                    </DockSplit>
                    <DockWindow
                        id="window"
                        rect={Rect::from_min_size(pos2(300.0, 580.0), vec2(360.0, 240.0))}
                    >
                        <DockPane id="window">
                            <DockTab id=4u64 title="Window">
                                <Frame @test_id="banded.window" />
                            </DockTab>
                        </DockPane>
                    </DockWindow>
                    <DockBar
                        id=9u64
                        placement={DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Center)}
                    >
                        <DockBarSurface @test_id="banded.bar">
                            <DockBarGrip />
                            <Button
                                label="Apps"
                                variant=ButtonVariant::Ghost
                                size=ButtonSize::Toolbar
                            />
                        </DockBarSurface>
                    </DockBar>
                </Docking>
            </Root>
        }
    }
}

#[test]
fn a_dock_bar_of_the_desktops_dock_spans_it_beneath_its_windows() {
    let mut test = BeuiTest::<BandedApp>::editing(Uuid::new_v4()).with_size(SIZE);
    test.run();
    test.run();

    let bar = test.rect_of("dock.bar.band.bottom");
    assert!(
        bar.left() < 0.5
            && (bar.right() - SIZE.x).abs() < 0.5
            && (bar.bottom() - SIZE.y).abs() < 0.5,
        "the plugin's dock draws its own bar in a band along the whole bottom: {bar:?}"
    );
    for pane in ["banded.left", "banded.right"] {
        let body = test.rect_of(pane);
        assert!(
            body.bottom() <= bar.top() + 0.5,
            "{pane} gives the band its room: {body:?} above {bar:?}"
        );
    }
    let window = test.rect_of("banded.window");
    assert!(
        window.bottom() <= bar.top() + 0.5,
        "a window placed past the band is moved up whole above it: {window:?}, {bar:?}"
    );
    test.snapshot("a_dock_bar_of_the_desktops_dock_spans_it_beneath_its_windows");

    let layout = LAYOUT
        .with(std::cell::Cell::get)
        .expect("the app made its dock");
    layout.place_bar(
        &9,
        DockBarPlacement::Beside(DockEdge::Left, DockAlign::Center),
    );
    test.run();
    test.run();
    let bar = test.rect_of("dock.bar.band.left");
    let left = test.rect_of("banded.left");
    assert!(
        (bar.height() - SIZE.y).abs() < 0.5 && left.left() >= bar.right() - 0.5,
        "placed beside the left, the band runs down the whole dock: {bar:?}, {left:?}"
    );
}
