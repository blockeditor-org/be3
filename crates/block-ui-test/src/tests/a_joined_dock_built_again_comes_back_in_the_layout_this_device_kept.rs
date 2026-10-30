use super::*;
use beui::dock::styled::{DockPane, DockTab, Docking, DockingLayout};
use beui::dock::unstyled::{DockBar, DockBarPlacement as Placement, DockEdge as Edge};
use block_plugin_api::{
    DockAlign, DockBarEdge, DockBarPlacement, DockEdge, DockHosted, DockHostedView, DockItem,
    DockItemKind, DockLayout, DockMessage, DockPane as Pane, DockPlacedBar, DockRequest,
    DockTabPath, DockTree, DockUpdate, ViewId,
};

const REBUILT_TYPE: Uuid = Uuid::from_u128(0x0b44);

struct RebuiltApp;

#[component]
impl BeuiApp for RebuiltApp {
    #[component]
    fn View(editor: Editor) -> Root {
        let presenting = editor.presenting();
        let docked = create_memo(move || !presenting.get());
        view! {
            <Root>
                <Layers>
                    <Show condition={docked}>
                        <Deck />
                    </Show>
                </Layers>
            </Root>
        }
    }
}

#[component]
fn Deck() -> NodeId {
    let layout = DockingLayout::<u64>::new();
    view! {
        <Docking layout>
            <DockPane id="stage" bare=true>
                <DockTab id=1u64 title="Stage">
                    <Frame />
                </DockTab>
            </DockPane>
            <DockBar
                id=3u64
                parent=1u64
                placement=Placement::Over(Edge::Top, beui::dock::unstyled::DockAlign::Center)
            >
                <Frame width={Size::Custom(120.0)} height={Size::Custom(24.0)} />
            </DockBar>
        </Docking>
    }
}

fn declarations(messages: &[DockMessage]) -> Vec<Option<(u64, u64)>> {
    messages
        .iter()
        .filter_map(|message| match message {
            DockMessage::Declare(declare) => Some(
                declare
                    .declaration
                    .as_ref()
                    .map(|declared| (declared.tabs[0].tab.tab, declared.bars[0].tab.tab)),
            ),
            _ => None,
        })
        .collect()
}

fn restores(messages: &[DockMessage]) -> Vec<DockLayout> {
    messages
        .iter()
        .filter_map(|message| match message {
            DockMessage::Request(request) => match &request.request {
                DockRequest::Restore(layout) => Some(layout.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn hosted(stage: u64, bar: u64, placement: DockBarPlacement) -> DockUpdate {
    DockUpdate::Hosted(DockHostedView {
        view: ViewId::MAIN,
        hosted: Some(DockHosted {
            layout: DockLayout {
                tree: DockTree {
                    items: vec![
                        DockItem {
                            depth: 0,
                            kind: DockItemKind::Pane(Pane {
                                key: "stage".to_owned(),
                                active: Some(0),
                                vertical: false,
                                icons_only: false,
                                sidebar: 180.0,
                                keep: false,
                                area: Some("stage".to_owned()),
                                empty: None,
                                bare: true,
                            }),
                        },
                        DockItem {
                            depth: 1,
                            kind: DockItemKind::Tab(DockTabPath::own(stage)),
                        },
                    ],
                    focus: None,
                },
                homes: Vec::new(),
                unlocked: Vec::new(),
                bars: vec![DockPlacedBar {
                    bar: DockTabPath::own(bar),
                    owner: Some(DockTabPath::own(stage)),
                    placement,
                    hidden: false,
                }],
                outside: Vec::new(),
            },
            tabs: vec![DockTabPath::own(stage)],
            focused: None,
            shown: None,
            visible: vec![DockTabPath::own(stage)],
            fullscreen: None,
            stacked: false,
            covers: Vec::new(),
        }),
    })
}

fn kept(test: &BeuiTest<RebuiltApp>, display: Uuid) -> String {
    let root = test.content::<DisplaySettingsContent>(Some(display)).root();
    root.docks
        .values()
        .next()
        .cloned()
        .expect("the editor keeps its dock's layout on this device")
}

#[test]
fn a_joined_dock_built_again_comes_back_in_the_layout_this_device_kept() {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_block_type(REBUILT_TYPE);
    let mut test =
        BeuiTest::<RebuiltApp>::new(OwnedEditor::new(host.clone(), Uuid::new_v4())).docked();
    let display = settings_here(&mut test, DisplaySettingsContent::default());
    let declared = declarations(&test.take_dock_messages());
    let (stage, bar) = declared
        .last()
        .copied()
        .flatten()
        .expect("the docked editor declares its dock");
    let moved = DockBarPlacement::Beside(DockBarEdge {
        edge: DockEdge::Left,
        align: DockAlign::Center,
    });
    test.receive_dock_updates(vec![hosted(stage, bar, moved)]);
    let saved = kept(&test, display);

    host.set_presenting(true);
    test.run();
    assert_eq!(
        declarations(&test.take_dock_messages()),
        [None],
        "a dock torn down withdraws its declaration"
    );
    test.receive_dock_updates(vec![DockUpdate::Hosted(DockHostedView {
        view: ViewId::MAIN,
        hosted: None,
    })]);

    host.set_presenting(false);
    test.run();
    let messages = test.take_dock_messages();
    let (stage, bar) = declarations(&messages)
        .last()
        .copied()
        .flatten()
        .expect("the dock built again declares again");
    let restored = restores(&messages);
    let restored = restored
        .last()
        .expect("the dock built again restores the layout kept for it");
    assert_eq!(restored.bars[0].bar, DockTabPath::own(bar));
    assert_eq!(
        restored.bars[0].placement, moved,
        "the moved bar comes back"
    );

    test.receive_dock_updates(vec![hosted(
        stage,
        bar,
        DockBarPlacement::Over(DockBarEdge {
            edge: DockEdge::Top,
            align: DockAlign::Center,
        }),
    )]);
    assert_eq!(
        kept(&test, display),
        saved,
        "the layout the root starts the rebuilt dock in is not kept over the saved one"
    );
}
