mod bar;
mod calendar;
mod popup;
mod power;
mod sessions;

use std::rc::Rc;

use block_editor_beui::Editor;
use block_editor_beui::beui::reactive::{
    ForEach, Frame, ItemSize, List, NodeRef, clone, component, create_memo, view,
};
use block_editor_beui::beui::styled::{Docking, use_theme};
use block_editor_beui::beui::unstyled::{
    Container, DockNode, DockPane, DockWindow, TabId, container_size,
};
use block_editor_beui::beui::{Modifiers, NodeId, Rect, Vec2, pos2, vec2};
use block_editor_beui::{HostPanel, HostWindowId};
use block_shell::{
    BlockTab, DialogWindow, Failure, PanelWindow, PickerDialogs, Workspace, WorkspaceDialogs,
};

use bar::DesktopBar;

const WINDOW_ORIGIN: f32 = 48.0;
const WINDOW_CASCADE: f32 = 32.0;
const WINDOW_CASCADES: u64 = 8;
const WINDOW_SIZE: Vec2 = vec2(960.0, 640.0);

pub struct LinuxDesktopApp;

impl block_editor_beui::BeuiApp for LinuxDesktopApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <DesktopShell editor={editor} />
        }
    }
}

#[component]
fn DesktopShell(editor: Editor) -> NodeId {
    let workspace = Workspace::new(editor, false);
    view! {
        <Container>
            {move |_| {
                let workspace = Rc::clone(&workspace);
                view! {
                    <DesktopBody workspace={workspace} />
                }
            }}
        </Container>
    }
}

#[component]
fn DesktopBody(workspace: Rc<Workspace>) -> NodeId {
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
    let blocks = Rc::clone(&workspace);
    let programs = Rc::clone(&workspace);
    let dialogs = Rc::clone(&workspace);
    let hosted = Rc::clone(&workspace);
    let bar = Rc::clone(&workspace);
    let pickers = Rc::clone(&workspace);
    let shell_dialogs = Rc::clone(&workspace);
    let theme = use_theme();
    view! {
        <Frame @node_ref={&surface} color={theme.background.clone()}>
            <List spacing=0.0>
                <Failure failed={failed} reason={reason} />
                <Docking
                    @sizing=ItemSize::Percent(100.0)
                    layout
                    drag_modifier={Some(Modifiers::LOGO)}
                >
                    <ForEach keys={block_tabs}>
                        {move |tab: TabId| view! {
                            <BlockWindow workspace={Rc::clone(&blocks)} tab />
                        }}
                    </ForEach>
                    <ForEach keys={top_windows}>
                        {move |window: HostWindowId| view! {
                            <DialogWindow workspace={Rc::clone(&programs)} window />
                        }}
                    </ForEach>
                    <ForEach keys={dialog_windows}>
                        {move |window: HostWindowId| view! {
                            <DialogWindow workspace={Rc::clone(&dialogs)} window />
                        }}
                    </ForEach>
                    <ForEach keys={panels}>
                        {move |panel: HostPanel| view! {
                            <PanelWindow workspace={Rc::clone(&hosted)} panel />
                        }}
                    </ForEach>
                </Docking>
                <DesktopBar workspace={bar} />
                <PickerDialogs workspace={pickers} />
                <WorkspaceDialogs workspace={shell_dialogs} />
            </List>
        </Frame>
    }
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
fn BlockWindow(workspace: Rc<Workspace>, tab: TabId) -> DockNode<TabId> {
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
