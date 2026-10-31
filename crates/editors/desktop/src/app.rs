mod apps;
mod bar;
mod calendar;
mod clock;
mod fullscreen;
mod launcher;
pub(crate) mod media;
mod notifications;
pub(crate) mod osd;
mod placeholders;
mod popup;
mod power;
mod rotate;
mod sessions;
mod status;
mod switcher;
mod tray;
mod volume;
mod wifi;
mod windows;

use block_editor_beui::beui::dock::styled::{
    DockBar, DockNode, DockPane, DockWindow, Docking, DockingLayout, TabId,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Modifiers, NodeId, Pos2, Rect, Vec2, pos2, vec2};
use block_editor_beui::{DisplaysWatch, MediaWatch, PowerWatch, WindowsWatch};
use block_editor_beui::{Editor, Loaded};
use block_editor_beui::{HostPanel, HostWindowId};
use block_shell::{
    BlockTab, Failure, FloatingWindow, PanelWindow, WindowDialog, Workspace, WorkspaceDialogs,
};
use uuid::Uuid;

use apps::{Apps, AppsKey, AppsOverlay};
use bar::{APPS_PLACE, AppsBar, Bars, STATUS_PLACE, Shell, StatusBar, WINDOWS_PLACE, WindowsBar};
use beui::ItemSize;
use media::MediaKeys;
use notifications::{DesktopNotifications, DesktopToasts};
use placeholders::{Placeholder, Placeholders};
use windows::DeskWindows;

const WINDOW_ORIGIN: f32 = 48.0;
const WINDOW_CASCADE: f32 = 32.0;
const WINDOW_CASCADES: u64 = 8;
const WINDOW_SIZE: Vec2 = vec2(960.0, 640.0);

pub struct DesktopApp;

#[component]
impl block_editor_beui::BeuiApp for DesktopApp {
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
    cover_under_fullscreen(editor, layout, display);
    let launches = shell.apps.launches();
    let area = windows_area(layout, region_area());
    let bars = Bars::of(display);
    view! {
        <Frame>
            <List spacing=Space::None>
                <Docking
                    @sizing=ItemSize::Percent(100.0)
                    @test_id={format!("desktop.dock.{display}")}
                    layout
                    drag_modifier={Some(Modifiers::LOGO)}
                >
                    <DockPane id={"desktop".to_owned()} bare=true></DockPane>
                    <DockBar id={bars.apps} placement=APPS_PLACE>
                        <AppsBar shell display layout />
                    </DockBar>
                    <DockBar id={bars.windows} placement=WINDOWS_PLACE>
                        <WindowsBar shell display />
                    </DockBar>
                    <DockBar id={bars.status} placement=STATUS_PLACE>
                        <StatusBar shell display />
                    </DockBar>
                </Docking>
                <media::LevelDisplay osd={desktop.osd} area display />
                <Show condition={launches}>
                    <AppsKey apps={shell.apps} windows={shell.windows} layout={main} />
                </Show>
                <AppsOverlay apps={shell.apps} display layout area />
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
    let desk = workspace.desk();
    let placeholders = Placeholders::new(workspace);
    let windows = DeskWindows::new(workspace, placeholders);
    let block_windows = windows.block_windows();
    hold_saving_while_ending(workspace);
    let placeholder = RenderFn::new(move |key: HostWindowId| {
        view! {
            <Placeholder placeholders key />
        }
    });
    let dialog_windows = workspace.dialogs();
    let panels = workspace.panels();
    let editor = workspace.editor();
    let apps = Apps::new(workspace);
    let launches = apps.launches();
    let keyed = editor.offered::<MediaWatch>();
    let notifications = DesktopNotifications::new(editor);
    let region = region_area();
    let area = windows_area(layout, region);
    let bars = Bars::of(0);
    let status = create_memo(move || match layout.is_bar_hidden(&bars.status) {
        true => None,
        false => layout.bar_placement(&bars.status),
    });
    let corner = create_memo(move || bar::toast_corner(status.get()));
    let toasts = create_memo(move || bar::clear_of_status(layout, status.get(), area.get()));
    let osd = media::Osd::new(editor);
    let shell = Shell::new(workspace, notifications, apps, windows);
    publish(editor.block_id(), Desktop { shell, osd });
    view! {
        <Frame @node_ref={&surface}>
            <List spacing=Space::None>
                <Failure failed={failed} reason={reason} />
                <Docking
                    @sizing=ItemSize::Percent(100.0)
                    @test_id={"desktop.dock"}
                    layout
                    drag_modifier={Some(Modifiers::LOGO)}
                >
                    <DockPane id={"desktop".to_owned()} bare=true></DockPane>
                    <ForEach keys={block_windows}>|tab: TabId|
                        <BlockWindow workspace tab />
                    </ForEach>
                    <ForEach keys={desk}>|key: HostWindowId|
                        <FloatingWindow workspace key placeholder />
                    </ForEach>
                    <ForEach keys={dialog_windows}>|(window, parent): (HostWindowId, HostWindowId)|
                        <WindowDialog workspace window parent />
                    </ForEach>
                    <ForEach keys={panels}>|panel: HostPanel|
                        <PanelWindow workspace panel />
                    </ForEach>
                    <DockBar id={bars.apps} placement=APPS_PLACE>
                        <AppsBar shell display=0 layout />
                    </DockBar>
                    <DockBar id={bars.windows} placement=WINDOWS_PLACE>
                        <WindowsBar shell display=0 />
                    </DockBar>
                    <DockBar id={bars.status} placement=STATUS_PLACE>
                        <StatusBar shell display=0 />
                    </DockBar>
                </Docking>
                <WorkspaceDialogs workspace />
                <Show condition={keyed}>
                    <MediaKeys editor osd />
                </Show>
                <media::LevelDisplay osd area display=0 />
                <Show condition={launches}>
                    <AppsKey apps windows layout />
                </Show>
                <AppsOverlay apps display=0 layout area />
                <DesktopToasts notifications area={toasts} corner />
            </List>
        </Frame>
    }
}

fn cover_under_fullscreen(editor: Editor, layout: DockingLayout<TabId>, display: u32) {
    let displays = editor.watch::<DisplaysWatch>();
    let windows = editor.watch::<WindowsWatch>();
    let screen = create_memo(move || {
        displays.with(|displays| displays.get(display as usize).map(|screen| screen.rect))
    });
    let covered = create_memo(move || {
        let Some(screen) = screen.get() else {
            return false;
        };
        windows.with(|windows| {
            windows.iter().any(|window| {
                window.focused
                    && window.shown
                    && window
                        .fullscreen
                        .is_some_and(|area| screen.contains(area.center()))
            })
        })
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

fn windows_area(layout: DockingLayout<TabId>, region: Memo<Rect>) -> Memo<Rect> {
    create_memo(move || {
        let inside = layout.area();
        match inside.is_positive() {
            true => inside,
            false => region.get(),
        }
    })
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
