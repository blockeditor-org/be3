mod bar;
mod calendar;
mod fullscreen;
pub(crate) mod launcher;
pub(crate) mod media;
mod notifications;
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
use block_editor_beui::beui::{Modifiers, NodeId, Pos2, Rect, Vec2, pos2, vec2};
use block_editor_beui::{MediaWatch, PowerWatch};
use block_editor_beui::{Editor, Loaded};
use block_editor_beui::{HostPanel, HostWindowId};
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

    #[component]
    fn Display(editor: Editor, display: u32) -> NodeId {
        let shared = shared(editor.block_id());
        let ready = create_memo(move || shared.with(Option::is_some));
        view! {
            <Layers>
                <Show condition={ready}>|| {
                let Some(desktop) = shared.get_untracked() else {
                    return view! {
                        <Frame />
                    };
                };
                view! {
                    <Container>|_|
                        <DisplayBody desktop display />
                    </Container>
                }
            }</Show>
            </Layers>
        }
    }
}

#[derive(Clone, Copy)]
struct Desktop {
    workspace: Workspace,
    notifications: DesktopNotifications,
    launcher: ProgramLauncher,
    placeholders: Placeholders,
    osd: media::Osd,
}

impl PartialEq for Desktop {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

thread_local! {
    static SHARED: std::cell::RefCell<std::collections::HashMap<Uuid, OwnedSignal<Option<Desktop>>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

fn shared_signal(block: Uuid) -> Signal<Option<Desktop>> {
    SHARED.with(|shared| {
        shared
            .borrow_mut()
            .entry(block)
            .or_insert_with(|| OwnedSignal::new(None))
            .handle()
    })
}

fn shared(block: Uuid) -> ReadSignal<Option<Desktop>> {
    shared_signal(block).read_only()
}

fn publish(block: Uuid, desktop: Desktop) {
    let signal = shared_signal(block);
    signal.set(Some(desktop));
    on_cleanup(move || signal.set(None));
}

#[component]
fn DisplayBody(desktop: Desktop, display: u32) -> NodeId {
    let area = region_area();
    view! {
        <Frame>
            <List spacing=0.0>
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <DesktopBar
                    workspace={desktop.workspace}
                    notifications={desktop.notifications}
                    launcher={desktop.launcher}
                    placeholders={desktop.placeholders}
                    display
                />
                <media::LevelDisplay osd={desktop.osd} area display />
            </List>
        </Frame>
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
    let area = region_area();
    let osd = media::Osd::new(editor);
    publish(
        editor.block_id(),
        Desktop {
            workspace,
            notifications,
            launcher,
            placeholders,
            osd,
        },
    );
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
                    display=0
                />
                <Pickers workspace />
                <WorkspaceDialogs workspace />
                <Show condition={keyed}>
                    <MediaKeys editor osd />
                </Show>
                <media::LevelDisplay osd area display=0 />
                <Show condition={launchable}>
                    <ProgramLauncherOverlay launcher />
                </Show>
                <DesktopToasts notifications bar={bar_node} area />
            </List>
        </Frame>
    }
}

fn region_area() -> Memo<Rect> {
    let area = container_size();
    create_memo(move || Rect::from_min_size(Pos2::ZERO, area.map_or(Vec2::ZERO, |area| area.get())))
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
