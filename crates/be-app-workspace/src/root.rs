use be_sdk_beui::be_block::EditorViewContent;
use std::cell::Cell;

use be_sdk_beui::be_block::{BlockContent, FILES_EDITOR, SettingsContent, ViewState};
use be_sdk_beui::beui::NodeId;
use be_sdk_beui::beui::dock::{
    DockEntry, DockMode, DockPane, DockSplit, DockTab, Docking, SplitSize, TabId,
};
use be_sdk_beui::beui::dock_host::styled::DockHost;
use be_sdk_beui::beui::icons::{ICON_FOLDER, ICON_ROTATE_CCW_CLOCK, ICON_SETTINGS};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::root_settings::own_settings_blocks;
use be_sdk_beui::{
    ChildBlock, ChildBlockHandle, ChildMode, ChildTarget, Editor, HostPanel, HostWindowId, Loaded,
    TopBar,
};
use be_sdk_shell::{
    BlockTab, Failure, PanelStatus, PanelWindow, ProgramTab, ProgramWindow, UnshownWindows,
    WindowDialog, Workspace, WorkspaceDialogs,
};
use beui::Align;
use uuid::Uuid;

use crate::phone::{DetailsSheet, PhoneSheet};
use crate::revisions::{RevisionHistory, RevisionHistoryPanel, RevisionPreviewBar};

pub(crate) const FILES: TabId = TabId::new(1);
pub(crate) const SETTINGS: TabId = TabId::new(1 << 39);
pub(crate) const REVISIONS: TabId = TabId::new((1 << 39) + 1);
const FILES_VIEW: &str = "files";
pub(crate) const FILES_WIDTH: f32 = 260.0;
const PANEL_PADDING: Space = Space::Lg;
const PANEL_SPACING: Space = Space::Sm;

pub struct WorkspaceUiRoot;

#[component]
impl be_sdk_beui::BeuiRoot for WorkspaceUiRoot {
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
    let workspace = Workspace::new(editor);
    let history = RevisionHistory::new(editor);
    let shell = Shell {
        workspace,
        history,
        sheet: create_signal(PhoneSheet::Closed),
        files: create_signal(None),
    };
    create_effect(move || shell.follow_focus());
    create_effect(move || shell.open_files());
    view! {
        <Loaded label="Loading the workspace">|| {
            let _layout = editor.view_content();
            view! {
                <Container>|_|
                    <WorkspaceBody shell />
                </Container>
            }
        }</Loaded>
    }
}

#[derive(Clone, Copy)]
struct Shell {
    workspace: Workspace,
    history: RevisionHistory,
    sheet: Signal<PhoneSheet>,
    files: Signal<Option<Uuid>>,
}

impl Shell {
    fn follow_focus(self) {
        let focus = self.workspace.focus().get();
        let target = match focus.shown {
            Some(tab) if tab == FILES || tab == SETTINGS || tab == REVISIONS => self
                .history
                .target()
                .filter(|item| self.workspace.holds(item.id)),
            Some(tab) => self.workspace.tab(tab),
            None => focus.active,
        };
        untrack(|| self.history.follow(target));
    }

    fn open_files(self) {
        if !self.workspace.ready().get() {
            return;
        }
        untrack(|| {
            if self.files.get().is_some() {
                return;
            }
            let editor = self.workspace.editor();
            let saved = editor
                .view_state(FILES_VIEW)
                .and_then(|state| state.refs.first().copied())
                .filter(|id| !id.is_nil());
            let files = saved.or_else(|| {
                let made = self.workspace.create_view(FILES_EDITOR, None)?;
                editor.set_view_state(FILES_VIEW, Some(&ViewState::new(&(), vec![made])));
                Some(made)
            });
            self.files.set(files);
        });
    }

    fn set_phone(self, phone: bool) {
        self.sheet.set(PhoneSheet::Closed);
        self.workspace.set_phone(phone);
    }
}

#[component]
fn WorkspaceBody(shell: Shell) -> NodeId {
    let workspace = shell.workspace;
    let history = shell.history;
    let layout = workspace.layout();
    let mode = layout.mode();
    let was_stacked = Cell::new(false);
    create_effect(move || {
        let now = mode.get() == DockMode::Stacked;
        if was_stacked.replace(now) != now {
            untrack(|| shell.set_phone(now));
        }
    });
    let surface = NodeRef::new();
    workspace.editor().content(&surface);
    let failure = workspace.error();
    let failed = create_memo(move || failure.get().is_some());
    let reason = create_memo(move || failure.get().unwrap_or_default());
    let block_tabs = workspace.block_tabs();
    let top_windows = workspace.program_tabs();
    let opens = opens_windows();
    let tabbed = create_memo(move || match opens.get() {
        true => Vec::new(),
        false => top_windows.get(),
    });
    let floating = create_memo(move || match opens.get() {
        true => top_windows.get(),
        false => Vec::new(),
    });
    let dialog_windows = workspace.dialogs();
    let panels = workspace.panels();
    view! {
        <Frame @node_ref={&surface}>
            <List spacing=Space::None>
                <Failure failed={failed} reason={reason} />
                <ListItem grow=1.0>
                    <DockHost layout={workspace.dock_host()}>
                        <Docking layout>
                            <DockSplit id="workspace" size={SplitSize::First(FILES_WIDTH)}>
                                <DockPane id="files" locked=true>
                                    <DockTab id=FILES title="Files" icon=ICON_FOLDER>
                                        <FilesPanel workspace files={shell.files} />
                                    </DockTab>
                                    <DockTab id=SETTINGS title="Settings" icon=ICON_SETTINGS>
                                        <SettingsPanel workspace />
                                    </DockTab>
                                    <DockTab
                                        id=REVISIONS
                                        title="History"
                                        icon=ICON_ROTATE_CCW_CLOCK
                                    >
                                        <RevisionHistoryPanel workspace history />
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
                                        <WorkspaceBlockTab shell tab />
                                    </ForEach>
                                    <ForEach keys={tabbed}>|tab: TabId|
                                        <ProgramTab workspace tab />
                                    </ForEach>
                                </DockPane>
                            </DockSplit>
                            <ForEach keys={panels}>|panel: HostPanel|
                                <PanelWindow workspace panel />
                            </ForEach>
                            <ForEach keys={floating}>|tab: TabId|
                                <ProgramWindow workspace tab />
                            </ForEach>
                            <ForEach
                                keys={dialog_windows}
                            >|(window, parent): (HostWindowId, HostWindowId)|
                                <WindowDialog workspace window parent />
                            </ForEach>
                        </Docking>
                    </DockHost>
                </ListItem>
                <WorkspaceDialogs workspace />
                <DetailsSheet workspace sheet={shell.sheet} />
                <UnshownWindows workspace />
            </List>
        </Frame>
    }
}

#[component]
fn WorkspaceBlockTab(shell: Shell, tab: TabId) -> DockEntry<TabId> {
    let Shell {
        workspace,
        history,
        sheet,
        ..
    } = shell;
    let preview = create_memo(move || history.preview_for(workspace.tab(tab)));
    view! {
        <BlockTab
            workspace
            tab
            bar={move || view! {
                <RevisionPreviewBar history tab workspace />
            }}
            preview
            on_details={move |()| sheet.set(PhoneSheet::Details(tab))}
        />
    }
}

#[component]
fn FilesPanel(workspace: Workspace, files: Signal<Option<Uuid>>) -> NodeId {
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
    view! {
        <List spacing=Space::None>
            <ListItem grow=1.0>
                <ChildBlock
                    editor={editor}
                    block={target}
                    mode=ChildMode::Live
                    own_frame=true
                    top_bar={top_bar}
                    @test_id={id}
                >|handle: ChildBlockHandle|
                    <PanelStatus state={handle.state} loading={loading.to_owned()} />
                </ChildBlock>
            </ListItem>
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
