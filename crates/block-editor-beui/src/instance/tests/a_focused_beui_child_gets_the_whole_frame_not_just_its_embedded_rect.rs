use super::*;
use beui::reactive::*;
use beui::styled::*;

struct RecordingApp;

#[component]
impl crate::BeuiApp for RecordingApp {
    #[component]
    fn View(_editor: crate::Editor) -> beui::styled::Root {
        let node = {
            view! {
                <Frame />
            }
        };
        view! {
            <beui::styled::Root>{node}</beui::styled::Root>
        }
    }
}

#[test]
fn a_focused_beui_child_gets_the_whole_frame_not_just_its_embedded_rect() {
    let mut session = session::<RecordingApp>(Uuid::new_v4());
    frame(
        &mut session,
        Some(Rect::from_min_size(
            beui::Pos2::new(300.0, 250.0),
            beui::Vec2::new(120.0, 90.0),
        )),
        TopBar::Shown,
    );

    session.run(ViewId::MAIN, 0);

    let report = session
        .report(ViewId::MAIN)
        .expect("the frame region reports its content rect");

    assert!(
        report.content.left + report.content.right < 400.0,
        "expected the child's own content to span close to the full viewport width, got {:?}",
        report.content
    );
}
