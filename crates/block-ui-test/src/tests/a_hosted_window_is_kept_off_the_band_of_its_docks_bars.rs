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
                    <DockWindow
                        id="window"
                        rect={Rect::from_min_size(pos2(10.0, 450.0), vec2(300.0, 200.0))}
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
                            <Button label="Bar" />
                        </DockBarSurface>
                    </DockBar>
                </Docking>
            </Root>
        }
    }
}

#[test]
fn a_hosted_window_is_kept_off_the_band_of_its_docks_bars() {
    let host = EditorHost::default();
    host.set_editable(true);
    let mut test = BeuiTest::<BandedApp>::new(OwnedEditor::new(host, Uuid::new_v4()));
    test.show_on_displays(&[(Rect::from_min_size(Pos2::ZERO, DISPLAY), 1.0)]);
    test.run();
    test.run();
    let band = test.rect_of("dock.bar.band.bottom");
    let shown = test.desk_windows();
    assert!(
        matches!(shown[..], [window] if window.bottom() <= band.top()),
        "the host keeps the window above the dock's band: {shown:?} {band:?}"
    );

    let layout = LAYOUT
        .with(std::cell::Cell::get)
        .expect("the app made its dock");
    layout.place_bar(
        &9,
        DockBarPlacement::Beside(DockEdge::Left, DockAlign::Center),
    );
    test.run();
    test.run();
    let band = test.rect_of("dock.bar.band.left");
    let moved = test.desk_windows();
    assert!(
        matches!(moved[..], [window] if window.left() >= band.right()),
        "a band moved down the left pushes the window clear of it: {moved:?} {band:?}"
    );
}
