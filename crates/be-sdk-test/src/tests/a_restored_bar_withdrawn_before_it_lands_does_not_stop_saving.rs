use super::*;
use be_sdk_api::{
    DockAlign, DockBarEdge, DockBarPlacement, DockEdge, DockHosted, DockHostedView, DockItem,
    DockItemKind, DockLayout, DockMessage, DockPane as Pane, DockPlacedBar, DockRequest,
    DockTabPath, DockTree, DockUpdate, ViewId,
};
use beui::dock::{DockBar, DockBarPlacement as Placement, DockEdge as Edge};
use beui::dock::{DockPane, DockTab, Docking, DockingLayout};

const STUCK_TYPE: Uuid = Uuid::from_u128(0x0b46);

struct StuckRoot;
thread_local! {
    /// Whether the dock declares its bar, set by the test.
    static BAR: std::cell::Cell<Option<Signal<bool>>> = const { std::cell::Cell::new(None) };
}


#[component]
impl BeuiRoot for StuckRoot {
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
    let shown = create_signal(true);
    BAR.with(|bar| bar.set(Some(shown)));
    let layout = DockingLayout::<u64>::new();
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
/// its content says its profile has none.
fn hosted(stage: u64, bar: u64, placement: DockBarPlacement, side: bool) -> DockUpdate {
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
        }),
    })
}

fn kept(test: &BeuiTest<StuckRoot>, display: Uuid) -> String {
    let root = test.content::<DisplaySettingsContent>(Some(display)).root();
    root.docks
        .values()
        .next()
        .cloned()
        .expect("the editor keeps its dock's layout on this device")
}

#[test]
fn a_restored_bar_withdrawn_before_it_lands_does_not_stop_saving() {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_block_type(STUCK_TYPE);
    let mut test =
        BeuiTest::<StuckRoot>::new(OwnedEditor::new(host.clone(), Uuid::new_v4())).docked();
    let display = settings_here(&mut test, DisplaySettingsContent::default());
    let (stage, bar) =
        declared(&test.take_dock_messages()).expect("the docked editor declares its dock");
    let moved = DockBarPlacement::Beside(DockBarEdge {
        edge: DockEdge::Left,
        align: DockAlign::Center,
    });
    test.receive_dock_updates(vec![hosted(stage, bar, moved, false)]);
    kept(&test, display);

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

    // The bar is withdrawn before the root takes the restore: the root never reports it.
    BAR.with(|flag| flag.get().unwrap().set(false));
    test.run();
    test.take_dock_messages();
    // The root reports the stage alone, then the person adds a side pane: a change of their own.
    let mut only_stage = hosted(stage, bar, moved, false);
    let mut with_side = hosted(stage, bar, moved, true);
    for update in [&mut only_stage, &mut with_side] {
        if let DockUpdate::Hosted(view) = update {
            view.hosted.as_mut().unwrap().layout.bars.clear();
        }
    }
    test.receive_dock_updates(vec![only_stage]);
    let before = kept(&test, display);
    test.receive_dock_updates(vec![with_side]);
    assert_ne!(
        kept(&test, display),
        before,
        "a change the person makes after the restored bar is withdrawn is kept"
    );
}

test_editor!(StuckRoot);
