use super::*;
use beui::dock::styled::{DockPane, DockTab, Docking, DockingLayout};
use beui::dock::unstyled::{DockBar, DockBarPlacement as Placement, DockEdge as Edge};
use block_editor_beui::be_block::{DisplaySettings, DisplaySettingsContent};
use block_plugin_api::{
    DockBarPlacement, DockEdge, DockHosted, DockHostedView, DockItem, DockItemKind, DockLayout,
    DockMessage, DockOutside, DockOutsidePane, DockOutsidePlace, DockPane as Pane, DockPlacedBar,
    DockRequest, DockTabPath, DockTree, DockUpdate, ViewId,
};

const BARRED_TYPE: Uuid = Uuid::from_u128(0x0b42);

struct BarredApp;

#[component]
impl BeuiApp for BarredApp {
    #[component]
    fn View(editor: Editor) -> Root {
        let _ = editor;
        let layout = DockingLayout::<u64>::new();
        view! {
            <Root>
                <Docking layout>
                    <DockPane id="stage" bare=true>
                        <DockTab id=1u64 title="Stage">
                            <Frame />
                        </DockTab>
                    </DockPane>
                    <DockBar id=3u64 parent=1u64 placement=Placement::Over(Edge::Top)>
                        <Frame width={Size::Custom(120.0)} height={Size::Custom(24.0)} />
                    </DockBar>
                </Docking>
            </Root>
        }
    }
}

fn barred(display: DisplaySettingsContent) -> (BeuiTest<BarredApp>, Uuid) {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_block_type(BARRED_TYPE);
    let mut test = BeuiTest::<BarredApp>::new(OwnedEditor::new(host, Uuid::new_v4())).docked();
    let held = settings_here(&mut test, display);
    (test, held)
}

fn ids(test: &mut BeuiTest<BarredApp>) -> (u64, u64) {
    let declaration = test
        .take_dock_messages()
        .into_iter()
        .filter_map(|message| match message {
            DockMessage::Declare(declare) => declare.declaration,
            _ => None,
        })
        .last()
        .expect("the docked editor declares its dock");
    (declaration.tabs[0].tab.tab, declaration.bars[0].tab.tab)
}

fn hosted(stage: u64, bar: u64, placement: DockBarPlacement) -> DockUpdate {
    DockUpdate::Hosted(DockHostedView {
        view: ViewId::MAIN,
        hosted: Some(DockHosted {
            layout: DockLayout {
                tree: DockTree {
                    items: vec![DockItem {
                        depth: 0,
                        kind: DockItemKind::Pane(Pane {
                            key: "stage".to_owned(),
                            active: None,
                            vertical: false,
                            icons_only: false,
                            sidebar: 180.0,
                            keep: false,
                            area: Some("stage".to_owned()),
                            empty: None,
                            bare: true,
                        }),
                    }],
                    focus: None,
                },
                homes: Vec::new(),
                unlocked: Vec::new(),
                bars: vec![DockPlacedBar {
                    bar: DockTabPath::own(bar),
                    owner: DockTabPath::own(stage),
                    placement,
                }],
                outside: vec![DockOutside {
                    tab: DockTabPath::own(stage),
                    place: DockOutsidePlace::Pane(DockOutsidePane {
                        key: "files".to_owned(),
                        index: 2,
                    }),
                }],
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

fn saved(test: &BeuiTest<BarredApp>, display: Uuid) -> (String, String) {
    let root = test.content::<DisplaySettingsContent>(Some(display)).root();
    root.docks
        .iter()
        .map(|(key, layout)| (key.clone(), layout.clone()))
        .next()
        .expect("the editor keeps its dock's layout on this device")
}

fn restored(test: &mut BeuiTest<BarredApp>) -> DockLayout {
    test.take_dock_messages()
        .into_iter()
        .filter_map(|message| match message {
            DockMessage::Request(request) => match request.request {
                DockRequest::Restore(layout) => Some(layout),
                _ => None,
            },
            _ => None,
        })
        .last()
        .expect("the editor restores the layout it kept")
}

#[test]
fn a_joined_docks_layout_is_kept_on_this_device_and_every_instance_follows_it() {
    let (mut first, display) = barred(DisplaySettingsContent::default());
    let (stage, bar) = ids(&mut first);
    first.receive_dock_updates(vec![hosted(
        stage,
        bar,
        DockBarPlacement::Beside(DockEdge::Left),
    )]);
    let (key, layout) = saved(&first, display);
    assert!(key.starts_with(&BARRED_TYPE.simple().to_string()));

    let kept = first.content::<DisplaySettingsContent>(Some(display));
    let (mut reopened, held) = barred(kept);
    let layout_back = restored(&mut reopened);
    assert_eq!(
        layout_back.bars[0].placement,
        DockBarPlacement::Beside(DockEdge::Left),
        "a moved bar survives reopening"
    );
    assert_eq!(
        layout_back.outside[0].place,
        DockOutsidePlace::Pane(DockOutsidePane {
            key: "files".to_owned(),
            index: 2,
        }),
        "a moved-out tab goes back where it went"
    );
    assert_eq!(layout_back.outside[0].tab.tab, stage);

    first.receive_dock_updates(vec![hosted(
        stage,
        bar,
        DockBarPlacement::Over(DockEdge::Bottom),
    )]);
    let (_, changed) = saved(&first, display);
    assert_ne!(changed, layout);
    reopened.edit::<DisplaySettingsContent>(
        Some(held),
        &DisplaySettings::set_dock_layout(&key, Some(changed)),
    );
    assert_eq!(
        restored(&mut reopened).bars[0].placement,
        DockBarPlacement::Over(DockEdge::Bottom),
        "an open instance follows the layout another one kept"
    );
}
