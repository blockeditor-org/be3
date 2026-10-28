use super::*;
use beui::reactive::*;
use block_plugin_api::{WindowPlacement, WindowRequest};

thread_local! {
    static VIEWPORT: std::cell::Cell<beui::Vec2> = const { std::cell::Cell::new(beui::Vec2::ZERO) };
}

#[beui::reactive::component]
fn OverOwnWindow() -> beui::NodeId {
    use beui::styled::{Frame, Window, WindowHandle, use_window};
    let viewport = use_viewport_size();
    create_effect(move || {
        let size = viewport.get();
        VIEWPORT.with(|held| held.set(size));
    });
    let own = use_window().map(|window| window.id);
    let placement = own.map_or(beui::WindowPlacement::AtPointer, |window| {
        beui::WindowPlacement::Over {
            window,
            at: beui::pos2(10.0, 20.0),
        }
    });
    view! {
        <beui::styled::List spacing=beui::styled::Space::None>
            <Window
                size={beui::vec2(80.0, 40.0)}
                placement
                role=beui::WindowRole::Popup
            >|_: WindowHandle|
                <Frame />
            </Window>
        </beui::styled::List>
    }
}

struct WindowedApp;

#[beui::reactive::component]
impl crate::BeuiApp for WindowedApp {
    #[beui::reactive::component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        use beui::dock::styled::{DockPane, DockTab, Docking, DockingLayout};
        let _ = editor;
        let layout = DockingLayout::<u64>::new();
        view! {
            <beui::styled::Root>
                <Docking layout>
                    <DockPane id="tabs">
                        <DockTab id=1u64 title="One">
                            <OverOwnWindow />
                        </DockTab>
                        <DockTab id=2u64 title="Two">
                            <beui::styled::Frame />
                        </DockTab>
                    </DockPane>
                </Docking>
            </beui::styled::Root>
        }
    }
}

fn show_windowed(
    session: &mut EditorSession,
    screens: &[(EditorRegion, u64)],
) -> Vec<WindowRequest> {
    let (placements, requests): (Vec<_>, Vec<_>) = screens
        .iter()
        .map(|(region, screen)| {
            let (placement, mut request) = dock_screen(*region, *screen, true);
            request.windows = true;
            (placement, request)
        })
        .unzip();
    session.place(&placements, &requests);
    let framed: Vec<_> = screens.iter().map(|(region, _)| (*region, 0)).collect();
    let mut asked = Vec::new();
    for _ in 0..2 {
        session.frame(&framed);
        let mut commit = block_plugin_api::FrameCommit {
            serial: 0,
            sizes: Vec::new(),
            reports: Vec::new(),
            windows: Vec::new(),
            children: Vec::new(),
            repaint_after_micros: None,
            animating: false,
            presented: None,
            window_requests: Vec::new(),
            docks: Vec::new(),
        };
        session.commit(&mut commit);
        asked.extend(commit.window_requests);
    }
    asked
}

fn opened_over(asked: &[WindowRequest]) -> Vec<(EditorRegion, EditorRegion)> {
    asked
        .iter()
        .filter_map(|request| match request {
            WindowRequest::Open(open) => match &open.placement {
                WindowPlacement::Over(over) => Some((open.opener.region, over.region.region)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn a_window_opened_in_a_docked_tab_body_opens_over_that_tab() {
    VIEWPORT.with(|held| held.set(beui::Vec2::ZERO));
    let mut session = session::<WindowedApp>(Uuid::new_v4());
    show_windowed(&mut session, &[(EditorRegion::Frame, 0)]);
    session
        .host()
        .receive_dock_updates(vec![hosted_tabs(&[1, 2], &[1], Some(1))]);
    show_windowed(&mut session, &[(EditorRegion::Frame, 0)]);

    let asked = show_windowed(
        &mut session,
        &[(EditorRegion::Frame, 0), (EditorRegion::Tab(1), 2)],
    );
    assert_eq!(
        opened_over(&asked),
        [(EditorRegion::Tab(1), EditorRegion::Tab(1))],
        "a window placed over its own window opens over the tab, not the undrawn frame"
    );
    assert_eq!(
        VIEWPORT.with(std::cell::Cell::get),
        beui::vec2(600.0, 300.0),
        "the viewport of a tab body is its tab's"
    );

    show_windowed(&mut session, &[(EditorRegion::Frame, 0)]);
    let asked = show_windowed(
        &mut session,
        &[(EditorRegion::Frame, 0), (EditorRegion::Tab(1), 3)],
    );
    assert_eq!(
        VIEWPORT.with(std::cell::Cell::get),
        beui::vec2(700.0, 300.0),
        "the body follows its tab to a new screen"
    );
    assert!(
        opened_over(&asked)
            .iter()
            .all(|opened| *opened == (EditorRegion::Tab(1), EditorRegion::Tab(1))),
        "{asked:?}"
    );
}
