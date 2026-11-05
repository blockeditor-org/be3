use super::*;
use be_sdk::editor_session::EditorSession;
use be_sdk::{App, Waker};
use be_sdk_api::{
    EditorInstanceId, FrameChrome, FrameSpec, ParentView, Rect, ScreenId, ScreenPlacement,
    ScreenRequest, TopBar, View, ViewId, ViewPlacement, ViewportMetrics,
};

mod a_child_block_asks_the_host_for_the_frame_it_will_own;
mod a_docked_editor_builds_a_tab_body_only_where_the_host_shows_it_and_keeps_it;
mod a_docked_editor_declares_its_dock_and_draws_no_tab_body_in_its_frame;
mod a_docked_editor_follows_the_roots_focus_and_closes_what_it_closes;
mod a_docked_editors_bar_declares_its_size_and_asks_the_root_to_place_and_move_it;
mod a_docked_editors_nested_dock_and_dialog_join_its_declaration;
mod a_docked_editors_own_dock_bar_crosses_with_no_owner;
mod a_docked_editors_palette_opens_over_the_tab_whose_menu_asked;
mod a_docked_editors_popover_is_an_anchored_view_of_its_tab;
mod a_focused_beui_child_gets_the_whole_frame_not_just_its_embedded_rect;
mod a_focused_beui_childs_reported_content_is_its_own_canvas_not_the_whole_view;
mod a_nested_docks_own_bar_is_declared_with_the_tab_its_group_shows;
mod a_root_dock_hosts_the_dock_its_child_declares_in_place_of_the_childs_tab;
mod a_root_draws_a_joined_editors_bar_and_moves_it_as_the_editor_asks;
mod a_root_focuses_the_joined_empty_pane_the_host_focuses;
mod a_root_gives_a_joined_editors_dock_bar_to_the_tab_hosting_it;
mod a_root_gives_a_tab_two_levels_down_its_owners_menu_block_and_dialog;
mod a_root_hosts_a_childs_dock_again_after_the_child_withdraws_and_redeclares_it;
mod a_root_places_bars_of_one_kind_together_and_others_apart;
mod a_root_shows_the_joined_tab_whose_body_the_host_focuses;
mod a_visible_editor_shows_its_user_as_active;
mod a_window_opened_in_a_docked_tab_body_opens_over_that_tab;
mod an_editor_not_docked_draws_its_dock_itself_and_declares_nothing;
mod an_open_beui_overlay_is_reported_over_the_child_it_covers;
mod each_frame_of_a_session_moves_its_clock_on_by_one_step;
mod the_frame_and_the_preview_of_one_block_both_load_its_content;

fn session<A: BeuiRoot>(block_type: Uuid) -> EditorSession {
    let mut session = EditorSession::new(EditorInstanceId(0), Waker::default(), BeuiApp::<A>::open);
    session.connect(Uuid::new_v4(), block_type);
    session
}

fn frame(session: &mut EditorSession, content: Option<Rect>, top_bar: TopBar) {
    session.place(
        &[ScreenPlacement {
            screen: ScreenId(0),
            instance: EditorInstanceId(0),
            view: ViewId::MAIN,
            surface: 1,
            width: 800,
            height: 600,
            scale_factor_millis: 1000,
            raster_millis: 1000,
        }],
        &[ScreenRequest {
            screen: ScreenId(0),
            instance: EditorInstanceId(0),
            view: ViewId::MAIN,
            placement: ViewPlacement::Parent(ParentView::Main),
            metrics: ViewportMetrics {
                logical: be_sdk_api::Size {
                    width: 800.0,
                    height: 600.0,
                },
                visible: be_sdk_api::Pos2::ZERO,
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
                anchor: None,
            }),
            windows: false,
            host_place: None,
            origin: None,
            docked: false,
            anchor_space: None,
            fit: false,
        }],
    );
}

fn dock_screen(shown: View, screen: u64, docked: bool) -> (ScreenPlacement, ScreenRequest) {
    let width = 400 + 100 * screen as u32;
    let placement = ScreenPlacement {
        screen: ScreenId(screen),
        instance: EditorInstanceId(0),
        view: shown.id,
        surface: screen as u32 + 1,
        width,
        height: 300,
        scale_factor_millis: 1000,
        raster_millis: 1000,
    };
    let request = ScreenRequest {
        screen: ScreenId(screen),
        instance: EditorInstanceId(0),
        view: shown.id,
        placement: shown.placement,
        metrics: ViewportMetrics {
            logical: be_sdk_api::Size {
                width: width as f32,
                height: 600.0,
            },
            visible: be_sdk_api::Pos2::ZERO,
            pixel_width: width,
            pixel_height: 600,
            scale_factor: 1.0,
            raster: 1.0,
        },
        frame: shown.is_main().then(FrameSpec::default),
        windows: false,
        host_place: None,
        origin: None,
        docked: docked && shown.is_main(),
        anchor_space: None,
        fit: false,
    };
    (placement, request)
}

fn show_docked(session: &mut EditorSession, screens: &[(View, u64)], docked: bool) {
    let (placements, requests): (Vec<_>, Vec<_>) = screens
        .iter()
        .map(|(shown, screen)| dock_screen(*shown, *screen, docked))
        .unzip();
    session.place(&placements, &requests);
    let framed: Vec<_> = screens.iter().map(|(shown, _)| (shown.id, 0)).collect();
    session.frame(&framed);
    session.frame(&framed);
}

fn dock_messages(session: &mut EditorSession) -> Vec<be_sdk_api::DockMessage> {
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
    commit.docks
}

fn hosted_tabs(tabs: &[u64], visible: &[u64], focused: Option<u64>) -> be_sdk_api::DockUpdate {
    use be_sdk_api::{
        DockHosted, DockHostedView, DockItem, DockItemKind, DockLayout, DockPane, DockTabPath,
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
            empty: None,
            bare: false,
            hide_empty_bar: false,
        }),
    }];
    items.extend(tabs.iter().map(|tab| DockItem {
        depth: 1,
        kind: DockItemKind::Tab(DockTabPath::own(*tab)),
    }));
    let own = |tabs: &[u64]| tabs.iter().copied().map(DockTabPath::own).collect();
    DockUpdate::Hosted(DockHostedView {
        view: ViewId::MAIN,
        hosted: Some(DockHosted {
            layout: DockLayout {
                tree: DockTree {
                    items,
                    focus: focused.map(DockTabPath::own),
                },
                homes: Vec::new(),
                bars: Vec::new(),
                outside: Vec::new(),
                unlocked: Vec::new(),
                compact: Vec::new(),
                auto_hidden: Vec::new(),
            },
            tabs: own(tabs),
            focused: focused.map(DockTabPath::own),
            shown: None,
            visible: own(visible),
            fullscreen: None,
            mode: be_sdk_api::DockMode::Tiled,
            covers: Vec::new(),
        }),
    })
}

thread_local! {
    static BODIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static CLOSED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static DOCK_TABS: std::cell::RefCell<Vec<(Option<Uuid>, beui::reactive::Memo<Vec<beui::reactive::MenuNode>>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
    static LAYOUT: std::cell::Cell<Option<beui::dock::DockingLayout<u64>>> =
        const { std::cell::Cell::new(None) };
    static BAR: std::cell::Cell<Option<beui::dock::DockBarControl>> =
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
                mode=be_sdk_api::ChildMode::Live
                own_frame=true
                on_state={move |_: crate::ChildState| {}}
                docked_tab={RenderFn::new(|docked: crate::ChildDockTab| {
                    DOCK_TABS.with(|tabs| tabs.borrow_mut().push((docked.owner, docked.menu)));
                    beui::dock::dock_menu(docked.menu);
                    docked.body
                })}
            />
        </beui::styled::List>
    }
}

struct DockedRoot;

#[beui::reactive::component]
impl crate::BeuiRoot for DockedRoot {
    #[beui::reactive::component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        use beui::dock::{DockPane, DockTab, Docking, DockingLayout};
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
    DOCK_TABS.with(|tabs| tabs.borrow_mut().clear());
    BODIES.with(|built| built.set(0));
    CLOSED.with(|closed| closed.set(false));
    session::<DockedRoot>(Uuid::new_v4())
}

#[beui::reactive::component]
fn BarProbe() -> beui::NodeId {
    use beui::reactive::*;
    let bar = beui::dock::use_dock_bar();
    BAR.with(|held| held.set(bar));
    let vertical =
        create_memo(move || bar.is_some_and(|bar| bar.direction() == beui::Direction::Vertical));
    let length = |length: f32| beui::styled::Length::Size(beui::styled::Size::Custom(length));
    let width = create_memo(move || length(if vertical.get() { 24.0 } else { 120.0 }));
    let height = create_memo(move || length(if vertical.get() { 120.0 } else { 24.0 }));
    view! {
        <beui::styled::Frame width={width} height={height} />
    }
}

struct BarredRoot;

#[beui::reactive::component]
impl crate::BeuiRoot for BarredRoot {
    #[beui::reactive::component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        use beui::dock::{DockBar, DockBarPlacement, DockEdge};
        use beui::dock::{DockPane, DockTab, Docking, DockingLayout};
        use beui::reactive::*;
        let _ = editor;
        let layout = DockingLayout::<u64>::new();
        LAYOUT.with(|held| held.set(Some(layout)));
        view! {
            <beui::styled::Root>
                <Docking layout>
                    <DockPane id="stage" bare=true>
                        <DockTab id=1u64 title="Stage">
                            <beui::styled::Frame />
                        </DockTab>
                    </DockPane>
                    <DockBar
                        id=3u64
                        parent=1u64
                        placement=DockBarPlacement::Over(DockEdge::Top, beui::dock::DockAlign::Center)
                    >
                        <BarProbe />
                    </DockBar>
                </Docking>
            </beui::styled::Root>
        }
    }
}

fn barred_session() -> EditorSession {
    BAR.with(|held| held.set(None));
    session::<BarredRoot>(Uuid::new_v4())
}

struct DockBarredRoot;

#[beui::reactive::component]
impl crate::BeuiRoot for DockBarredRoot {
    #[beui::reactive::component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        use beui::dock::{DockBar, DockBarPlacement, DockEdge};
        use beui::dock::{DockPane, DockTab, Docking, DockingLayout};
        use beui::reactive::*;
        let _ = editor;
        let layout = DockingLayout::<u64>::new();
        LAYOUT.with(|held| held.set(Some(layout)));
        view! {
            <beui::styled::Root>
                <Docking layout>
                    <DockPane id="stage" bare=true>
                        <DockTab id=1u64 title="Stage">
                            <beui::styled::Frame />
                        </DockTab>
                    </DockPane>
                    <DockBar
                        id=3u64
                        placement=DockBarPlacement::Beside(DockEdge::Bottom, beui::dock::DockAlign::Center)
                    >
                        <BarProbe />
                    </DockBar>
                </Docking>
            </beui::styled::Root>
        }
    }
}

fn dock_barred_session() -> EditorSession {
    BAR.with(|held| held.set(None));
    session::<DockBarredRoot>(Uuid::new_v4())
}

struct TwoPaneRoot;

#[beui::reactive::component]
impl crate::BeuiRoot for TwoPaneRoot {
    #[beui::reactive::component]
    fn View(editor: crate::Editor) -> beui::styled::Root {
        use beui::dock::{DockPane, DockSplit, DockTab, Docking, DockingLayout, SplitSize};
        use beui::reactive::*;
        let layout = DockingLayout::<u64>::new();
        LAYOUT.with(|held| held.set(Some(layout)));
        view! {
            <beui::styled::Root>
                <Docking layout>
                    <DockSplit id="root" size={SplitSize::Fraction(0.5)}>
                        <DockPane id="joined">
                            <DockTab id=1u64 title="One">
                                <CountedBody editor />
                            </DockTab>
                        </DockPane>
                        <DockPane id="other">
                            <DockTab id=2u64 title="Two">
                                <beui::styled::Frame />
                            </DockTab>
                        </DockPane>
                    </DockSplit>
                </Docking>
            </beui::styled::Root>
        }
    }
}

/// Wide enough that the dock tiles both panes rather than stacking the focused one.
const WIDE: &[(View, u64)] = &[(View::MAIN, 4)];

/// A root whose tab 1 holds a child that joins `joined_dock` to the root's dock, with the focus
/// then given to tab 2 in the other pane.
fn joined_then_away() -> (EditorSession, beui::dock::DockingLayout<u64>) {
    use be_sdk_api::{DockDeclaredChild, DockUpdate};
    DOCK_TABS.with(|tabs| tabs.borrow_mut().clear());
    let mut session = session::<TwoPaneRoot>(Uuid::new_v4());
    show_docked(&mut session, WIDE, false);
    let child = session.placed_children(ViewId::MAIN)[0].child;
    dock_messages(&mut session);
    session
        .host()
        .receive_dock_updates(vec![DockUpdate::Child(DockDeclaredChild {
            child,
            declaration: Some(joined_dock()),
        })]);
    show_docked(&mut session, WIDE, false);
    show_docked(&mut session, WIDE, false);
    dock_messages(&mut session);
    let layout = LAYOUT
        .with(std::cell::Cell::get)
        .expect("the app made its layout");
    layout.show(&2);
    show_docked(&mut session, WIDE, false);
    assert_eq!(
        layout.focused(),
        Some(2),
        "the other pane's tab has the focus"
    );
    (session, layout)
}

/// The child the root placed for the joined dock's view of `tab`: a tab's body, or an empty pane's.
fn joined_body(session: &EditorSession, tab: u64) -> be_sdk_api::ChildId {
    session
        .placed_children(ViewId::MAIN)
        .iter()
        .find(|placed| {
            matches!(&placed.content, be_sdk_api::ChildContent::DockTab(path) if path.tab == tab)
        })
        .map(|placed| placed.child)
        .unwrap_or_else(|| panic!("the root places the joined dock's view of {tab}"))
}

/// A joined dock as a session's: a files pane holding tab 5 beside an editors pane with no tabs,
/// whose empty view is 9, both locked as the workspace's are.
fn joined_dock() -> be_sdk_api::DockDeclaration {
    use be_sdk_api::{
        DockDeclaration, DockDirection, DockItem, DockItemKind, DockPane, DockSplit, DockSplitSize,
        DockTab, DockTabPath, DockTree,
    };
    let pane = |key: &str, empty: Option<DockTabPath>| DockItem {
        depth: 1,
        kind: DockItemKind::Pane(DockPane {
            key: key.to_owned(),
            active: None,
            vertical: false,
            icons_only: false,
            sidebar: 180.0,
            keep: true,
            area: Some(key.to_owned()),
            empty,
            bare: false,
            hide_empty_bar: false,
        }),
    };
    DockDeclaration {
        tree: DockTree {
            items: vec![
                DockItem {
                    depth: 0,
                    kind: DockItemKind::Split(DockSplit {
                        key: "main".to_owned(),
                        direction: DockDirection::Horizontal,
                        size: DockSplitSize::Fraction(0.5),
                    }),
                },
                pane("files", None),
                DockItem {
                    depth: 2,
                    kind: DockItemKind::Tab(DockTabPath::own(5)),
                },
                pane("editors", Some(DockTabPath::own(9))),
            ],
            focus: None,
        },
        tabs: vec![DockTab {
            tab: DockTabPath::own(5),
            key: "5".to_owned(),
            title: "Files".to_owned(),
            glyph: String::new(),
            closable: false,
            back_closes: true,
            menu: Vec::new(),
        }],
        dialogs: Vec::new(),
        bars: Vec::new(),
    }
}
