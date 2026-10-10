use std::cell::Cell;

use block_editor_beui::beui::reactive::{
    Align, ClickCallback, Direction, Frame, Layers, List, Memo, NodeRef, clone, component,
    create_effect, create_memo, create_signal, untrack, view,
};
use block_editor_beui::beui::styled::{Body, Button, ButtonVariant, WINDOW_CHROME, use_theme};
use block_editor_beui::beui::unstyled::{Edge, Floating, TabId, use_dock_tab};
use block_editor_beui::beui::{NodeId, Rect, pos2, vec2};
use block_editor_beui::{
    ChildMode, Editor, HostWindow, HostWindowId, Subregion, SubregionContent, WindowAction,
};

const WINDOW_TABS: u64 = 1 << 41;
const DIALOG_ORIGIN: f32 = 80.0;
const DIALOG_CASCADE: f32 = 32.0;
const DIALOG_CASCADES: u64 = 8;
const BANNER_PADDING: f32 = 12.0;
const BANNER_SPACING: f32 = 12.0;
const BANNER_RADIUS: u8 = 8;

pub(crate) fn window_tab(window: HostWindowId) -> TabId {
    TabId::new(WINDOW_TABS + window.0)
}

pub(crate) fn tab_window(tab: TabId) -> Option<HostWindowId> {
    tab.value().checked_sub(WINDOW_TABS).map(HostWindowId)
}

pub(crate) fn dialog_window(window: &HostWindow) -> Rect {
    let step = (window.id.0 % DIALOG_CASCADES) as f32 * DIALOG_CASCADE;
    Rect::from_min_size(
        pos2(DIALOG_ORIGIN + step, DIALOG_ORIGIN + step),
        vec2(window.size.width, window.size.height) + WINDOW_CHROME,
    )
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
) -> NodeId {
    if let Some(tab) = use_dock_tab() {
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
