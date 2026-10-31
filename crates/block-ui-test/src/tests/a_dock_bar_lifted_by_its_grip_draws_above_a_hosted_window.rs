use super::*;
use beui::dock::styled::{
    DockAlign, DockBar, DockBarGrip, DockBarPlacement, DockBarSurface, DockEdge, DockPane, DockTab,
    DockWindow, Docking, DockingLayout,
};
use beui::{Pos2, Rect, Vec2, pos2, vec2};

const DISPLAY: Vec2 = vec2(900.0, 600.0);

struct LiftedApp;

#[component]
impl BeuiApp for LiftedApp {
    #[component]
    fn View(editor: Editor) -> Root {
        let _ = editor;
        let layout = DockingLayout::<u64>::new();
        view! {
            <Root>
                <Docking layout>
                    <DockWindow
                        id="window"
                        rect={Rect::from_min_size(pos2(200.0, 80.0), vec2(400.0, 300.0))}
                    >
                        <DockPane id="window">
                            <DockTab id=4u64 title="Window">
                                <Frame />
                            </DockTab>
                        </DockPane>
                    </DockWindow>
                    <DockBar
                        id=9u64
                        placement={DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Center)}
                    >
                        <DockBarSurface @test_id={"bar"}>
                            <DockBarGrip />
                            <Button label="Bar" />
                        </DockBarSurface>
                    </DockBar>
                </Docking>
            </Root>
        }
    }
}

#[test]
fn a_dock_bar_lifted_by_its_grip_draws_above_a_hosted_window() {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<LiftedApp>::new(OwnedEditor::new(host, Uuid::new_v4()));
    test.show_on_displays(&[(Rect::from_min_size(Pos2::ZERO, DISPLAY), 1.0)]);
    test.run();
    test.run();
    let windows = test.desk_windows();
    let [window] = windows[..] else {
        panic!("the dock's window is a window of its own on the desk: {windows:?}");
    };
    let grip = test.desk_rects_of("dock.bar.9.grip");
    let [grip] = grip[..] else {
        panic!("the bar shows its grip: {grip:?}");
    };

    let from = grip.center();
    let to = window.center();
    test.desk_press(from);
    for step in 1..=8 {
        test.desk_move(from + (to - from) * (step as f32 / 8.0));
    }
    test.settle();
    let bar = test.desk_rects_of("bar");
    let [bar] = bar[..] else {
        panic!("the lifted bar is shown once: {bar:?}");
    };
    assert!(
        bar.contains(to),
        "the lifted bar follows the pointer over the window: {bar:?} {to:?}"
    );
    let escaped = test.document().escaped_layers();
    assert!(
        escaped.iter().any(|layer| layer.rect.contains_rect(bar)),
        "the lifted bar escapes the dock's own screen, so the host draws it above its windows: {bar:?}"
    );

    test.desk_release(to);
    test.settle();
    let bar = test.desk_rects_of("bar");
    assert!(
        matches!(bar[..], [bar] if bar.top() < 20.0 && (bar.center().x - DISPLAY.x / 2.0).abs() < 20.0),
        "let go at rest over the window, nearest the top, the bar floats over the top: {bar:?}"
    );
}
