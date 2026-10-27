use std::cell::Cell;
use workspace_ui_types::EditorViewContent;

use block_editor_beui::be_block::{BlockContent, FILES_EDITOR, SettingsContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::dock::styled::{
    DockMode, DockPane, DockSplit, DockTab, Docking, TabId, use_dock_tab,
};
use block_editor_beui::beui::icons::{ICON_FOLDER, ICON_ROTATE_CCW_CLOCK, ICON_SETTINGS};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::root_settings::own_settings_blocks;
use block_editor_beui::{
    ChildBlock, ChildBlockHandle, ChildMode, ChildTarget, Editor, HostPanel, HostWindowId, Loaded,
    TopBar,
};
use block_shell::{
    AttachedWindow, BlockTab, FILES, Failure, FloatingWindow, PanelStatus, PanelWindow, REVISIONS,
    RenameDialog, RevisionHistoryPanel, SETTINGS, WindowTab, Workspace, WorkspaceDialogs,
};
use uuid::Uuid;

const FILES_SHARE: f32 = 0.22;
const PANEL_PADDING: Space = Space::Lg;
const PANEL_SPACING: Space = Space::Sm;

pub struct WorkspaceUiApp;

#[component]
impl block_editor_beui::BeuiApp for WorkspaceUiApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <WorkspaceShell editor={editor} />
            </Root>
        }
    }
}

#[component]
fn WorkspaceShell(editor: Editor) -> NodeId {
    let workspace = Workspace::new(editor, true, false);
    view! {
        <Loaded label="Loading the workspace">|| {
            let _layout = editor.view_content();
            view! {
                <Container>|_|
                    <WorkspaceBody workspace />
                </Container>
            }
        }</Loaded>
    }
}

#[component]
pub(crate) fn WorkspaceBody(workspace: Workspace) -> NodeId {
    let layout = workspace.layout();
    let mode = layout.mode();
    let was_stacked = Cell::new(false);
    create_effect(move || {
        let now = mode.get() == DockMode::Stacked;
        if was_stacked.replace(now) != now {
            untrack(|| workspace.set_phone(now));
        }
    });
    let surface = NodeRef::new();
    workspace.editor().content(&surface);
    let failure = workspace.error();
    let failed = create_memo(move || failure.get().is_some());
    let reason = create_memo(move || failure.get().unwrap_or_default());
    let block_tabs = workspace.block_tabs();
    let top_windows = workspace.windows();
    let opens = opens_windows();
    let tabbed = create_memo(move || match opens.get() {
        true => Vec::new(),
        false => top_windows.get(),
    });
    let floating = create_memo(move || match opens.get() {
        true => top_windows.get(),
        false => Vec::new(),
    });
    let placeholder = RenderFn::new(|_: HostWindowId| {
        view! {
            <Frame />
        }
    });
    let dialog_windows = workspace.dialogs();
    let rename_dialogs = workspace.rename_dialogs();
    let panels = workspace.panels();
    view! {
        <Frame @node_ref={&surface}>
            <List spacing=Space::None>
                <Failure failed={failed} reason={reason} />
                <Docking @sizing=ItemSize::Percent(100.0) layout>
                    <DockSplit id="workspace" fraction=FILES_SHARE>
                        <DockPane id="files" locked=true>
                            <DockTab id=FILES title="Files" icon=ICON_FOLDER>
                                <FilesPanel workspace />
                            </DockTab>
                            <DockTab id=SETTINGS title="Settings" icon=ICON_SETTINGS>
                                <SettingsPanel workspace />
                            </DockTab>
                            <DockTab id=REVISIONS title="History" icon=ICON_ROTATE_CCW_CLOCK>
                                <RevisionHistoryPanel workspace />
                            </DockTab>
                        </DockPane>
                        <DockPane
                            id="editors"
                            locked=true
                            empty={move || view! {
                                <EmptyPanel />
                            }}
                        >
                            <ForEach keys={block_tabs}>|tab: TabId|
                                <BlockTab workspace tab />
                            </ForEach>
                            <ForEach keys={tabbed}>|window: HostWindowId|
                                <WindowTab workspace key={window} />
                            </ForEach>
                        </DockPane>
                    </DockSplit>
                    <ForEach keys={panels}>|panel: HostPanel|
                        <PanelWindow workspace panel />
                    </ForEach>
                    <ForEach keys={floating}>|key: HostWindowId|
                        <FloatingWindow workspace key placeholder />
                    </ForEach>
                    <ForEach keys={dialog_windows}>|(window, parent): (HostWindowId, HostWindowId)|
                        <AttachedWindow workspace window parent />
                    </ForEach>
                    <ForEach keys={rename_dialogs}>|(block, tab): (Uuid, TabId)|
                        <RenameDialog workspace block tab />
                    </ForEach>
                </Docking>
                <WorkspaceDialogs workspace />
            </List>
        </Frame>
    }
}

#[component]
fn FilesPanel(workspace: Workspace) -> NodeId {
    let files = workspace.files();
    let target = create_memo(move || {
        files.get().map(|id| {
            ChildTarget::new(id, EditorViewContent::CONTENT_TYPE)
                .edited_with(Some(FILES_EDITOR))
                .viewed_by(id)
        })
    });
    view! {
        <SidePanel workspace target loading="Files are loading…" id="workspace.files" />
    }
}

#[component]
fn SettingsPanel(workspace: Workspace) -> NodeId {
    let editor = workspace.editor();
    let target = create_memo(move || {
        let settings = own_settings_blocks(&editor)?.first().copied()?;
        Some(ChildTarget::new(settings, SettingsContent::CONTENT_TYPE))
    });
    view! {
        <SidePanel workspace target loading="Settings are loading…" id="workspace.settings" />
    }
}

#[component]
fn SidePanel(
    workspace: Workspace,
    target: Memo<Option<ChildTarget>>,
    loading: &'static str,
    id: &'static str,
) -> NodeId {
    let phone = workspace.phone();
    let top_bar = create_memo(move || match phone.get() {
        true => TopBar::Phone,
        false => TopBar::Hidden,
    });
    let editor = workspace.editor();
    let tab = use_dock_tab();
    view! {
        <List spacing=Space::None>
            <ChildBlock
                @sizing=ItemSize::Percent(100.0)
                editor={editor}
                block={target}
                mode=ChildMode::Live
                own_frame=true
                top_bar={top_bar}
                on_focus={move |()| tab.iter().for_each(|tab| tab.show())}
                @test_id={id}
            >|handle: ChildBlockHandle|
                <PanelStatus state={handle.state} loading={loading.to_owned()} />
            </ChildBlock>
        </List>
    }
}

#[component]
fn EmptyPanel() -> NodeId {
    view! {
        <Frame padding=PANEL_PADDING>
            <List spacing=PANEL_SPACING align=Align::Center>
                <Text style=TextStyle::HEADING>"No file open"</Text>
                <Text style=TextStyle::CAPTION>
                    "Open or create a file from Files to get started."
                </Text>
            </List>
        </Frame>
    }
}
