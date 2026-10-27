use super::*;
use block_editor_plugin::editor_session::EditorSession;
use block_editor_plugin::{Plugin, Waker};
use block_plugin_api::{
    EditorInstanceId, FrameChrome, FrameSpec, Rect, ScreenId, ScreenPlacement, ScreenRequest,
    TopBar, ViewportMetrics,
};

mod a_child_block_asks_the_host_for_the_frame_it_will_own;
mod a_docked_editor_builds_a_tab_body_only_where_the_host_shows_it_and_keeps_it;
mod a_docked_editor_declares_its_dock_and_draws_no_tab_body_in_its_frame;
mod a_docked_editor_follows_the_roots_focus_and_closes_what_it_closes;
mod a_focused_beui_child_gets_the_whole_frame_not_just_its_embedded_rect;
mod a_focused_beui_childs_reported_content_is_its_own_canvas_not_the_whole_view;
mod a_root_dock_hosts_the_dock_its_child_declares_in_place_of_the_childs_tab;
mod a_visible_editor_shows_its_user_as_active;
mod an_editor_not_docked_draws_its_dock_itself_and_declares_nothing;
mod an_open_beui_overlay_is_reported_over_the_child_it_covers;
mod the_frame_and_the_preview_of_one_block_both_load_its_content;

fn session<A: BeuiApp>(block_type: Uuid) -> EditorSession {
    let mut session =
        EditorSession::new(EditorInstanceId(0), Waker::default(), BeuiPlugin::<A>::open);
    session.connect(Uuid::new_v4(), block_type);
    session
}

fn frame(session: &mut EditorSession, content: Option<Rect>, top_bar: TopBar) {
    session.place(
        &[ScreenPlacement {
            screen: ScreenId(0),
            instance: EditorInstanceId(0),
            region: EditorRegion::Frame,
            surface: 1,
            width: 800,
            height: 600,
            scale_factor_millis: 1000,
            raster_millis: 1000,
        }],
        &[ScreenRequest {
            screen: ScreenId(0),
            instance: EditorInstanceId(0),
            region: EditorRegion::Frame,
            metrics: ViewportMetrics {
                logical: block_plugin_api::Size {
                    width: 800.0,
                    height: 600.0,
                },
                visible: block_plugin_api::Pos2::ZERO,
                pixel_width: 800,
                pixel_height: 600,
                scale_factor: 1.0,
                raster: 1.0,
            },
            frame: Some(FrameSpec {
                chrome: FrameChrome::Drawn,
                content,
                top_bar,
                dialog: None,
            }),
            windows: false,
            placement: None,
            docked: false,
        }],
    );
}

fn dock_screen(
    region: EditorRegion,
    screen: u64,
    docked: bool,
) -> (ScreenPlacement, ScreenRequest) {
    let width = 400 + 100 * screen as u32;
    let placement = ScreenPlacement {
        screen: ScreenId(screen),
        instance: EditorInstanceId(0),
        region,
        surface: screen as u32 + 1,
        width,
        height: 300,
        scale_factor_millis: 1000,
        raster_millis: 1000,
    };
    let request = ScreenRequest {
        screen: ScreenId(screen),
        instance: EditorInstanceId(0),
        region,
        metrics: ViewportMetrics {
            logical: block_plugin_api::Size {
                width: width as f32,
                height: 300.0,
            },
            visible: block_plugin_api::Pos2::ZERO,
            pixel_width: width,
            pixel_height: 300,
            scale_factor: 1.0,
            raster: 1.0,
        },
        frame: (region == EditorRegion::Frame).then(FrameSpec::default),
        windows: false,
        placement: None,
        docked: docked && region == EditorRegion::Frame,
    };
    (placement, request)
}

fn show_docked(session: &mut EditorSession, screens: &[(EditorRegion, u64)], docked: bool) {
    let (placements, requests): (Vec<_>, Vec<_>) = screens
        .iter()
        .map(|(region, screen)| dock_screen(*region, *screen, docked))
        .unzip();
    session.place(&placements, &requests);
    let framed: Vec<_> = screens.iter().map(|(region, _)| (*region, 0)).collect();
    session.frame(&framed);
    session.frame(&framed);
}

fn dock_messages(session: &mut EditorSession) -> Vec<block_plugin_api::DockMessage> {
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
    commit.docks
}

fn hosted_tabs(
    tabs: &[u64],
    visible: &[u64],
    focused: Option<u64>,
) -> block_plugin_api::DockUpdate {
    use block_plugin_api::{
        DockHosted, DockHostedRegion, DockItem, DockItemKind, DockLayout, DockPane, DockTabPath,
        DockTree, DockUpdate,
    };
    let mut items = vec![DockItem {
        depth: 0,
        kind: DockItemKind::Pane(DockPane {
            key: "tabs".to_owned(),
            active: focused
                .and_then(|focused| tabs.iter().position(|tab| *tab == focused))
                .map(|index| index as u32),
            vertical: false,
            icons_only: false,
            sidebar: 180.0,
            keep: false,
            area: None,
        }),
    }];
    items.extend(tabs.iter().map(|tab| DockItem {
        depth: 1,
        kind: DockItemKind::Tab(DockTabPath::own(*tab)),
    }));
    let own = |tabs: &[u64]| tabs.iter().copied().map(DockTabPath::own).collect();
    DockUpdate::Hosted(DockHostedRegion {
        region: EditorRegion::Frame,
        hosted: Some(DockHosted {
            layout: DockLayout {
                tree: DockTree {
                    items,
                    focus: focused.map(DockTabPath::own),
                },
                homes: Vec::new(),
            },
            tabs: own(tabs),
            focused: focused.map(DockTabPath::own),
            shown: None,
            visible: own(visible),
            fullscreen: None,
            stacked: false,
        }),
    })
}

thread_local! {
    static BODIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static CLOSED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static LAYOUT: std::cell::Cell<Option<beui::dock::unstyled::DockingLayout<u64>>> =
        const { std::cell::Cell::new(None) };
}

const DOCKED_CHILD: Uuid = Uuid::from_u128(11);
const DOCKED_CHILD_TYPE: Uuid = Uuid::from_u128(12);

#[beui::reactive::component]
fn CountedBody(editor: crate::Editor) -> beui::NodeId {
    use crate::ChildBlock;
    use beui::reactive::*;
    BODIES.with(|built| built.set(built.get() + 1));
    view! {
        <beui::styled::List spacing=beui::styled::Space::None>
            <ChildBlock
                editor={editor}
                block={Some(crate::ChildTarget::new(DOCKED_CHILD, DOCKED_CHILD_TYPE))}
                mode=block_plugin_api::ChildMode::Live
                own_frame=true
                on_state={move |_: crate::ChildState| {}}
            />
        </beui::styled::List>
    }
}

struct DockedApp;

#[beui::reactive::component]
impl crate::BeuiApp for DockedApp {
    #[beui::reactive::component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        use beui::dock::styled::{DockPane, DockTab, Docking, DockingLayout};
        use beui::reactive::*;
        let layout = DockingLayout::<u64>::new();
        LAYOUT.with(|held| held.set(Some(layout)));
        view! {
            <beui::styled::Root>
                <Docking layout>
                    <DockPane id="tabs">
                        <DockTab id=1u64 title="One">
                            <CountedBody editor />
                        </DockTab>
                        <DockTab
                            id=2u64
                            title="Two"
                            on_close={move || CLOSED.with(|closed| closed.set(true))}
                        >
                            <beui::styled::Frame />
                        </DockTab>
                    </DockPane>
                </Docking>
            </beui::styled::Root>
        }
    }
}

fn docked_session() -> EditorSession {
    BODIES.with(|built| built.set(0));
    CLOSED.with(|closed| closed.set(false));
    session::<DockedApp>(Uuid::new_v4())
}
