use be_sdk_beui::beui::icons::ICON_BLOCKS;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::native::last_pointer;
use be_sdk_beui::beui::{Image, NodeId, Pos2, ScrollGesture, window_origin};
use be_sdk_beui::desktop;
use beui::{Align, Direction};

use super::bar::bar_bounds;
use be_sdk_beui::{
    Editor, HostImage, HostTrayItem, TrayClick, TrayMenuEntry, TrayRequest, TrayStatus, TrayToggle,
    TrayWatch,
};

const TRAY_SPACING: Space = Space::Xs2;
const WHEEL_STEP: f32 = 3.0;

type Chosen = Callback<i32>;

fn picture(icon: &HostImage) -> Option<Image> {
    let fits = icon.width > 0 && icon.height > 0 && icon.is_whole();
    fits.then(|| Image::from_rgba(icon.width, icon.height, icon.rgba.clone()))
}

fn checked(toggle: TrayToggle) -> Option<bool> {
    match toggle {
        TrayToggle::None => None,
        TrayToggle::Check(on) | TrayToggle::Radio(on) => Some(on),
    }
}

fn pointed(item: &str) -> TrayClick {
    let at = last_pointer()
        .map(|sample| window_origin(sample.window).unwrap_or(Pos2::ZERO) + sample.pos.to_vec2())
        .unwrap_or(Pos2::ZERO);
    TrayClick {
        item: item.to_owned(),
        x: at.x.round() as i32,
        y: at.y.round() as i32,
    }
}

fn scrolled(gesture: ScrollGesture) -> Option<(i32, bool)> {
    let delta = gesture.delta;
    let horizontal = delta.x.abs() > delta.y.abs();
    let along = match horizontal {
        true => delta.x,
        false => delta.y,
    };
    let steps = (along * WHEEL_STEP).round() as i32;
    (steps != 0).then_some((steps, horizontal))
}

#[component]
pub(crate) fn TrayItems(
    editor: Editor,
    #[prop(default = Direction::Horizontal)] direction: Prop<Direction>,
) -> NodeId {
    let listed = editor.watch::<TrayWatch>();
    let keys = create_memo(move || {
        listed.with(|items| {
            items
                .iter()
                .filter(|item| item.status != TrayStatus::Passive)
                .map(|item| item.id.clone())
                .collect::<Vec<_>>()
        })
    });
    view! {
        <List direction align=Align::Center spacing=TRAY_SPACING @test_id={"desktop.tray"}>
            <ForEach keys>|id: String|
                <TrayButton editor={editor} listed={listed} id />
            </ForEach>
        </List>
    }
}

#[component]
fn TrayButton(editor: Editor, listed: Memo<Vec<HostTrayItem>>, id: String) -> NodeId {
    let item = create_memo({
        let id = id.clone();
        move || {
            listed.with(|items| {
                items
                    .iter()
                    .find(|item| item.id == id)
                    .cloned()
                    .unwrap_or_default()
            })
        }
    });
    let label = create_memo(move || {
        item.with(
            |item| match item.tooltip.is_empty() || item.tooltip == item.title {
                true => item.title.clone(),
                false => format!("{}: {}", item.title, item.tooltip),
            },
        )
    });
    let icon = create_memo(move || item.with(|item| item.icon.clone()));
    let image = create_memo(move || icon.with(|icon| icon.as_ref().and_then(picture)));
    let entries = create_memo(move || item.with(|item| item.entries().unwrap_or_default()));
    let has_menu = create_memo(move || entries.with(|entries| !entries.is_empty()));
    let item_is_menu = create_memo(move || item.with(|item| item.item_is_menu));
    let click_opens = create_memo(move || item_is_menu.get() && has_menu.get());
    let request = {
        let (editor, id) = (editor, id.clone());
        move |request: fn(TrayClick) -> TrayRequest| {
            let request = request(pointed(&id));
            editor.act(move || desktop::tray::request(request).detach())
        }
    };
    let (click, secondary, middle, own_menu) =
        (request.clone(), request.clone(), request.clone(), request);
    let (scrolled_id, opened_id, chosen_id) = (id.clone(), id.clone(), id.clone());
    let chosen = Chosen::new(move |entry: i32| {
        let request = TrayRequest::MenuChosen {
            item: chosen_id.clone(),
            entry,
        };
        editor.act(move || desktop::tray::request(request).detach());
    });
    let prefix = format!("desktop.tray.{id}");
    let open = create_signal(false);
    let activating = create_signal(false);
    let mut was_open = false;
    create_effect(move || {
        let now = open.get();
        if now != was_open {
            was_open = now;
            untrack(|| {
                activating.set(false);
                let request = match now {
                    true => TrayRequest::MenuOpened(pointed(&opened_id)),
                    false => TrayRequest::MenuClosed(opened_id.clone()),
                };
                editor.act(move || desktop::tray::request(request).detach())
            });
        }
    });
    let mut was_menu = item_is_menu.get_untracked();
    create_effect(move || {
        let menu = item_is_menu.get();
        let became = menu && !was_menu;
        was_menu = menu;
        if became && activating.get_untracked() {
            activating.set(false);
            untrack(|| match has_menu.get() {
                true => open.set(true),
                false => own_menu(TrayRequest::MenuOpened),
            });
        }
    });
    view! {
        <MenuButton
            icon_only=true
            variant=ButtonVariant::Ghost
            size=ButtonSize::Compact
            label
            image
            glyph=ICON_BLOCKS
            click_opens
            secondary_opens={has_menu}
            bounds={bar_bounds()}
            @test_id={prefix.clone()}
            items={view! {
                <TrayEntries entries chosen prefix={prefix.clone()} />
            }}
            on_click={move || match item_is_menu.get_untracked() {
                true => click(TrayRequest::MenuOpened),
                false => {
                    activating.set(true);
                    click(TrayRequest::Activate)
                }
            }}
            on_secondary_click={move || secondary(TrayRequest::MenuOpened)}
            on_middle_click={move || middle(TrayRequest::SecondaryActivate)}
            on_scroll={move |gesture: ScrollGesture| {
                if let Some((delta, horizontal)) = scrolled(gesture) {
                    let request = TrayRequest::Scroll {
                        item: scrolled_id.clone(),
                        delta,
                        horizontal,
                    };
                    editor.act(move || desktop::tray::request(request).detach());
                }
            }}
            open
        />
    }
}

#[component]
fn TrayEntries(
    entries: Memo<Vec<TrayMenuEntry>>,
    chosen: Chosen,
    prefix: String,
) -> DynamicSegment<MenuItem> {
    let keys = create_memo(move || {
        entries.with(|entries| {
            entries
                .iter()
                .filter(|entry| !entry.item.separator)
                .map(|entry| (entry.item.id, matches!(entry.item.toggle, TrayToggle::Radio(_))))
                .collect::<Vec<(i32, bool)>>()
        })
    });
    view! {
        <ForEach keys>|(id, radio): (i32, bool)|
            <TrayEntry entries={entries} id radio chosen={chosen} prefix={prefix.clone()} />
        </ForEach>
    }
}

#[component]
fn TrayEntry(
    entries: Memo<Vec<TrayMenuEntry>>,
    id: i32,
    radio: bool,
    chosen: Chosen,
    prefix: String,
) -> MenuItem {
    let entry = create_memo(move || {
        entries.with(|entries| {
            entries
                .iter()
                .find(|entry| entry.item.id == id && !entry.item.separator)
                .cloned()
                .unwrap_or_default()
        })
    });
    let separated = create_memo(move || {
        entries.with(|entries| {
            let Some(at) = entries
                .iter()
                .position(|entry| entry.item.id == id && !entry.item.separator)
            else {
                return false;
            };
            let before = &entries[..at];
            before.last().is_some_and(|entry| entry.item.separator)
                && before.iter().any(|entry| !entry.item.separator)
        })
    });
    let label = create_memo(move || entry.with(|entry| entry.item.label.clone()));
    let checked = create_memo(move || entry.with(|entry| checked(entry.item.toggle)));
    let disabled = create_memo(move || entry.with(|entry| !entry.item.enabled));
    let children = create_memo(move || entry.with(|entry| entry.children.clone()));
    let row_test_id = format!("{prefix}.{id}");
    view! {
        <MenuItem label checked radio disabled separated row_test_id on_click={move || chosen.call(id)}>
            <TrayEntries entries={children} chosen prefix />
        </MenuItem>
    }
}
