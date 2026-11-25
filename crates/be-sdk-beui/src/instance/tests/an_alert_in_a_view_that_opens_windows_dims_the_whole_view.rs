use super::*;
use beui::reactive::*;

struct AlertRoot;

#[beui::reactive::component]
impl crate::BeuiRoot for AlertRoot {
    #[beui::reactive::component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        use beui::styled::{Dialog, DialogKind, Frame, Text};
        let _ = editor;
        view! {
            <beui::styled::Root>
                <beui::styled::List spacing=beui::styled::Space::None>
                    <Frame />
                    <Dialog open=true kind=DialogKind::Alert title="Discard?" on_dismiss={|| {}}>
                        <Text>"Body"</Text>
                    </Dialog>
                </beui::styled::List>
            </beui::styled::Root>
        }
    }
}

#[test]
fn an_alert_in_a_view_that_opens_windows_dims_the_whole_view() {
    let mut session = session::<AlertRoot>(Uuid::new_v4());
    let (placement, mut request) = dock_screen(View::MAIN, 0, false);
    request.windows = true;
    session.place(&[placement], &[request]);
    let mut opened = Vec::new();
    for _ in 0..3 {
        session.frame(&[(View::MAIN.id, 0)]);
        let mut commit = be_sdk_api::LayoutCommit {
            serial: 0,
            sizes: Vec::new(),
            reports: Vec::new(),
            windows: Vec::new(),
            children: Vec::new(),
            window_requests: Vec::new(),
            docks: Vec::new(),
        };
        session.commit(&mut commit);
        opened.extend(commit.window_requests);
    }
    assert!(
        opened
            .iter()
            .any(|request| matches!(request, be_sdk_api::WindowRequest::Open(_))),
        "the alert opens as a window of its own: {opened:?}"
    );
    let report = session.report(View::MAIN.id).expect("the frame reports");
    let scrims: Vec<_> = report.scrims.iter().map(|scrim| scrim.rect).collect();
    assert_eq!(
        scrims,
        [be_sdk_api::Rect {
            min: be_sdk_api::Pos2::ZERO,
            max: be_sdk_api::Pos2 { x: 400.0, y: 600.0 },
        }],
        "its cover dims the whole view it is opened over, not a corner the size of its window"
    );
}
