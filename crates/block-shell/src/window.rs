use block_editor_beui::desktop;
use std::cell::Cell;

use block_editor_beui::beui::dock::styled::WINDOW_CHROME;
use block_editor_beui::beui::dock::styled::{TabId, use_dock_tab};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Image, NodeId, Rect, Vec2, pos2};
use block_editor_beui::{
    ChildMode, Editor, HostImage, HostWindow, HostWindowId, Subregion, SubregionContent,
    WindowAction,
};

const WINDOW_TABS: u64 = 1 << 41;
const WINDOW_ORIGIN: f32 = 80.0;
const WINDOW_CASCADE: f32 = 32.0;
const WINDOW_CASCADES: u64 = 8;
const BANNER_PADDING: Space = Space::Lg;
const BANNER_SPACING: Space = Space::Lg;
const PLACEHOLDER_EDGE: f32 = 1.0;
const PLACEHOLDER_SIZE: Vec2 = Vec2::new(800.0, 500.0);

pub fn window_tab(window: HostWindowId) -> TabId {
    TabId::new(WINDOW_TABS + window.0)
}

pub(crate) fn tab_window(tab: TabId) -> Option<HostWindowId> {
    (!crate::picker_view::is_picker_tab(tab))
        .then(|| tab.value().checked_sub(WINDOW_TABS).map(HostWindowId))
        .flatten()
}

pub(crate) fn floating_window(window: &HostWindow) -> Rect {
    let step = (window.id.0 % WINDOW_CASCADES) as f32 * WINDOW_CASCADE;
    let size = window.size;
    Rect::from_min_size(
        pos2(WINDOW_ORIGIN + step, WINDOW_ORIGIN + step),
        shown_size(size) + WINDOW_CHROME,
    )
}

pub(crate) fn kept_window(key: HostWindowId) -> Rect {
    let step = (key.0 % WINDOW_CASCADES) as f32 * WINDOW_CASCADE;
    Rect::from_min_size(
        pos2(WINDOW_ORIGIN + step, WINDOW_ORIGIN + step),
        PLACEHOLDER_SIZE + WINDOW_CHROME,
    )
}

pub(crate) fn shown_size(size: Vec2) -> Vec2 {
    own_size(size).unwrap_or(PLACEHOLDER_SIZE)
}

pub(crate) fn own_size(size: Vec2) -> Option<Vec2> {
    (size.x > PLACEHOLDER_EDGE && size.y > PLACEHOLDER_EDGE).then_some(size)
}

pub fn window_picture(icon: &HostImage) -> Option<Image> {
    (icon.width > 0 && icon.height > 0 && icon.is_whole())
        .then(|| Image::from_rgba(icon.width, icon.height, icon.rgba.clone()))
}

pub fn shown_windows(entries: &[HostWindow]) -> Vec<HostWindow> {
    let by_id: std::collections::HashMap<HostWindowId, &HostWindow> =
        entries.iter().map(|entry| (entry.id, entry)).collect();
    let top_shown = |window: &HostWindow| {
        let mut at = window;
        for _ in 0..entries.len() {
            match at.parent.and_then(|parent| by_id.get(&parent)) {
                Some(parent) => at = parent,
                None => break,
            }
        }
        at.shown && window.shown
    };
    entries
        .iter()
        .filter(|window| top_shown(window))
        .cloned()
        .collect()
}

pub fn window_title(window: &HostWindow) -> String {
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
    offers_fullscreen: bool,
    fullscreen: Memo<Option<Rect>>,
    name: Memo<String>,
    responding: Memo<bool>,
    focused: Memo<bool>,
    size: Memo<Option<Vec2>>,
    note: Memo<Option<String>>,
) -> NodeId {
    if let Some(tab) = use_dock_tab() {
        tab.show_notice(note);
        if offers_fullscreen {
            tab.offer_fullscreen();
        }
        create_effect(move || {
            if let Some(size) = size.get().and_then(own_size) {
                untrack(|| tab.resize_window(size + WINDOW_CHROME));
            }
        });
        create_effect(move || {
            if focused.get() {
                untrack(|| tab.show());
            }
        });
        let shown = create_memo(move || tab.focused());
        let host = editor.host();
        let settled = Cell::new(false);
        create_effect(move || {
            let shown = shown.get();
            if settled.replace(true) && shown && !focused.get_untracked() {
                host.act(move || desktop::windows::request(WindowAction::Focus(window)));
            }
        });
        create_effect(move || match fullscreen.get() {
            Some(area) => tab.enter_fullscreen(Some(area)),
            None => untrack(|| tab.leave_fullscreen()),
        });
        let given = create_memo(move || tab.keyboard());
        let host = editor.host();
        create_effect(move || {
            if given.get().is_some() {
                host.act(move || desktop::windows::request(WindowAction::Focus(window)));
            }
        });
        let called = create_memo(move || tab.attention());
        let host = editor.host();
        create_effect(move || {
            if called.get().is_some() {
                host.act(move || desktop::windows::request(WindowAction::Focus(window)));
            }
        });
        let shown = create_memo(move || tab.fullscreen());
        let host = editor.host();
        create_effect(move || {
            let shown = shown.get();
            if shown != fullscreen.get_untracked().is_some() {
                host.act(move || {
                    desktop::windows::request(WindowAction::Fullscreen {
                        window,
                        fullscreen: shown,
                    })
                });
            }
        });
    }
    let panel = NodeRef::new();
    let waited = create_signal(false);
    create_effect(move || {
        if responding.get() {
            waited.set(false);
        }
    });
    let banner = create_memo(move || !responding.get() && !waited.get());
    let host = editor.host();
    let force_close = ClickCallback::new(move || {
        host.act(move || desktop::windows::request(WindowAction::Close(window)))
    });
    view! {
        <Layers>
            <Subregion
                editor={editor}
                placed={Some(SubregionContent::Window(window))}
                mode=ChildMode::Live
                @test_id={"workspace.window"}
                @node_ref=&panel
            />
            <Floating anchor={panel} edge=Edge::Top open={banner}>
                <NotResponding
                    name
                    on_wait={move || waited.set(true)}
                    on_force_close={force_close}
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
    let message = create_memo(move || format!("{} is not responding", name.get()));
    view! {
        <Frame
            fill=ColorRole::Surface
            border={Border::DEFAULT}
            radius=Radius::Lg
            corners=CornerSet::BOTTOM
            padding=BANNER_PADDING
            @test_id={"workspace.window.not_responding"}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=BANNER_SPACING>
                <Text>{message}</Text>
                <Button
                    label="Wait"
                    variant=ButtonVariant::Secondary
                    @test_id={"workspace.window.wait"}
                    on_click={on_wait}
                />
                <Button
                    label="Force close"
                    variant=ButtonVariant::Primary
                    @test_id={"workspace.window.force_close"}
                    on_click={on_force_close}
                />
            </List>
        </Frame>
    }
}
