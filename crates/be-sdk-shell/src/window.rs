use be_sdk_beui::desktop;

use crate::KeptWindow;
use std::cell::Cell;

use be_sdk_beui::beui::dock::styled::WINDOW_CHROME;
use be_sdk_beui::beui::dock::styled::{TabId, use_dock_tab};
use be_sdk_beui::beui::icons::{ICON_EYE_OFF, ICON_FULLSCREEN, ICON_X};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{Image, NodeId, Pos2, Rect, Vec2, pos2, vec2};
use be_sdk_beui::{
    ChildMode, Editor, HostImage, HostWindow, HostWindowId, Subregion, SubregionContent,
    WindowAction, WindowMenu,
};
use beui::{Align, Direction};

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
    tab.value().checked_sub(WINDOW_TABS).map(HostWindowId)
}

pub(crate) fn floating_window(window: &HostWindow, area: Rect) -> Rect {
    let size = within_limits(shown_size(window.size), window.min_size, window.max_size);
    cascaded(window.id, size + WINDOW_CHROME, area)
}

pub(crate) fn within_limits(size: Vec2, min: Vec2, max: Vec2) -> Vec2 {
    let axis = |size: f32, min: f32, max: f32| match max > 0.0 {
        true => size.min(max).max(min),
        false => size.max(min),
    };
    vec2(axis(size.x, min.x, max.x), axis(size.y, min.y, max.y))
}

pub(crate) fn kept_window(key: HostWindowId, area: Rect) -> Rect {
    cascaded(key, PLACEHOLDER_SIZE + WINDOW_CHROME, area)
}

pub(crate) fn cascaded(key: HostWindowId, size: Vec2, area: Rect) -> Rect {
    if !area.is_positive() {
        let step = (key.0 % WINDOW_CASCADES) as f32 * WINDOW_CASCADE;
        return Rect::from_min_size(pos2(WINDOW_ORIGIN + step, WINDOW_ORIGIN + step), size);
    }
    let size = size.min(area.size());
    let free = area.size() - size;
    let room = (free.x.min(free.y) - WINDOW_ORIGIN) / WINDOW_CASCADE;
    let cascades = match room >= 0.0 {
        true => (room as u64 + 1).min(WINDOW_CASCADES),
        false => 1,
    };
    let step = WINDOW_ORIGIN + (key.0 % cascades) as f32 * WINDOW_CASCADE;
    Rect::from_min_size(area.min + vec2(step.min(free.x), step.min(free.y)), size)
}

pub(crate) fn followed_size(size: Vec2, asked: Option<Vec2>, min: Vec2, max: Vec2) -> Vec2 {
    let size = within_limits(size, min, max);
    let Some(asked) = asked else {
        return size;
    };
    let held = within_limits(asked, min, max);
    let axis = |size: f32, asked: f32, held: f32| match (size - held).abs() < 0.5 {
        true => asked,
        false => size,
    };
    vec2(
        axis(size.x, asked.x, held.x),
        axis(size.y, asked.y, held.y),
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
        .map(|window| HostWindow {
            parent: window.parent.filter(|parent| by_id.contains_key(parent)),
            ..window.clone()
        })
        .collect()
}

fn readable_id(program_id: &str) -> String {
    let id = program_id.trim();
    let file = id.rsplit(['/', '\\']).next().unwrap_or(id);
    let file = match file.len().checked_sub(4).and_then(|cut| file.get(cut..)) {
        Some(extension) if extension.eq_ignore_ascii_case(".exe") => &file[..file.len() - 4],
        _ => file,
    };
    let package = match file.split_once('!') {
        Some((family, _)) => family.split('_').next().unwrap_or(family),
        None => file,
    };
    let last = package.rsplit('.').next().unwrap_or(package);
    let words = last.replace(['-', '_'], " ");
    let mut letters = words.trim().chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}

fn without_name<'a>(title: &'a str, name: &str) -> &'a str {
    if name.is_empty() {
        return title;
    }
    for separator in [" - ", " \u{2014} ", " \u{2013} ", " | "] {
        let Some(cut) = title.len().checked_sub(separator.len() + name.len()) else {
            continue;
        };
        let (Some(kept), Some(tail)) = (title.get(..cut), title.get(cut..)) else {
            continue;
        };
        if tail.starts_with(separator) && tail[separator.len()..].eq_ignore_ascii_case(name) {
            return kept.trim();
        }
    }
    title
}

fn label(name: &str, title: &str, program_id: &str) -> String {
    let title = without_name(title.trim(), name);
    let titled =
        !title.is_empty() && !title.eq_ignore_ascii_case(name) && title != program_id.trim();
    match (name.is_empty(), titled) {
        (true, false) => "Window".to_owned(),
        (true, true) => title.to_owned(),
        (false, false) => name.to_owned(),
        (false, true) => format!("{name} \u{2014} {title}"),
    }
}

pub fn program_name(window: &HostWindow, parent: Option<&HostWindow>) -> String {
    let named =
        |window: &HostWindow| Some(window.name.trim().to_owned()).filter(|name| !name.is_empty());
    let read =
        |window: &HostWindow| Some(readable_id(&window.program_id)).filter(|name| !name.is_empty());
    named(window)
        .or_else(|| parent.and_then(named))
        .or_else(|| read(window))
        .or_else(|| parent.and_then(read))
        .unwrap_or_default()
}

pub fn window_name(window: &HostWindow, parent: Option<&HostWindow>) -> String {
    label(
        &program_name(window, parent),
        &window.title,
        &window.program_id,
    )
}

impl KeptWindow {
    pub fn program_name(&self) -> String {
        match self.name.trim() {
            "" => readable_id(&self.program_id),
            name => name.to_owned(),
        }
    }

    pub fn window_name(&self) -> String {
        label(&self.program_name(), &self.title, &self.program_id)
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
    limits: Memo<(Vec2, Vec2)>,
    note: Memo<Option<String>>,
    menu: Memo<Option<WindowMenu>>,
) -> NodeId {
    if let Some(tab) = use_dock_tab() {
        tab.show_notice(note);
        if offers_fullscreen {
            tab.offer_fullscreen();
        }
        create_effect(move || {
            if let Some(size) = size.get().and_then(own_size) {
                let (min, max) = limits.get();
                untrack(|| {
                    let asked = tab.window_size().map(|shown| shown - WINDOW_CHROME);
                    tab.resize_window(followed_size(size, asked, min, max) + WINDOW_CHROME)
                });
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
                host.act(move || desktop::windows::request(WindowAction::Focus(window)).detach());
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
                host.act(move || desktop::windows::request(WindowAction::Focus(window)).detach());
            }
        });
        let called = create_memo(move || tab.attention());
        let host = editor.host();
        create_effect(move || {
            if called.get().is_some() {
                host.act(move || desktop::windows::request(WindowAction::Focus(window)).detach());
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
                    .detach()
                });
            }
        });
    }
    let panel = NodeRef::new();
    let menu_at = create_signal(None::<Pos2>);
    let seen = Cell::new(menu.get_untracked().map(|menu| menu.asked));
    create_effect(move || {
        let Some(asked) = menu.get() else {
            return;
        };
        if seen.replace(Some(asked.asked)) == Some(asked.asked) {
            return;
        }
        let shown = untrack(|| {
            let node = panel.try_get()?;
            measure(node).get_untracked()
        });
        if let Some(shown) = shown {
            menu_at.set(Some(shown.rect.min + asked.at));
        }
    });
    let waited = create_signal(false);
    create_effect(move || {
        if responding.get() {
            waited.set(false);
        }
    });
    let banner = create_memo(move || !responding.get() && !waited.get());
    let host = editor.host();
    let force_close = ClickCallback::new(move || {
        host.act(move || desktop::windows::request(WindowAction::Close(window)).detach())
    });
    let act = move |action: WindowAction| {
        let host = editor.host();
        move || host.act(move || desktop::windows::request(action).detach())
    };
    let fullscreen_label = create_memo(move || match fullscreen.get() {
        Some(_) => "Leave full screen".to_owned(),
        None => "Full screen".to_owned(),
    });
    let host = editor.host();
    let make_fullscreen = ClickCallback::new(move || {
        let action = WindowAction::Fullscreen {
            window,
            fullscreen: fullscreen.get_untracked().is_none(),
        };
        host.act(move || desktop::windows::request(action).detach())
    });
    view! {
        <Layers>
            <ContextMenu
                disabled=true
                open_at={menu_at}
                on_close={move || menu_at.set(None)}
                items={view! {
                    <Show condition={offers_fullscreen}>
                        <MenuItem
                            label="Hide"
                            glyph=ICON_EYE_OFF
                            row_test_id="workspace.window.menu.hide"
                            on_click={act(WindowAction::Hide(window))}
                        />
                        <MenuItem
                            label={fullscreen_label}
                            glyph=ICON_FULLSCREEN
                            row_test_id="workspace.window.menu.fullscreen"
                            on_click={make_fullscreen}
                        />
                    </Show>
                    <MenuItem
                        label="Close"
                        glyph=ICON_X
                        row_test_id="workspace.window.menu.close"
                        on_click={act(WindowAction::Close(window))}
                    />
                }}
            >
                <Frame />
            </ContextMenu>
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
