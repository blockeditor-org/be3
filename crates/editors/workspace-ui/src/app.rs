use std::cell::Cell;

use block_editor_beui::be_block::{BlockContent, FILES_EDITOR, SettingsContent};
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::{ICON_FOLDER, ICON_SETTINGS};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{
    Container, DockMode, DockPane, DockSplit, DockTab, TabId, container_size, narrower_than,
    use_dock_tab,
};
use block_editor_beui::root_settings::own_settings_blocks;
use block_editor_beui::{
    ChildBlock, ChildBlockHandle, ChildMode, ChildTarget, Editor, HostPanel, HostWindowId, Loaded,
    NARROW_WIDTH, TopBar,
};
use block_shell::{
    AttachedWindow, BlockTab, FILES, Failure, PanelStatus, PanelWindow, PickerDialogs, REVISIONS,
    RevisionHistoryPanel, SETTINGS, WindowTab, Workspace, WorkspaceDialogs,
};

const FILES_SHARE: f32 = 0.22;
const FILES_MIN_WIDTH: f32 = 300.0;
const PANEL_PADDING: f32 = 14.0;
const PANEL_SPACING: f32 = 6.0;

pub struct WorkspaceUiApp;

impl block_editor_beui::BeuiApp for WorkspaceUiApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <WorkspaceShell editor={editor} />
        }
    }
}

#[component]
fn WorkspaceShell(editor: Editor) -> NodeId {
    let workspace = Workspace::new(editor, true, false);
    let layout = editor.view_content().map(|view| view.loaded());
    let loaded = create_memo(move || layout.is_none_or(|loaded| loaded.get()));
    view! {
        <Loaded loaded={loaded} label="Loading the workspace">
            <Container>|_|
                <WorkspaceBody workspace />
            </Container>
        </Loaded>
    }
}

#[component]
fn WorkspaceBody(workspace: Workspace) -> NodeId {
    let narrow = narrower_than(NARROW_WIDTH);
    let size = container_size().expect("the workspace has a container");
    let files_share = create_memo(move || {
        FILES_SHARE
            .max(FILES_MIN_WIDTH / size.get().x.max(1.0))
            .min(0.5)
    });
    let was_narrow = Cell::new(false);
    create_effect(move || {
        let now = narrow.get();
        if was_narrow.replace(now) != now {
            untrack(|| workspace.set_phone(now));
        }
    });
    let phone = workspace.phone();
    let mode = create_memo(move || match phone.get() {
        true => DockMode::Stacked,
        false => DockMode::Tiled,
    });
    let surface = NodeRef::new();
    workspace.editor().content(&surface);
    let layout = workspace.layout();
    let failure = workspace.error();
    let failed = create_memo(move || failure.get().is_some());
    let reason = create_memo(move || failure.get().unwrap_or_default());
    let block_tabs = workspace.block_tabs();
    let top_windows = workspace.windows();
    let dialog_windows = workspace.dialogs();
    let panels = workspace.panels();
    view! {
        <Frame @node_ref={&surface} fill=ColorRole::Background>
            <List spacing=0.0>
                <Failure failed={failed} reason={reason} />
                <Docking @sizing=ItemSize::Percent(100.0) layout mode>
                    <DockSplit id="workspace" fraction={files_share}>
                        <DockPane id="files">
                            <DockTab id=FILES title="Files" icon=ICON_FOLDER>
                                <FilesPanel workspace />
                            </DockTab>
                            <DockTab id=SETTINGS title="Settings" icon=ICON_SETTINGS>
                                <SettingsPanel workspace />
                            </DockTab>
                            <DockTab id=REVISIONS title="History">
                                <RevisionHistoryPanel workspace />
                            </DockTab>
                        </DockPane>
                        <DockPane
                            id="editors"
                            empty={move || view! {
                                <EmptyPanel />
                            }}
                        >
                            <ForEach keys={block_tabs}>|tab: TabId|
                                <BlockTab workspace tab />
                            </ForEach>
                            <ForEach keys={top_windows}>|window: HostWindowId|
                                <WindowTab workspace key={window} />
                            </ForEach>
                        </DockPane>
                    </DockSplit>
                    <ForEach keys={panels}>|panel: HostPanel|
                        <PanelWindow workspace panel />
                    </ForEach>
                    <ForEach keys={dialog_windows}>|(window, parent): (HostWindowId, HostWindowId)|
                        <AttachedWindow workspace window parent />
                    </ForEach>
                </Docking>
                <PickerDialogs workspace />
                <WorkspaceDialogs workspace />
            </List>
        </Frame>
    }
}

#[component]
fn FilesPanel(workspace: Workspace) -> NodeId {
    let files = workspace.files();
    let target = create_memo(move || {
        files
            .get()
            .map(|id| ChildTarget::new(id, FILES_EDITOR).viewed_by(id))
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
        <List spacing=0.0>
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
                <Text style=TextStyle::HEADING string="No file open" />
                <Text
                    style=TextStyle::CAPTION
                    string="Open or create a file from Files to get started."
                />
            </List>
        </Frame>
    }
}
