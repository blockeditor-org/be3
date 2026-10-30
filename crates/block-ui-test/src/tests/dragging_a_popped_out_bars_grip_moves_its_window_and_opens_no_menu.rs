use super::*;
use beui::dock::styled::{
    DockBar, DockBarGrip, DockBarPlacement, DockBarSurface, DockEdge, DockPane, DockTab, Docking,
    DockingLayout,
};
use beui::{Pos2, Rect, Vec2, pos2, vec2};

const DISPLAY: Vec2 = vec2(900.0, 600.0);

struct PoppedApp;

#[component]
impl BeuiApp for PoppedApp {
    #[component]
    fn View(editor: Editor) -> Root {
        let _ = editor;
        let layout = DockingLayout::<u64>::new();
        view! {
            <Root>
                <Docking layout>
                    <DockPane id="main">
                        <DockTab id=1u64 title="Main">
                            <Frame />
                        </DockTab>
                    </DockPane>
                    <DockBar
                        id=9u64
                        placement={DockBarPlacement::Window {
                            edge: DockEdge::Top,
                            origin: pos2(200.0, 150.0),
                        }}
                    >
                        <DockBarSurface>
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
fn dragging_a_popped_out_bars_grip_moves_its_window_and_opens_no_menu() {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<PoppedApp>::new(OwnedEditor::new(host, Uuid::new_v4()));
    test.show_on_displays(&[(Rect::from_min_size(Pos2::ZERO, DISPLAY), 1.0)]);
    test.run();
    test.run();
    let windows = test.desk_windows();
    let [window] = windows[..] else {
        panic!("the bar is a window of its own on the desk: {windows:?}");
    };
    let grip = test.desk_rects_of("dock.bar.9.grip");
    let [grip] = grip[..] else {
        panic!("the bar's window shows its grip: {grip:?}");
    };

    test.desk_drag(grip.center(), grip.center() + vec2(120.0, 80.0), 8);
    test.run();
    test.run();
    let moved = test.desk_windows();
    assert_eq!(
        moved,
        vec![window.translate(vec2(120.0, 80.0))],
        "the host moved the bar's window with the pointer"
    );
    assert!(
        test.document().find_test_ids("grip.dock.bar.hide").is_empty(),
        "a press that became a drag opens no menu on release"
    );

    let grip = test.desk_rects_of("dock.bar.9.grip")[0];
    test.desk_press(grip.center());
    test.desk_release(grip.center());
    test.run();
    test.run();
    assert!(
        !test.document().find_test_ids("grip.dock.bar.hide").is_empty(),
        "a click on the grip still opens its menu"
    );
}
