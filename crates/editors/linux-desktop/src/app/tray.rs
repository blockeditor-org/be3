use std::rc::Rc;

use block_editor_beui::beui::icons::{
    ICON_CHECK_BOX, ICON_CHECK_BOX_OUTLINE_BLANK, ICON_RADIO_BUTTON_CHECKED,
    ICON_RADIO_BUTTON_UNCHECKED, ICON_WIDGETS,
};
use block_editor_beui::beui::reactive::{
    Align, Direction, DynamicSegment, ForEach, List, Memo, clone, component, create_memo, view,
};
use block_editor_beui::beui::styled::PictureMenuButton;
use block_editor_beui::beui::unstyled::MenuItem;
use block_editor_beui::beui::{Image, NodeId, ScrollGesture};
use block_editor_beui::{
    Editor, HostImage, HostTrayItem, Tray, TrayMenuEntry, TrayRequest, TrayStatus, TrayToggle,
};

const TRAY_SPACING: f32 = 2.0;
const WHEEL_STEP: f32 = 3.0;

#[derive(Clone)]
struct Chosen(Rc<dyn Fn(i32)>);

fn picture(icon: &HostImage) -> Option<Image> {
    let fits = icon.width > 0 && icon.height > 0 && icon.is_whole();
    fits.then(|| Image::from_rgba(icon.width, icon.height, icon.rgba.clone()))
}

fn toggle_glyph(toggle: TrayToggle) -> &'static str {
    match toggle {
        TrayToggle::None => "",
        TrayToggle::Check(true) => ICON_CHECK_BOX,
        TrayToggle::Check(false) => ICON_CHECK_BOX_OUTLINE_BLANK,
        TrayToggle::Radio(true) => ICON_RADIO_BUTTON_CHECKED,
        TrayToggle::Radio(false) => ICON_RADIO_BUTTON_UNCHECKED,
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
pub(crate) fn TrayItems(editor: Editor) -> NodeId {
    let listed = editor.host_value::<Tray>();
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
            <ForEach keys>
                {move |id: String| view! {
                    <TrayButton editor={editor} listed={listed} id />
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn TrayButton(editor: Editor, listed: Memo<Vec<HostTrayItem>>, id: String) -> NodeId {
    let item = create_memo(clone!(id -> move || {
        listed.with(|items| items.iter().find(|item| item.id == id).cloned().unwrap_or_default())
    }));
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
    let entries = create_memo(move || item.with(|item| item.menu.clone()));
    let has_menu = create_memo(move || entries.with(|entries| !entries.is_empty()));
    let item_is_menu = create_memo(move || item.with(|item| item.item_is_menu));
    let click_opens = create_memo(move || item_is_menu.get() && has_menu.get());
    let request = {
        let (editor, id) = (editor, id.clone());
        move |request: fn(String) -> TrayRequest| editor.act(request(id.clone()))
    };
    let (click, secondary, middle) = (request.clone(), request.clone(), request);
    let (scrolling, opening, choosing) = (editor, editor, editor);
    let (scrolled_id, opened_id, chosen_id) = (id.clone(), id.clone(), id.clone());
    let chosen = Chosen(Rc::new(move |entry: i32| {
        choosing.act(TrayRequest::MenuChosen {
            item: chosen_id.clone(),
            entry,
        });
    }));
    let prefix = format!("desktop.tray.{id}");
    view! {
        <PictureMenuButton
            label
            image
            glyph=ICON_WIDGETS
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
                    scrolling.act(TrayRequest::Scroll {
                        item: scrolled_id.clone(),
                        delta,
                        horizontal,
                    });
                }
            }}
            on_open_change={move |open: bool| {
                opening.act(match open {
                    true => TrayRequest::MenuOpened(opened_id.clone()),
                    false => TrayRequest::MenuClosed(opened_id.clone()),
                });
            }}
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
                .filter(|entry| !entry.separator)
                .map(|entry| entry.id)
                .collect::<Vec<i32>>()
        })
    });
    view! {
        <ForEach keys>
            {move |id: i32| view! {
                <TrayEntry entries={entries} id chosen={chosen.clone()} prefix={prefix.clone()} />
            }}
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
                .find(|entry| entry.id == id && !entry.separator)
                .cloned()
                .unwrap_or_default()
        })
    });
    let separated = create_memo(move || {
        entries.with(|entries| {
            let Some(at) = entries
                .iter()
                .position(|entry| entry.id == id && !entry.separator)
            else {
                return false;
            };
            let before = &entries[..at];
            before.last().is_some_and(|entry| entry.separator)
                && before.iter().any(|entry| !entry.separator)
        })
    });
    let label = create_memo(move || entry.with(|entry| entry.label.clone()));
    let glyph = create_memo(move || entry.with(|entry| toggle_glyph(entry.toggle).to_owned()));
    let disabled = create_memo(move || entry.with(|entry| !entry.enabled));
    let children = create_memo(move || entry.with(|entry| entry.children.clone()));
    let row_test_id = format!("{prefix}.{id}");
    let choose = chosen.clone();
    view! {
        <MenuItem label glyph disabled separated row_test_id on_click={move || (choose.0)(id)}>
            <TrayEntries entries={children} chosen prefix />
        </MenuItem>
    }
}
