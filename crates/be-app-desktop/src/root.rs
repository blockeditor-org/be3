mod bar;
mod calendar;
mod clock;
mod ending;
mod launcher;
mod layouts;
pub(crate) mod media;
mod notifications;
pub(crate) mod osd;
mod popup;
mod power;
mod programs_panel;
mod rotate;
mod sessions;
mod status;
mod tray;
mod volume;
mod wifi;
mod window_keys;
mod windows;

use be_sdk_beui::MediaWatch;
use be_sdk_beui::beui::dock::{DockBar, DockNode, DockPane, DockWindow, Docking, TabId};
use be_sdk_beui::beui::dock_host::styled::{DockHost, DockHostLayout};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{Modifiers, NodeId, Pos2, Rect, Vec2, pos2, vec2};
use be_sdk_beui::{Editor, Loaded};
use be_sdk_beui::{HostPanel, HostWindowId};
use be_sdk_shell::{
    BlockTab, Failure, PanelWindow, ProgramWindow, UnshownWindows, WindowDialog, Workspace,
    WorkspaceDialogs,
};
use uuid::Uuid;

use bar::{
    Bars, PROGRAMS_PLACE, ProgramsBar, STATUS_PLACE, Shell, StatusBar, WINDOWS_PLACE, WindowsBar,
};
use media::MediaKeys;
use notifications::{DesktopNotifications, DesktopToasts};
use programs_panel::{ProgramsPanel, ProgramsPanelKey, ProgramsPanelOverlay};
use window_keys::WindowShortcuts;
use windows::DeskWindows;

const WINDOW_ORIGIN: f32 = 48.0;
const WINDOW_CASCADE: f32 = 32.0;
const WINDOW_CASCADES: u64 = 8;
const WINDOW_SIZE: Vec2 = vec2(960.0, 640.0);

pub struct DesktopRoot;

#[component]
impl be_sdk_beui::BeuiRoot for DesktopRoot {
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
    shell: Shell,
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
    let shell = desktop.shell;
    let editor = shell.workspace.editor();
    let main = shell.workspace.layout();
    let layout = bar::display_layout(editor, main, display);
    let host = DockHostLayout::new();
    cover_under_fullscreen(shell.workspace.dock_host(), host, display);
    let launches = shell.programs_panel.launches();
    let area = windows_area(host, region_area());
    shell.windows.report_area(display, area);
    provide_context(bar::BarArea(area));
    let bars = Bars::of(display);
    view! {
        <Frame>
            <List spacing=Space::None>
                <ListItem grow=1.0>
                    <DockHost
                        @test_id={format!("desktop.dock.{display}")}
                        layout=host
                        drag_modifier={Some(Modifiers::LOGO)}
                    >
                        <Docking layout>
                            <DockPane id={"desktop".to_owned()} bare=true></DockPane>
                            <DockBar id={bars.programs} placement=PROGRAMS_PLACE>
                                <ProgramsBar shell display layout />
                            </DockBar>
                            <DockBar id={bars.windows} placement=WINDOWS_PLACE>
                                <WindowsBar shell display />
                            </DockBar>
                            <DockBar id={bars.status} placement=STATUS_PLACE>
                                <StatusBar shell display />
                            </DockBar>
                        </Docking>
                    </DockHost>
                </ListItem>
                <media::LevelDisplay osd={desktop.osd} area display />
                <Show condition={launches}>
                    <ProgramsPanelKey
                        programs_panel={shell.programs_panel}
                        windows={shell.windows}
                        host={shell.workspace.dock_host()}
                    />
                </Show>
                <ProgramsPanelOverlay programs_panel={shell.programs_panel} display layout area />
            </List>
        </Frame>
    }
}

#[component]
fn DesktopShell(editor: Editor) -> NodeId {
    let workspace = Workspace::new(editor);
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
    let host = workspace.dock_host();
    let failure = workspace.error();
    let failed = create_memo(move || failure.get().is_some());
    let reason = create_memo(move || failure.get().unwrap_or_default());
    let program_windows = workspace.program_tabs();
    let windows = DeskWindows::new(workspace);
    let block_windows = workspace.block_tabs();
    ending::keep_windows_through_ending(workspace);
    let dialog_windows = workspace.dialogs();
    let panels = workspace.panels();
    let editor = workspace.editor();
    let programs_panel = ProgramsPanel::new(workspace);
    let launches = programs_panel.launches();
    let keyed = editor.offered::<MediaWatch>();
    let notifications = DesktopNotifications::new(editor);
    let region = region_area();
    let area = windows_area(host, region);
    provide_context(bar::BarArea(area));
    let bars = Bars::of(0);
    let status = create_memo(move || match layout.is_bar_hidden(&bars.status) {
        true => None,
        false => layout.bar_placement(&bars.status),
    });
    let corner = create_memo(move || bar::toast_corner(status.get()));
    let toasts = create_memo(move || bar::clear_of_status(host, status.get(), area.get()));
    let osd = media::Osd::new(editor);
    let layouts = layouts::Layouts::new(editor);
    let several_layouts = layouts.several;
    let shell = Shell::new(workspace, notifications, programs_panel, windows, layouts);
    publish(editor.block_id(), Desktop { shell, osd });
    view! {
        <Frame @node_ref={&surface}>
            <List spacing=Space::None>
                <Failure failed={failed} reason={reason} />
                <ListItem grow=1.0>
                    <DockHost
                        @test_id={"desktop.dock"}
                        layout=host
                        drag_modifier={Some(Modifiers::LOGO)}
                    >
                        <Docking layout>
                            <DockPane
                                id={"desktop".to_owned()}
                                hide_empty_bar=true
                                empty={move || view! {
                                    <Frame />
                                }}
                            ></DockPane>
                            <ForEach keys={block_windows}>|tab: TabId|
                                <BlockWindow workspace tab />
                            </ForEach>
                            <ForEach keys={program_windows}>|tab: TabId|
                                <ProgramWindow workspace tab />
                            </ForEach>
                            <ForEach
                                keys={dialog_windows}
                            >|(window, parent): (HostWindowId, HostWindowId)|
                                <WindowDialog workspace window parent />
                            </ForEach>
                            <ForEach keys={panels}>|panel: HostPanel|
                                <PanelWindow workspace panel />
                            </ForEach>
                            <DockBar id={bars.programs} placement=PROGRAMS_PLACE>
                                <ProgramsBar shell display=0 layout />
                            </DockBar>
                            <DockBar id={bars.windows} placement=WINDOWS_PLACE>
                                <WindowsBar shell display=0 />
                            </DockBar>
                            <DockBar id={bars.status} placement=STATUS_PLACE>
                                <StatusBar shell display=0 />
                            </DockBar>
                        </Docking>
                    </DockHost>
                </ListItem>
                <WorkspaceDialogs workspace />
                <UnshownWindows workspace />
                <bar::UnreadableBarsDialog editor />
                <Show condition={keyed}>
                    <MediaKeys editor osd />
                </Show>
                <Show condition={several_layouts}>
                    <layouts::LayoutKey layouts />
                </Show>
                <WindowShortcuts editor host />
                <media::LevelDisplay osd area display=0 />
                <Show condition={launches}>
                    <ProgramsPanelKey programs_panel windows host />
                </Show>
                <ProgramsPanelOverlay programs_panel display=0 layout area />
                <DesktopToasts notifications area={toasts} corner />
                <ending::EndingBar workspace area />
            </List>
        </Frame>
    }
}

fn cover_under_fullscreen(main: DockHostLayout, layout: DockHostLayout, display: u32) {
    let covered = create_memo(move || {
        main.fullscreen()
            .and_then(|tab| main.area_of_window(tab))
            .is_some_and(|area| area == display as usize)
    });
    create_effect(move || {
        let covered = covered.get();
        untrack(|| layout.set_covered(covered));
    });
}

fn region_area() -> Memo<Rect> {
    let area = container_size();
    create_memo(move || Rect::from_min_size(Pos2::ZERO, area.map_or(Vec2::ZERO, |area| area.get())))
}

fn windows_area(layout: DockHostLayout, region: Memo<Rect>) -> Memo<Rect> {
    create_memo(move || {
        let inside = layout.area();
        match inside.is_positive() {
            true => inside,
            false => region.get(),
        }
    })
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
