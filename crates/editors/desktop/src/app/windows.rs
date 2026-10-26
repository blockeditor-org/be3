use block_editor_beui::beui::icons::{ICON_APP_WINDOW, ICON_EYE_OFF, ICON_LAYERS, ICON_MONITOR};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::NodeId;
use block_editor_beui::desktop;
use block_editor_beui::{HostWindow, HostWindowId, WindowAction, WindowKind};
use block_shell::{Workspace, window_picture, window_title};

use super::popup::BarPopup;

const LIST_HEIGHT: Size = Size::Custom(420.0);
const SPACING: Space = Space::Md;
const ROW_SPACING: Space = Space::Xs2;

pub(crate) fn picked(entries: &[HostWindow]) -> Vec<HostWindowId> {
    let top = entries.iter().filter(|entry| entry.parent.is_none());
    let displays = top
        .clone()
        .filter(|entry| entry.kind == WindowKind::Display);
    let windows = top.filter(|entry| entry.kind == WindowKind::Window);
    displays.chain(windows).map(|entry| entry.id).collect()
}

#[component]
pub(crate) fn WindowPicker(workspace: Workspace) -> NodeId {
    view! {
        <BarPopup
                label="Windows"
                glyph={Some(ICON_LAYERS)}
                icon_only=true
                @test_id={"desktop.windows"}
            >|handle: PopoverHandle|
                <WindowList workspace handle />
            </BarPopup>
    }
}

#[component]
fn WindowList(workspace: Workspace, handle: PopoverHandle) -> NodeId {
    let entries = workspace.entries();
    let ids = create_memo(move || entries.with(|entries| picked(entries)));
    let none = create_memo(move || ids.with(Vec::is_empty));
    let listing = create_memo(move || !none.get());
    view! {
        <List spacing=SPACING @test_id={"desktop.windows.panel"}>
            <Text style=TextStyle::HEADING>"Windows"</Text>
            <Show condition={none}>
                <Text style=TextStyle::CAPTION>"No windows are open."</Text>
            </Show>
            <Show condition={listing}>
                <Scroll max_length=LIST_HEIGHT bleed={Bleed::POPOVER.top(false)}>
                    <List spacing=ROW_SPACING>
                        <ForEach keys={ids}>|id: HostWindowId|
                            <EntryRow workspace id handle />
                        </ForEach>
                    </List>
                </Scroll>
            </Show>
        </List>
    }
}

#[component]
fn EntryRow(workspace: Workspace, id: HostWindowId, handle: PopoverHandle) -> NodeId {
    let editor = workspace.editor();
    let entries = workspace.entries();
    let entry = create_memo(move || {
        entries.with(|entries| entries.iter().find(|entry| entry.id == id).cloned())
    });
    let title = create_memo(move || entry.with(|entry| entry.as_ref().map(window_title)).unwrap_or_default());
    let display = create_memo(move || {
        entry.with(|entry| entry.as_ref().is_some_and(|entry| entry.kind == WindowKind::Display))
    });
    let glyph = create_memo(move || match display.get() {
        true => Some(ICON_MONITOR),
        false => Some(ICON_APP_WINDOW),
    });
    let image = create_memo(move || {
        entry.with(|entry| entry.as_ref()?.icon.as_ref().and_then(window_picture))
    });
    let shown = create_memo(move || entry.with(|entry| entry.as_ref().is_some_and(|entry| entry.shown)));
    let focused = create_memo(move || {
        shown.get() && entry.with(|entry| entry.as_ref().is_some_and(|entry| entry.focused))
    });
    let detail = create_memo(move || match (display.get(), shown.get()) {
        (true, true) => "Display".to_owned(),
        (true, false) => "Display, hidden".to_owned(),
        (false, true) => String::new(),
        (false, false) => "Hidden".to_owned(),
    });
    let choose = move || {
        let hidden = !shown.get_untracked();
        editor.act(move || {
            if hidden {
                desktop::windows::request(WindowAction::Show(id));
            }
            desktop::windows::request(WindowAction::Focus(id));
        });
        handle.close.call(());
    };
    let hide = move || editor.act(move || desktop::windows::request(WindowAction::Hide(id)));
    let row_id = format!("desktop.windows.entry.{}", id.0);
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=ROW_SPACING>
            <ListRow
                @sizing=ItemSize::Percent(100.0)
                @test_id={row_id.clone()}
                size=RowSize::Large
                label={title}
                detail
                glyph
                image
                selected={focused}
                on_click={choose}
            />
            <Show condition={shown}>
                <Button
                    icon_only=true
                    glyph=ICON_EYE_OFF
                    label="Hide"
                    variant=ButtonVariant::Ghost
                    @test_id={format!("{row_id}.hide")}
                    on_click={hide}
                />
            </Show>
        </List>
    }
}
