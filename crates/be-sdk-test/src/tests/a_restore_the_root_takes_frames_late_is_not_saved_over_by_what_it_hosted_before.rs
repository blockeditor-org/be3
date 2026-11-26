use super::*;
use be_sdk_api::{
    DockAlign, DockBarEdge, DockBarPlacement, DockEdge, DockHosted, DockHostedView, DockItem,
    DockItemKind, DockLayout, DockMessage, DockPane as Pane, DockPlacedBar, DockRequest,
    DockTabPath, DockTree, DockUpdate, ViewId,
};
use beui::dock::{DockBar, DockBarPlacement as Placement, DockEdge as Edge};
use beui::dock::{DockPane, DockTab, Docking, DockingLayout};

const LATE_TYPE: Uuid = Uuid::from_u128(0x0b45);

struct LateRoot;

#[component]
impl BeuiRoot for LateRoot {
    #[component]
    fn View(editor: Editor) -> Root {
        let presenting = editor.presenting();
        view! {
            <Root>
                <Layers>
                    <Show value={move || !presenting.get()}>
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
                placement=Placement::Over(Edge::Top, beui::dock::DockAlign::Center)
            >
                <Frame width={Size::Custom(120.0)} height={Size::Custom(24.0)} />
            </DockBar>
        </Docking>
    }
}

fn declared(messages: &[DockMessage]) -> Option<(u64, u64)> {
    messages.iter().rev().find_map(|message| match message {
        DockMessage::Declare(declare) => declare
            .declaration
            .as_ref()
            .map(|declared| (declared.tabs[0].tab.tab, declared.bars[0].tab.tab)),
        _ => None,
    })
}

fn restored(messages: &[DockMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message,
            DockMessage::Request(request) if matches!(request.request, DockRequest::Restore(_))
        )
    })
}

fn pane(key: &str) -> DockItem {
    DockItem {
        depth: 0,
        kind: DockItemKind::Pane(Pane {
            key: key.to_owned(),
            active: Some(0),
            vertical: false,
            icons_only: false,
            sidebar: 180.0,
            keep: false,
            area: Some(key.to_owned()),
            empty: None,
            bare: true,
            hide_empty_bar: false,
        }),
    }
}

/// The root's dock with the stage and its bar at `placement`, and with a side pane before it
/// when `side`: a pane the editor declared before, as the canvas declares its inspector until
/// its content says its profile has none. `restored` counts the restores the root has taken.
fn hosted(stage: u64, bar: u64, placement: DockBarPlacement, side: bool, restored: u64) -> DockUpdate {
    let mut items = Vec::new();
    if side {
        items.push(pane("side"));
    }
    items.push(pane("stage"));
    items.push(DockItem {
        depth: 1,
        kind: DockItemKind::Tab(DockTabPath::own(stage)),
    });
    DockUpdate::Hosted(DockHostedView {
        view: ViewId::MAIN,
        hosted: Some(DockHosted {
            layout: DockLayout {
                tree: DockTree { items, focus: None },
                homes: Vec::new(),
                unlocked: Vec::new(),
                compact: Vec::new(),
                auto_hidden: Vec::new(),
                bars: vec![DockPlacedBar {
                    bar: DockTabPath::own(bar),
                    owner: Some(DockTabPath::own(stage)),
                    placement,
                    hidden: false,
                    pane: None,
                }],
                outside: Vec::new(),
            },
            tabs: vec![DockTabPath::own(stage)],
            focused: None,
            shown: None,
            visible: vec![DockTabPath::own(stage)],
            fullscreen: None,
            mode: be_sdk_api::DockMode::Tiled,
            covers: Vec::new(),
            restored,
        }),
    })
}

fn kept(test: &BeuiTest<LateRoot>, display: Uuid) -> String {
    let root = test.content::<DisplaySettingsContent>(Some(display)).root();
    root.docks
        .values()
        .next()
        .cloned()
        .expect("the editor keeps its dock's layout on this device")
}

fn over(edge: DockEdge, align: DockAlign) -> DockBarPlacement {
    DockBarPlacement::Over(DockBarEdge { edge, align })
}

#[test]
fn a_restore_the_root_takes_frames_late_is_not_saved_over_by_what_it_hosted_before() {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_block_type(LATE_TYPE);
    let mut test =
        BeuiTest::<LateRoot>::new(OwnedEditor::new(host.clone(), Uuid::new_v4())).docked();
    let display = settings_here(&mut test, DisplaySettingsContent::default());
    let (stage, bar) =
        declared(&test.take_dock_messages()).expect("the docked editor declares its dock");
    let moved = DockBarPlacement::Beside(DockBarEdge {
        edge: DockEdge::Left,
        align: DockAlign::Center,
    });
    test.receive_dock_updates(vec![hosted(stage, bar, moved, false, 0)]);
    let saved = kept(&test, display);

    host.set_presenting(true);
    test.run();
    test.take_dock_messages();
    test.receive_dock_updates(vec![DockUpdate::Hosted(DockHostedView {
        view: ViewId::MAIN,
        hosted: None,
    })]);

    host.set_presenting(false);
    test.run();
    let messages = test.take_dock_messages();
    let (stage, bar) = declared(&messages).expect("the dock built again declares again");
    assert!(
        restored(&messages),
        "the dock built again restores the layout kept for it"
    );

    // The root's answers come a frame late: before it takes the restore, it reports the dock
    // as it hosted each declaration it heard, the bar where the declaration puts it.
    let declared_at = over(DockEdge::Top, DockAlign::Center);
    test.receive_dock_updates(vec![hosted(stage, bar, declared_at, true, 0)]);
    test.receive_dock_updates(vec![hosted(stage, bar, declared_at, false, 0)]);
    assert_eq!(
        kept(&test, display),
        saved,
        "no layout the root hosted before it took the restore is kept over the saved one"
    );

    test.receive_dock_updates(vec![hosted(stage, bar, moved, false, 1)]);
    assert_eq!(
        kept(&test, display),
        saved,
        "the restored layout is the one kept"
    );

    let dragged = over(DockEdge::Bottom, DockAlign::Start);
    test.receive_dock_updates(vec![hosted(stage, bar, dragged, false, 1)]);
    assert_ne!(
        kept(&test, display),
        saved,
        "once restored, a bar moved again is kept"
    );
}

test_editor!(LateRoot);
