use super::*;
use beui::dock::{DockDialog, DockPane, DockTab, Docking};
use beui::dock_host::styled::DockHost;

/// A tab in a bare pane shows no label to point at, so its dialog opens at the top centre of
/// the tab's panel with no beak.
#[test]
fn a_dock_dialog_whose_tab_shows_no_label_opens_at_the_top_of_its_panel() {
    let size = Vec2::new(640.0, 400.0);
    let document = build(move || {
        view! {
            <Root>
                <DockHost @test_id="dock">
                    <Docking>
                        <DockPane id="stage" bare=true>
                            <DockTab id=1u64 title="Page">
                                <Text>"The page under the dialog"</Text>
                            </DockTab>
                        </DockPane>
                        <DockDialog
                            id=2u64
                            parent=1u64
                            title="Dialog"
                            size={Vec2::new(240.0, 80.0)}
                        >
                            <Text>"Opened at the top"</Text>
                        </DockDialog>
                    </Docking>
                </DockHost>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, size);
    test.frame(Vec::new());
    let dialog = test.rect_of("dock.dialog.2");
    let panel = test.rect_of("dock");
    assert!(
        (dialog.center().x - panel.center().x).abs() < 1.0,
        "the dialog is centred across its panel: {dialog:?} in {panel:?}"
    );
    assert!(
        dialog.top() < panel.top() + 60.0,
        "at its top: {dialog:?} in {panel:?}"
    );
    test.snapshot("docking_dialog_unpointed");
}
