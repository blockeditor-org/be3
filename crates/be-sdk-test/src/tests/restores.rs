//! What the tests of a joined dock's restore share: an editor whose dock has a stage and a bar
//! over it, built again after presenting, and the layouts the root reports for it.

use super::*;
use be_sdk_api::{
    DockAlign, DockBarEdge, DockBarPlacement, DockEdge, DockHosted, DockHostedView, DockItem,
    DockItemKind, DockLayout, DockMessage, DockPane as Pane, DockPlacedBar, DockRequest,
    DockTabPath, DockTree, DockUpdate, ViewId,
};
use beui::dock::{DockBar, DockBarPlacement as Placement, DockEdge as Edge};
use beui::dock::{DockPane, DockTab, Docking, DockingLayout};

const RESTORE_TYPE: Uuid = Uuid::from_u128(0x0b45);

pub(super) struct RestoreRoot;

thread_local! {
    /// Whether the dock built last declares its bar, which a test may set.
    pub(super) static BAR: std::cell::Cell<Option<Signal<bool>>> = const { std::cell::Cell::new(None) };
}

#[component]
impl BeuiRoot for RestoreRoot {
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
    let shown = create_signal(true);
    BAR.with(|bar| bar.set(Some(shown)));
    view! {
        <Docking layout>
            <DockPane id="stage" bare=true>
                <DockTab id=1u64 title="Stage">
                    <Frame />
                </DockTab>
            </DockPane>
            <Show value={move || shown.get()}>
                <DockBar
                    id=3u64
                    parent=1u64
                    placement=Placement::Over(Edge::Top, beui::dock::DockAlign::Center)
                >
                    <Frame width={Size::Custom(120.0)} height={Size::Custom(24.0)} />
                </DockBar>
            </Show>
        </Docking>
    }
}

pub(super) fn declared(messages: &[DockMessage]) -> Option<(u64, u64)> {
    messages.iter().rev().find_map(|message| match message {
        DockMessage::Declare(declare) => declare
            .declaration
            .as_ref()
            .map(|declared| (declared.tabs[0].tab.tab, declared.bars[0].tab.tab)),
        _ => None,
    })
}

pub(super) fn restored(messages: &[DockMessage]) -> bool {
    messages.iter().any(|message| {
        matches!(
            message,
            DockMessage::Request(request) if matches!(request.request, DockRequest::Restore(_))
        )
    })
}

pub(super) fn pane(key: &str) -> DockItem {
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
pub(super) fn hosted(stage: u64, bar: u64, placement: DockBarPlacement, side: bool, restored: u64) -> DockUpdate {
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

pub(super) fn kept(test: &BeuiTest<RestoreRoot>, display: Uuid) -> String {
    let root = test.content::<DisplaySettingsContent>(Some(display)).root();
    root.docks
        .values()
        .next()
        .cloned()
        .expect("the editor keeps its dock's layout on this device")
}

pub(super) fn over(edge: DockEdge, align: DockAlign) -> DockBarPlacement {
    DockBarPlacement::Over(DockBarEdge { edge, align })
}

/// An editor whose joined dock the root hosted with its bar at `moved`, which the device keeps
/// (`saved`), then built again after presenting: it declares its dock again (`stage`, `bar`)
/// and asks to restore the kept layout, which the root has not taken yet.
pub(super) struct Rebuilt {
    pub(super) test: BeuiTest<RestoreRoot>,
    pub(super) display: Uuid,
    pub(super) saved: String,
    pub(super) stage: u64,
    pub(super) bar: u64,
}

pub(super) fn rebuilt(moved: DockBarPlacement) -> Rebuilt {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_block_type(RESTORE_TYPE);
    let mut test =
        BeuiTest::<RestoreRoot>::new(OwnedEditor::new(host.clone(), Uuid::new_v4())).docked();
    let display = settings_here(&mut test, DisplaySettingsContent::default());
    let (stage, bar) =
        declared(&test.take_dock_messages()).expect("the docked editor declares its dock");
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
    Rebuilt {
        test,
        display,
        saved,
        stage,
        bar,
    }
}

test_editor!(RestoreRoot);
