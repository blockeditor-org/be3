use super::*;
use beui::dock::styled::{
    DockBar, DockBarGrip, DockBarPlacement, DockBarSurface, DockEdge, DockPane, DockTab, Docking,
    DockingLayout,
};
use beui::{Key, Modifiers, Pos2, Rect, Vec2, pos2, vec2};

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
fn dragging_a_popped_out_bars_grip_lifts_it_out_of_its_window_and_opens_no_menu() {
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

    test.click(grip.center());
    test.run();
    test.run();
    let hide = test.desk_rects_of("grip.dock.bar.hide");
    let [hide] = hide[..] else {
        panic!("a click on the grip opens its menu: {hide:?}");
    };
    assert!(
        !window.contains_rect(hide),
        "the menu escapes the bar's small window to show whole: {hide:?} {window:?}"
    );
    test.reveal("grip.dock.bar.hide");
    let display = Rect::from_min_size(Pos2::ZERO, DISPLAY);
    let hide = test.desk_rects_of("grip.dock.bar.hide")[0];
    assert!(
        display.contains_rect(hide),
        "the menu, taller than the room on either side of its grip, fits the display and scrolls its last row into it: {hide:?}"
    );
    test.key(Key::Escape, Modifiers::NONE);
    test.run();
    test.run();
    assert!(
        test.desk_rects_of("grip.dock.bar.hide").is_empty(),
        "Escape closes the grip's menu"
    );

    let from = grip.center();
    let to = from + vec2(120.0, 80.0);
    test.desk_press(from);
    test.desk_moves(from, to, 20, std::time::Duration::from_millis(25));
    let held = test.desk_rects_of("dock.bar.9.grip");
    let [held] = held[..] else {
        panic!("the lifted bar shows its grip: {held:?}");
    };
    assert!(
        test.desk_windows()
            .iter()
            .all(|window| !window.intersects(held)),
        "the bar leaves its window when its grip is dragged: {held:?}"
    );
    assert!(
        held.contains(to),
        "the lifted bar follows the pointer: {held:?} {to:?}"
    );
    test.desk_release(to);
    test.settle();
    assert!(
        test.document()
            .find_test_ids("grip.dock.bar.hide")
            .is_empty(),
        "a press that became a drag opens no menu on release"
    );
    assert!(
        test.desk_windows().is_empty(),
        "the dropped bar lands in the dock, not in a window"
    );
    let landed = test.desk_rects_of("dock.bar.9.grip");
    let [landed] = landed[..] else {
        panic!("the landed bar shows its grip: {landed:?}");
    };
    assert!(
        display.contains_rect(landed),
        "the bar lands on the display: {landed:?}"
    );

    test.click(landed.center());
    test.run();
    test.run();
    test.click("grip.dock.bar.hide");
    test.run();
    test.run();
    assert!(
        test.desk_rects_of("dock.bar.9.grip").is_empty(),
        "a click on the landed bar's menu reaches it, and the bar hides"
    );
}
