use std::cell::Cell;
use std::rc::Rc;

use block_editor_beui::be_block::FILES_EDITOR;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::icons::ICON_FOLDER;
use block_editor_beui::beui::reactive::{
    Align, ForEach, Frame, ItemSize, List, NodeRef, clone, component, create_effect, create_memo,
    untrack, view,
};
use block_editor_beui::beui::styled::{Caption, Docking, Heading, use_theme};
use block_editor_beui::beui::unstyled::{
    Container, DockMode, DockPane, DockSplit, DockTab, TabId, narrower_than,
};
use block_editor_beui::{
    ChildBlock, ChildBlockHandle, ChildMode, ChildTarget, Editor, HostPanel, HostWindowId,
    NARROW_WIDTH, TopBar,
};
use block_shell::{
    BlockTab, DialogWindow, FILES, Failure, PanelStatus, PanelWindow, PickerDialogs, ProblemToasts,
    WindowTab, Workspace, WorkspaceDialogs,
};

const FILES_SHARE: f32 = 0.22;
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
    let workspace = Workspace::new(editor, true);
    view! {
        <Container>
            {move |_| {
                let workspace = Rc::clone(&workspace);
                view! {
                    <WorkspaceBody workspace={workspace} />
                }
            }}
        </Container>
    }
}

#[component]
fn WorkspaceBody(workspace: Rc<Workspace>) -> NodeId {
    let narrow = narrower_than(NARROW_WIDTH);
    let sizing = Rc::downgrade(&workspace);
    let was_narrow = Cell::new(false);
    create_effect(move || {
        let now = narrow.get();
        let Some(workspace) = sizing.upgrade() else {
            return;
        };
        if was_narrow.replace(now) != now {
            untrack(|| workspace.set_phone(now));
        }
    });
    let phone = workspace.phone();
    let mode = create_memo(clone!(phone -> move || match phone.get() {
        true => DockMode::Stacked,
        false => DockMode::Tiled,
    }));
    let surface = NodeRef::new();
    workspace.editor().content(&surface);
    let layout = workspace.layout();
    let failure = workspace.error();
    let failed = create_memo(clone!(failure -> move || failure.get().is_some()));
    let reason = create_memo(clone!(failure -> move || failure.get().unwrap_or_default()));
    let block_tabs = workspace.block_tabs();
    let top_windows = workspace.windows(false);
    let dialog_windows = workspace.windows(true);
    let panels = workspace.panels();
    let files = Rc::clone(&workspace);
    let blocks = Rc::clone(&workspace);
    let programs = Rc::clone(&workspace);
    let hosted = Rc::clone(&workspace);
    let dialogs = Rc::clone(&workspace);
    let pickers = Rc::clone(&workspace);
    let shell_dialogs = Rc::clone(&workspace);
    let editor = workspace.editor().clone();
    let theme = use_theme();
    view! {
        <Frame @node_ref={&surface} color={theme.background.clone()}>
            <List spacing=0.0>
                <Failure failed={failed} reason={reason} />
                <Docking @sizing=ItemSize::Percent(100.0) layout mode home=FILES>
                    <DockSplit id="workspace" fraction=FILES_SHARE>
                        <DockPane id="files">
                            <DockTab id=FILES title="Files" icon=ICON_FOLDER>
                                <FilesPanel workspace={Rc::clone(&files)} />
                            </DockTab>
                        </DockPane>
                        <DockPane
                            id="editors"
                            empty={move || view! {
                                <EmptyPanel />
                            }}
                        >
                            <ForEach keys={block_tabs}>
                                {move |tab: TabId| view! {
                                    <BlockTab workspace={Rc::clone(&blocks)} tab />
                                }}
                            </ForEach>
                            <ForEach keys={top_windows}>
                                {move |window: HostWindowId| view! {
                                    <WindowTab workspace={Rc::clone(&programs)} window />
                                }}
                            </ForEach>
                        </DockPane>
                    </DockSplit>
                    <ForEach keys={panels}>
                        {move |panel: HostPanel| view! {
                            <PanelWindow workspace={Rc::clone(&hosted)} panel />
                        }}
                    </ForEach>
                    <ForEach keys={dialog_windows}>
                        {move |window: HostWindowId| view! {
                            <DialogWindow workspace={Rc::clone(&dialogs)} window />
                        }}
                    </ForEach>
                </Docking>
                <PickerDialogs workspace={pickers} />
                <WorkspaceDialogs workspace={shell_dialogs} />
                <ProblemToasts editor anchor={surface.clone()} />
            </List>
        </Frame>
    }
}

#[component]
fn FilesPanel(workspace: Rc<Workspace>) -> NodeId {
    let phone = workspace.phone();
    let top_bar = create_memo(move || match phone.get() {
        true => TopBar::Phone,
        false => TopBar::Hidden,
    });
    let files = workspace.files();
    let target = create_memo(move || {
        files
            .get()
            .map(|id| ChildTarget::new(id, FILES_EDITOR).viewed_by(id))
    });
    let editor = workspace.editor().clone();
    view! {
        <List spacing=0.0>
            <ChildBlock
                @sizing=ItemSize::Percent(100.0)
                editor={editor}
                block={target}
                mode=ChildMode::Live
                own_frame=true
                top_bar={top_bar}
                @test_id={"workspace.files"}
            >
                {move |handle: ChildBlockHandle| view! {
                    <PanelStatus state={handle.state} loading="Files are loading…" />
                }}
            </ChildBlock>
        </List>
    }
}

#[component]
fn EmptyPanel() -> NodeId {
    view! {
        <Frame padding_horizontal=PANEL_PADDING padding_vertical=PANEL_PADDING>
            <List spacing=PANEL_SPACING align=Align::Center>
                <Heading content="No file open" />
                <Caption content="Open or create a file from Files to get started." />
            </List>
        </Frame>
    }
}
