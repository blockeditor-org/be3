use super::*;
use beui::dock::styled::{
    DockAlign, DockBar, DockBarPlacement, DockBarSurface, DockEdge, DockPane, DockTab, DockWindow,
    Docking, DockingLayout,
};
use beui::{Pos2, Rect, Vec2, pos2, vec2};

const DISPLAY: Vec2 = vec2(900.0, 600.0);

thread_local! {
    static LAYOUT: std::cell::Cell<Option<DockingLayout<u64>>> = const { std::cell::Cell::new(None) };
}

struct HostedApp;

#[component]
impl BeuiApp for HostedApp {
    #[component]
    fn View(editor: Editor) -> Root {
        let _ = editor;
        let layout = DockingLayout::<u64>::new();
        LAYOUT.with(|held| held.set(Some(layout)));
        view! {
            <Root>
                <Docking layout>
                    <DockPane id="main">
                        <DockTab id=1u64 title="Main">
                            <Frame />
                        </DockTab>
                    </DockPane>
                    <DockWindow
                        id="window"
                        rect={Rect::from_min_size(pos2(200.0, 150.0), vec2(300.0, 200.0))}
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
                        <DockBarSurface>
                            <Button label="Bar" />
                        </DockBarSurface>
                    </DockBar>
                </Docking>
            </Root>
        }
    }
}

#[test]
fn a_hosted_window_keeps_its_place_when_a_dock_bar_moves_the_docks_area() {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<HostedApp>::new(OwnedEditor::new(host, Uuid::new_v4()));
    test.show_on_displays(&[(Rect::from_min_size(Pos2::ZERO, DISPLAY), 1.0)]);
    test.run();
    test.run();
    let layout = LAYOUT
        .with(std::cell::Cell::get)
        .expect("the app made its dock");
    let was = layout.window_rect(&4).expect("the window is open");
    let shown = test.desk_windows();
    assert!(
        matches!(shown[..], [window] if window.contains_rect(was)),
        "the host holds the dock's window where the dock placed it, though the window was built before it was hosted: {shown:?}, {was:?}"
    );

    layout.place_bar(
        &9,
        DockBarPlacement::Beside(DockEdge::Top, DockAlign::Center),
    );
    test.run();
    test.run();
    assert_eq!(
        test.desk_windows(),
        shown,
        "the host's window stays where it was when the dock's band moves to the top"
    );
    assert_eq!(
        layout.window_rect(&4),
        Some(was),
        "and the dock still knows where it is, though its own area moved down under the band"
    );
}
