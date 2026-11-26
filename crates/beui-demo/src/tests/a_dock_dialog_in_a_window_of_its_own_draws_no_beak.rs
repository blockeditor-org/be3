use super::*;
use beui::dock::{DockDialog, DockPane, DockTab, Docking};
use beui::dock_host::styled::DockHost;
use beui::{WindowPlacement, WindowRequest};

/// Where a host opens windows, a dock dialog is a window of its own over its tab's window: what
/// it points at is in that other window, so it draws no beak.
#[test]
fn a_dock_dialog_in_a_window_of_its_own_draws_no_beak() {
    let document = build(move || {
        view! {
            <Root>
                <DockHost>
                    <Docking>
                        <DockPane id="main">
                            <DockTab id=1u64 title="Page">
                                <Text>"The page under the dialog"</Text>
                            </DockTab>
                        </DockPane>
                        <DockDialog
                            id=2u64
                            parent=1u64
                            title="Dialog"
                            size={Vec2::new(240.0, 80.0)}
                            on_close={|| {}}
                        >
                            <Text>"In a window of its own"</Text>
                        </DockDialog>
                    </Docking>
                </DockHost>
            </Root>
        }
    });
    let mut test = DocumentTest::hosting_windows(document, Vec2::new(900.0, 600.0));
    for _ in 0..4 {
        test.frame(Vec::new());
    }
    let window = test
        .take_window_requests()
        .into_iter()
        .find_map(|request| match request {
            WindowRequest::Open {
                window,
                placement: WindowPlacement::Over { .. },
                ..
            } => Some(window),
            _ => None,
        })
        .expect("the dialog opens a window of its own");
    test.frame(Vec::new());
    test.snapshot_window("docking_dialog_window_of_its_own", window);
}
