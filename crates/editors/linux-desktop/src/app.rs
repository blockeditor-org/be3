mod bar;
mod calendar;
mod fullscreen;
pub(crate) mod launcher;
pub(crate) mod media;
mod notifications;
pub(crate) mod osd;
mod placeholders;
mod popup;
mod power;
mod rotate;
mod sessions;
mod switcher;
mod tray;
mod volume;
mod wifi;

use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{
    Container, DockNode, DockPane, DockWindow, TabId, container_size,
};
use block_editor_beui::beui::{Modifiers, NodeId, Rect, Vec2, pos2, vec2};
use block_editor_beui::{Editor, Loaded};
use block_editor_beui::{HostPanel, HostWindowId};
use block_editor_beui::{MediaWatch, PowerWatch};
use block_shell::{
    AttachedWindow, BlockTab, Failure, FloatingWindow, PanelWindow, PickerDialog, Pickers,
    RenameDialog, Workspace, WorkspaceDialogs,
};
use uuid::Uuid;

use bar::DesktopBar;
use launcher::{ProgramLauncher, ProgramLauncherOverlay};
use media::MediaKeys;
use notifications::{DesktopNotifications, DesktopToasts};
use placeholders::{Placeholder, Placeholders};

const WINDOW_ORIGIN: f32 = 48.0;
const WINDOW_CASCADE: f32 = 32.0;
const WINDOW_CASCADES: u64 = 8;
const WINDOW_SIZE: Vec2 = vec2(960.0, 640.0);

pub struct LinuxDesktopApp;

#[component]
impl block_editor_beui::BeuiApp for LinuxDesktopApp {
    #[component]
    fn View(editor: Editor) -> Root {
        view! {
            <Root>
                <DesktopShell editor={editor} />
            </Root>
        }
    }
}

#[component]
fn DesktopShell(editor: Editor) -> NodeId {
    let workspace = Workspace::new(editor, false, true);
    view! {
        <Loaded label="Loading the desktop">|| {
            let _layout = editor.view_content();
            view! {
                <Container>|_|
                    <DesktopBody workspace />
                </Container>
            }
        }</Loaded>
    }
}

#[component]
fn DesktopBody(workspace: Workspace) -> NodeId {
    let surface = NodeRef::new();
    workspace.editor().content(&surface);
    let layout = workspace.layout();
    switcher::bind_window_switcher(layout);
    fullscreen::bind_fullscreen(workspace.editor());
    let failure = workspace.error();
    let failed = create_memo(move || failure.get().is_some());
    let reason = create_memo(move || failure.get().unwrap_or_default());
    let block_tabs = workspace.block_tabs();
    let desk = workspace.desk();
    let placeholders = Placeholders::new(workspace);
    hold_saving_while_ending(workspace);
    let placeholder = RenderFn::new(move |key: HostWindowId| {
        view! {
            <Placeholder placeholders key />
        }
    });
    let dialog_windows = workspace.dialogs();
    let picker_dialogs = workspace.picker_dialogs();
    let rename_dialogs = workspace.rename_dialogs();
    let panels = workspace.panels();
    let editor = workspace.editor();
    let launcher = ProgramLauncher::new(editor);
    let launchable = launcher.offered;
    let keyed = editor.offered::<MediaWatch>();
    let notifications = DesktopNotifications::new(editor);
    let bar_node = NodeRef::new();
    view! {
        <Frame @node_ref={&surface}>
            <List spacing=0.0>
                <Failure failed={failed} reason={reason} />
                <Docking
                    @sizing=ItemSize::Percent(100.0)
                    @test_id={"desktop.dock"}
                    layout
                    drag_modifier={Some(Modifiers::LOGO)}
                >
                    <ForEach keys={block_tabs}>|tab: TabId|
                        <BlockWindow workspace tab />
                    </ForEach>
                    <ForEach keys={desk}>|key: HostWindowId|
                        <FloatingWindow workspace key placeholder />
                    </ForEach>
                    <ForEach keys={dialog_windows}>|(window, parent): (HostWindowId, HostWindowId)|
                        <AttachedWindow workspace window parent />
                    </ForEach>
                    <ForEach keys={picker_dialogs}>|(pick, tab): (u64, TabId)|
                        <PickerDialog workspace pick tab />
                    </ForEach>
                    <ForEach keys={rename_dialogs}>|(block, tab): (Uuid, TabId)|
                        <RenameDialog workspace block tab />
                    </ForEach>
                    <ForEach keys={panels}>|panel: HostPanel|
                        <PanelWindow workspace panel />
                    </ForEach>
                </Docking>
                <DesktopBar
                    @node_ref=&bar_node
                    workspace
                    notifications={notifications}
                    launcher={launcher}
                    placeholders
                />
                <Pickers workspace />
                <WorkspaceDialogs workspace />
                <Show condition={keyed}>
                    <MediaKeys editor />
                </Show>
                <Show condition={launchable}>
                    <ProgramLauncherOverlay launcher />
                </Show>
                <DesktopToasts notifications bar={bar_node} />
            </List>
        </Frame>
    }
}

fn hold_saving_while_ending(workspace: Workspace) {
    let power = workspace.editor().watch::<PowerWatch>();
    let ending = create_memo(move || power.with(|power| power.ending));
    create_effect(move || {
        let ending = ending.get();
        untrack(|| workspace.hold_saving(ending));
    });
}

fn block_window(tab: TabId, area: Vec2) -> Rect {
    let step = (tab.value() % WINDOW_CASCADES) as f32 * WINDOW_CASCADE;
    let origin = pos2(WINDOW_ORIGIN + step, WINDOW_ORIGIN + step);
    let room = area - origin.to_vec2() - vec2(WINDOW_ORIGIN, WINDOW_ORIGIN * 2.0);
    let size = match room.x > 0.0 && room.y > 0.0 {
        true => room.min(WINDOW_SIZE),
        false => WINDOW_SIZE,
    };
    Rect::from_min_size(origin, size)
}

#[component]
fn BlockWindow(workspace: Workspace, tab: TabId) -> DockNode<TabId> {
    let area = container_size().map_or(Vec2::ZERO, |size| size.get_untracked());
    let key = format!("block.{}", tab.value());
    view! {
        <DockWindow id={key.clone()} rect={block_window(tab, area)}>
            <DockPane id={key}>
                <BlockTab workspace tab />
            </DockPane>
        </DockWindow>
    }
}
