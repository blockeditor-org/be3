use std::cell::Cell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use block_editor_beui::beui::icons::ICON_WEB_ASSET;
use block_editor_beui::beui::reactive::{
    Align, ClickCallback, Direction, Frame, Layers, List, Memo, NodeRef, clone, component,
    create_effect, create_memo, create_signal, untrack, view,
};
use block_editor_beui::beui::styled::{Body, Button, ButtonVariant, WINDOW_CHROME, use_theme};
use block_editor_beui::beui::unstyled::{
    DockDialog, DockEntry, DockNode, DockPane, DockTab, DockWindow, DockingLayout, Edge, Floating,
    TabId, use_dock_tab,
};
use block_editor_beui::beui::{Image, NodeId, Rect, Vec2, pos2, vec2};
use block_editor_beui::{
    ChildMode, Editor, HostImage, HostPanel, HostWindow, HostWindowId, HostWindows, Subregion,
    SubregionContent, WindowAction,
};

use super::host_panel::panel_window;

const WINDOW_TABS: u64 = 1 << 41;
const WINDOW_ORIGIN: f32 = 80.0;
const WINDOW_CASCADE: f32 = 32.0;
const WINDOW_CASCADES: u64 = 8;
const BANNER_PADDING: f32 = 12.0;
const BANNER_SPACING: f32 = 12.0;
const BANNER_RADIUS: u8 = 8;
const PLACEHOLDER_EDGE: f32 = 1.0;
const PLACEHOLDER_SIZE: Vec2 = Vec2::new(800.0, 500.0);

pub(crate) fn window_tab(window: HostWindowId) -> TabId {
    TabId::new(WINDOW_TABS + window.0)
}

pub struct ProgramWindows {
    editor: Editor,
    windows: Memo<Vec<HostWindow>>,
}

impl ProgramWindows {
    pub fn new(editor: &Editor, layout: DockingLayout<TabId>) -> Rc<Self> {
        let windows = Rc::new(Self {
            editor: editor.clone(),
            windows: editor.host_value::<HostWindows>(),
        });
        let raising = Rc::downgrade(&windows);
        let seen = RefCell::new(HashMap::new());
        create_effect(move || {
            if let Some(windows) = raising.upgrade() {
                windows.show_raised(&layout, &mut seen.borrow_mut());
            }
        });
        windows
    }

    pub fn windows(&self) -> Memo<Vec<HostWindowId>> {
        let windows = self.windows.clone();
        create_memo(move || {
            windows.with(|windows| {
                windows
                    .iter()
                    .filter(|window| window.parent.is_none())
                    .map(|window| window.id)
                    .collect::<Vec<_>>()
            })
        })
    }

    pub fn dialogs(&self) -> Memo<Vec<(HostWindowId, HostWindowId)>> {
        let windows = self.windows.clone();
        create_memo(move || {
            windows.with(|windows| {
                windows
                    .iter()
                    .filter_map(|window| Some((window.id, window.parent?)))
                    .collect::<Vec<_>>()
            })
        })
    }

    fn close(&self, window: HostWindowId) {
        self.editor.host().act(WindowAction::Close(window));
    }

    fn show_raised(&self, layout: &DockingLayout<TabId>, seen: &mut HashMap<HostWindowId, u64>) {
        let (raised, parents): (
            HashMap<HostWindowId, u64>,
            HashMap<HostWindowId, HostWindowId>,
        ) = self.windows.with(|windows| {
            (
                windows
                    .iter()
                    .map(|window| (window.id, window.raised))
                    .collect(),
                windows
                    .iter()
                    .filter_map(|window| Some((window.id, window.parent?)))
                    .collect(),
            )
        });
        let forward: Vec<HostWindowId> = raised
            .iter()
            .filter(|(window, count)| seen.get(window).copied().unwrap_or_default() < **count)
            .map(|(window, _)| *window)
            .collect();
        *seen = raised;
        untrack(|| {
            for window in forward {
                let mut root = window;
                for _ in 0..parents.len() {
                    match parents.get(&root) {
                        Some(parent) => root = *parent,
                        None => break,
                    }
                }
                layout.show(&window_tab(root));
            }
        });
    }
}

pub(crate) fn floating_window(window: &HostWindow) -> Rect {
    let step = (window.id.0 % WINDOW_CASCADES) as f32 * WINDOW_CASCADE;
    let size = vec2(window.size.width, window.size.height);
    Rect::from_min_size(
        pos2(WINDOW_ORIGIN + step, WINDOW_ORIGIN + step),
        shown_size(size) + WINDOW_CHROME,
    )
}

pub(crate) fn shown_size(size: Vec2) -> Vec2 {
    own_size(size).unwrap_or(PLACEHOLDER_SIZE)
}

pub(crate) fn own_size(size: Vec2) -> Option<Vec2> {
    (size.x > PLACEHOLDER_EDGE && size.y > PLACEHOLDER_EDGE).then_some(size)
}

pub(crate) fn window_picture(icon: &HostImage) -> Option<Image> {
    (icon.width > 0 && icon.height > 0 && icon.is_whole())
        .then(|| Image::from_rgba(icon.width, icon.height, icon.rgba.clone()))
}

pub(crate) fn window_title(window: &HostWindow) -> String {
    match (window.title.is_empty(), window.app_id.is_empty()) {
        (false, _) => window.title.clone(),
        (true, false) => window.app_id.clone(),
        (true, true) => "Window".to_owned(),
    }
}

#[component]
pub(crate) fn WindowPanel(
    editor: Editor,
    window: HostWindowId,
    fullscreen: Memo<Option<Rect>>,
    name: Memo<String>,
    responding: Memo<bool>,
    focused: Memo<bool>,
    size: Memo<Option<Vec2>>,
) -> NodeId {
    if let Some(tab) = use_dock_tab() {
        create_effect(clone!(tab -> move || {
            if let Some(size) = size.get().and_then(own_size) {
                untrack(|| tab.resize_window(size + WINDOW_CHROME));
            }
        }));
        create_effect(clone!(tab focused -> move || {
            if focused.get() {
                untrack(|| tab.show());
            }
        }));
        let shown = create_memo(clone!(tab -> move || tab.focused()));
        let host = editor.host().clone();
        let settled = Cell::new(false);
        create_effect(clone!(focused -> move || {
            let shown = shown.get();
            if settled.replace(true) && shown && !focused.get_untracked() {
                host.act(WindowAction::Focus(window));
            }
        }));
        create_effect(clone!(tab fullscreen -> move || match fullscreen.get() {
            Some(area) => tab.enter_fullscreen(Some(area)),
            None => untrack(|| tab.leave_fullscreen()),
        }));
        let called = create_memo(clone!(tab -> move || tab.attention()));
        let host = editor.host().clone();
        create_effect(move || {
            if called.get().is_some() {
                host.act(WindowAction::Focus(window));
            }
        });
        let shown = create_memo(clone!(tab -> move || tab.fullscreen()));
        let host = editor.host().clone();
        create_effect(move || {
            let shown = shown.get();
            if shown != fullscreen.get_untracked().is_some() {
                host.act(WindowAction::Fullscreen {
                    window,
                    fullscreen: shown,
                });
            }
        });
    }
    let panel = NodeRef::new();
    let (waited, set_waited) = create_signal(false);
    create_effect(clone!(responding set_waited -> move || {
        if responding.get() {
            set_waited.set(false);
        }
    }));
    let banner = create_memo(clone!(responding -> move || !responding.get() && !waited.get()));
    let anchor = panel.clone();
    let host = editor.host().clone();
    view! {
        <Layers>
            <Subregion
                editor={editor}
                placed={Some(SubregionContent::Window(window))}
                mode=ChildMode::Live
                @test_id={"workspace.window"}
                @node_ref=&panel
            />
            <Floating anchor edge=Edge::Top open={banner}>
                <NotResponding
                    name
                    on_wait={move || set_waited.set(true)}
                    on_force_close={move || host.act(WindowAction::Close(window))}
                />
            </Floating>
        </Layers>
    }
}

#[component]
fn NotResponding(
    name: Memo<String>,
    on_wait: ClickCallback,
    on_force_close: ClickCallback,
) -> NodeId {
    let theme = use_theme();
    let message = create_memo(move || format!("{} is not responding", name.get()));
    view! {
        <Frame
            color={theme.surface.clone()}
            outline={theme.border.clone()}
            outline_width=1.0
            outline_visible=true
            radius=BANNER_RADIUS
            radius_top_left={Some(0)}
            radius_top_right={Some(0)}
            padding_horizontal=BANNER_PADDING
            padding_vertical=BANNER_PADDING
            @test_id={"workspace.window.not_responding"}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=BANNER_SPACING>
                <Body content={message} />
                <Button
                    label="Wait"
                    variant=ButtonVariant::Secondary
                    @test_id={"workspace.window.wait"}
                    on_click={move || on_wait.call()}
                />
                <Button
                    label="Force close"
                    variant=ButtonVariant::Primary
                    @test_id={"workspace.window.force_close"}
                    on_click={move || on_force_close.call()}
                />
            </List>
        </Frame>
    }
}

#[component]
fn WindowTab(windows: Rc<ProgramWindows>, window: HostWindowId) -> DockEntry<TabId> {
    let listed = windows.windows.clone();
    let name = create_memo(move || {
        listed.with(|windows| {
            windows
                .iter()
                .find(|listed| listed.id == window)
                .map_or_else(|| "Window".to_owned(), window_title)
        })
    });
    let listed = windows.windows.clone();
    let responding = create_memo(move || {
        listed.with(|windows| {
            windows
                .iter()
                .find(|listed| listed.id == window)
                .is_none_or(|listed| listed.responding)
        })
    });
    let title = create_memo(clone!(name responding -> move || match responding.get() {
        true => name.get(),
        false => format!("{} (not responding)", name.get()),
    }));
    let listed = windows.windows.clone();
    let fullscreen = create_memo(move || {
        listed.with(|windows| {
            let area = windows
                .iter()
                .find(|listed| listed.id == window)?
                .fullscreen?;
            Some(Rect::from_min_size(
                pos2(area.x, area.y),
                vec2(area.width, area.height),
            ))
        })
    });
    let listed = windows.windows.clone();
    let focused = create_memo(move || {
        listed.with(|windows| {
            windows
                .iter()
                .any(|listed| listed.id == window && listed.focused)
        })
    });
    let listed = windows.windows.clone();
    let size = create_memo(move || {
        listed.with(|windows| {
            windows
                .iter()
                .find(|listed| listed.id == window)
                .map(|listed| vec2(listed.size.width, listed.size.height))
        })
    });
    let listed = windows.windows.clone();
    let icon = create_memo(move || {
        listed.with(|windows| {
            windows
                .iter()
                .find(|listed| listed.id == window)?
                .icon
                .clone()
        })
    });
    let picture = create_memo(move || icon.get().as_ref().and_then(window_picture));
    let closing = Rc::clone(&windows);
    let editor = windows.editor.clone();
    view! {
        <DockTab
            id={window_tab(window)}
            title
            icon=ICON_WEB_ASSET
            picture
            on_close={move || closing.close(window)}
        >
            <WindowPanel
                editor={editor.clone()}
                window
                fullscreen={fullscreen.clone()}
                name={name.clone()}
                responding={responding.clone()}
                focused={focused.clone()}
                size={size.clone()}
            />
        </DockTab>
    }
}

#[component]
pub fn FloatingWindow(windows: Rc<ProgramWindows>, window: HostWindowId) -> DockNode<TabId> {
    let rect = windows
        .windows
        .get_untracked()
        .iter()
        .find(|listed| listed.id == window)
        .map(floating_window)
        .unwrap_or_else(|| panel_window(HostPanel::BlockStack));
    let key = format!("window.{}", window.0);
    view! {
        <DockWindow id={key.clone()} rect>
            <DockPane id={key}>
                <WindowTab windows window />
            </DockPane>
        </DockWindow>
    }
}

#[component]
pub fn AttachedWindow(
    windows: Rc<ProgramWindows>,
    window: HostWindowId,
    parent: HostWindowId,
) -> DockNode<TabId> {
    let listed = windows.windows.clone();
    let info = create_memo(move || {
        listed.with(|windows| windows.iter().find(|listed| listed.id == window).cloned())
    });
    let title = create_memo(clone!(info -> move || {
        info.get().map_or_else(|| "Window".to_owned(), |info| window_title(&info))
    }));
    let size = create_memo(clone!(info -> move || {
        info.get().map_or(Vec2::ZERO, |info| {
            shown_size(vec2(info.size.width, info.size.height))
        })
    }));
    let followed = create_memo(|| None);
    let modal = create_memo(clone!(info -> move || info.get().is_some_and(|info| info.modal)));
    let responding = create_memo(clone!(info -> move || {
        info.get().is_none_or(|info| info.responding)
    }));
    let focused = create_memo(clone!(info -> move || info.get().is_some_and(|info| info.focused)));
    let fullscreen = create_memo(|| None);
    let closing = Rc::clone(&windows);
    let editor = windows.editor.clone();
    view! {
        <DockDialog
            id={window_tab(window)}
            parent={window_tab(parent)}
            title={title.clone()}
            icon=ICON_WEB_ASSET
            size
            modal
            on_close={move || closing.close(window)}
        >
            <WindowPanel
                editor={editor.clone()}
                window
                fullscreen={fullscreen.clone()}
                name={title.clone()}
                responding={responding.clone()}
                focused={focused.clone()}
                size={followed.clone()}
            />
        </DockDialog>
    }
}
