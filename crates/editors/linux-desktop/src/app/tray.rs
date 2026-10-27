use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::icons::{
    ICON_BLOCKS, ICON_CIRCLE, ICON_CIRCLE_DOT, ICON_SQUARE, ICON_SQUARE_CHECK,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Image, NodeId, ScrollGesture};
use block_editor_beui::linux_desktop;
use block_editor_beui::{
    Editor, HostImage, HostTrayItem, TrayMenuEntry, TrayRequest, TrayStatus, TrayToggle, TrayWatch,
};

const TRAY_SPACING: Space = Space::Xs2;
const WHEEL_STEP: f32 = 3.0;

type Chosen = Callback<i32>;

fn picture(icon: &HostImage) -> Option<Image> {
    let fits = icon.width > 0 && icon.height > 0 && icon.is_whole();
    fits.then(|| Image::from_rgba(icon.width, icon.height, icon.rgba.clone()))
}

fn toggle_glyph(toggle: TrayToggle) -> Option<Glyph> {
    Some(match toggle {
        TrayToggle::None => return None,
        TrayToggle::Check(true) => ICON_SQUARE_CHECK,
        TrayToggle::Check(false) => ICON_SQUARE,
        TrayToggle::Radio(true) => ICON_CIRCLE_DOT,
        TrayToggle::Radio(false) => ICON_CIRCLE,
    })
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
pub(crate) fn TrayItems(editor: Editor) -> NodeId {
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
        <List
            direction=Direction::Horizontal
            align=Align::Center
            spacing=TRAY_SPACING
            @test_id={"desktop.tray"}
        >
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
        move |request: fn(String) -> TrayRequest| {
            let request = request(id.clone());
            editor.act(move || linux_desktop::tray::request(request))
        }
    };
    let (click, secondary, middle) = (request.clone(), request.clone(), request);
    let (scrolled_id, opened_id, chosen_id) = (id.clone(), id.clone(), id.clone());
    let chosen = Chosen::new(move |entry: i32| {
        let request = TrayRequest::MenuChosen {
            item: chosen_id.clone(),
            entry,
        };
        editor.act(move || linux_desktop::tray::request(request));
    });
    let prefix = format!("desktop.tray.{id}");
    let open = create_signal(false);
    let mut was_open = false;
    create_effect(move || {
        let now = open.get();
        if now != was_open {
            was_open = now;
            untrack(|| {
                let request = match now {
                    true => TrayRequest::MenuOpened(opened_id.clone()),
                    false => TrayRequest::MenuClosed(opened_id.clone()),
                };
                editor.act(move || linux_desktop::tray::request(request))
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
            @test_id={prefix.clone()}
            items={view! {
                <TrayEntries entries chosen prefix={prefix.clone()} />
            }}
            on_click={move || match item_is_menu.get_untracked() {
                true => click(TrayRequest::MenuOpened),
                false => click(TrayRequest::Activate),
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
                    editor.act(move || linux_desktop::tray::request(request));
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
                .map(|entry| entry.item.id)
                .collect::<Vec<i32>>()
        })
    });
    view! {
        <ForEach keys>|id: i32|
            <TrayEntry entries={entries} id chosen={chosen} prefix={prefix.clone()} />
        </ForEach>
    }
}

#[component]
fn TrayEntry(
    entries: Memo<Vec<TrayMenuEntry>>,
    id: i32,
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
    let glyph = create_memo(move || entry.with(|entry| toggle_glyph(entry.item.toggle)));
    let disabled = create_memo(move || entry.with(|entry| !entry.item.enabled));
    let children = create_memo(move || entry.with(|entry| entry.children.clone()));
    let row_test_id = format!("{prefix}.{id}");
    view! {
        <MenuItem
            label
            glyph
            disabled
            separated
            @test_id={row_test_id}
            on_click={move || chosen.call(id)}
        >
            <TrayEntries entries={children} chosen prefix />
        </MenuItem>
    }
}
