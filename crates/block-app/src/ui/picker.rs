use beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, Spacer, Text, clone, component,
    create_memo, view,
};
use beui::styled::{
    Button, ButtonVariant, Caption, Dialog, Icon, Scroll, Separator, Spinner, Tabs, TextInput,
    use_theme,
};
use beui::unstyled::{self, ButtonHandle, ChoiceOption};
use beui::{NodeId, TextAlign};
use uuid::Uuid;

use super::onboarding::ErrorText;
use super::{AppViewStore, UiCommand, send};
use crate::block_picker::{
    ChooseView, CreateView, LinkRow, PickerAction, PickerCommand, PickerTab, PickerView, Tile,
    TileAction, creation_surface,
};
use crate::surfaces::{self, HostSurface, SurfaceId};

const TILE_WIDTH: f32 = 132.0;
const TILE_HEIGHT: f32 = 124.0;
const PICKER_WIDTH: f32 = 640.0;
const PICKER_HEIGHT: f32 = 420.0;

fn act(picker: Uuid, action: PickerAction) {
    send(UiCommand::Picker(PickerCommand { picker, action }));
}

#[component]
pub(super) fn PickerDialogs(view: AppViewStore) -> NodeId {
    let pickers = view.pickers.clone();
    let ids = create_memo(clone!(pickers -> move || {
        pickers.with(|pickers| pickers.iter().map(|picker| picker.id).collect::<Vec<_>>())
    }));
    view! {
        <List spacing=0.0>
            <ForEach keys={ids}>
                {move |id: Uuid| {
                    let picker = create_memo(clone!(pickers -> move || {
                        pickers.with(|pickers| pickers.iter().find(|picker| picker.id == id).cloned())
                    }));
                    view! {
                        <PickerDialog id picker />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn PickerDialog(id: Uuid, picker: Memo<Option<PickerView>>) -> NodeId {
    let surface = creation_surface(
        picker.with_untracked(|picker| picker.as_ref().map_or(0, |picker| picker.depth)),
    );
    let id = create_memo(move || id);
    let choose =
        create_memo(clone!(picker -> move || picker.get().and_then(|picker| picker.choose)));
    let create =
        create_memo(clone!(picker -> move || picker.get().and_then(|picker| picker.create)));
    let error = create_memo(move || picker.get().and_then(|picker| picker.error));
    view! {
        <List spacing=0.0>
            <ChooseDialog id={id.clone()} choose />
            <CreateDialog id={id.clone()} create surface />
            <PickerError id error />
        </List>
    }
}

#[component]
fn ChooseDialog(id: Memo<Uuid>, choose: Memo<Option<ChooseView>>) -> NodeId {
    let open = create_memo(clone!(choose -> move || choose.get().is_some()));
    let tab = create_memo(clone!(choose -> move || {
        choose.get().map_or(PickerTab::Add, |choose| choose.tab)
    }));
    let tab_index = create_memo(clone!(tab -> move || match tab.get() {
        PickerTab::Add => 0,
        PickerTab::Templates => 1,
        PickerTab::LinkExisting => 2,
    }));
    let linking = create_memo(clone!(tab -> move || tab.get() == PickerTab::LinkExisting));
    let tiling = create_memo(clone!(linking -> move || !linking.get()));
    let tiles = create_memo(clone!(choose -> move || {
        choose.get().map(|choose| choose.tiles).unwrap_or_default()
    }));
    let important = create_memo(clone!(tiles -> move || {
        tiles.get().into_iter().filter(|tile| tile.important).collect::<Vec<_>>()
    }));
    let others = create_memo(clone!(tiles -> move || {
        tiles.get().into_iter().filter(|tile| !tile.important).collect::<Vec<_>>()
    }));
    let divided = create_memo(clone!(important others -> move || {
        !important.get().is_empty() && !others.get().is_empty()
    }));
    let links = create_memo(clone!(choose -> move || {
        choose.get().map(|choose| choose.links).unwrap_or_default()
    }));
    let link_keys = create_memo(clone!(links -> move || {
        links.get().into_iter().map(|link| link.id).collect::<Vec<_>>()
    }));
    let no_links = create_memo(clone!(link_keys -> move || link_keys.get().is_empty()));
    let empty = create_memo(clone!(choose -> move || {
        choose.get().map(|choose| choose.empty).unwrap_or_default()
    }));
    let search = create_memo(move || choose.get().map(|choose| choose.search).unwrap_or_default());
    let (tab_id, search_id, close_id) = (id.clone(), id.clone(), id.clone());
    let (tile_id, link_id, done_id) = (id.clone(), id.clone(), id.clone());
    view! {
        <Dialog
            open={open}
            title="Add block"
            width=PICKER_WIDTH
            on_dismiss={move || act(close_id.get_untracked(), PickerAction::Close)}
        >
            <List spacing=10.0>
                <Tabs
                    selected={tab_index}
                    on_change={move |index: usize| {
                        let tab = match index {
                            0 => PickerTab::Add,
                            1 => PickerTab::Templates,
                            _ => PickerTab::LinkExisting,
                        };
                        act(tab_id.get_untracked(), PickerAction::Tab(tab));
                    }}
                    options={view! {
                        <ChoiceOption label="Add" />
                        <ChoiceOption label="Templates" />
                        <ChoiceOption label="Link existing" />
                    }}
                />
                <Frame height=PICKER_HEIGHT>
                    <List spacing=8.0>
                        <Show condition={tiling}>
                            <Scroll @sizing=ItemSize::Percent(100.0)>
                                <List spacing=16.0>
                                    <TileGrid id={tile_id.clone()} tiles={important} />
                                    <Show condition={divided}>
                                        <Separator />
                                    </Show>
                                    <TileGrid id={tile_id} tiles={others} />
                                </List>
                            </Scroll>
                        </Show>
                        <Show condition={linking.clone()}>
                            <TextInput
                                value={search}
                                placeholder="Search by name or UUID"
                                label="Search"
                                on_change={move |value: String| {
                                    act(search_id.get_untracked(), PickerAction::Search(value));
                                }}
                            />
                        </Show>
                        <Show condition={linking}>
                            <Scroll @sizing=ItemSize::Percent(100.0)>
                                <Show condition={no_links}>
                                    <Caption content={empty} />
                                </Show>
                                <ForEach keys={link_keys}>
                                    {move |block: Uuid| {
                                        let links = links.clone();
                                        let link = create_memo(move || {
                                            links.get().into_iter().find(|link| link.id == block)
                                        });
                                        view! {
                                            <LinkButton id={link_id.clone()} link />
                                        }
                                    }}
                                </ForEach>
                            </Scroll>
                        </Show>
                    </List>
                </Frame>
                <Separator />
                <List direction=Direction::Horizontal spacing=8.0>
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Button
                        label="Close"
                        variant=ButtonVariant::Secondary
                        on_click={move || act(done_id.get_untracked(), PickerAction::Close)}
                    />
                </List>
            </List>
        </Dialog>
    }
}

#[component]
fn TileGrid(id: Memo<Uuid>, tiles: Memo<Vec<Tile>>) -> NodeId {
    let keys = create_memo(clone!(tiles -> move || {
        tiles.get().into_iter().map(|tile| tile.key).collect::<Vec<_>>()
    }));
    view! {
        <List direction=Direction::Horizontal spacing=8.0 wrap=true>
            <ForEach keys={keys}>
                {move |key: String| {
                    let tiles = tiles.clone();
                    let tile = create_memo(move || tiles.get().into_iter().find(|tile| tile.key == key));
                    view! {
                        <TileButton id={id.clone()} tile />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn TileButton(id: Memo<Uuid>, tile: Memo<Option<Tile>>) -> NodeId {
    let label =
        create_memo(clone!(tile -> move || tile.get().map(|tile| tile.label).unwrap_or_default()));
    let glyph =
        create_memo(clone!(tile -> move || tile.get().map(|tile| tile.icon).unwrap_or_default()));
    let pick = move || {
        let Some(tile) = tile.get_untracked() else {
            return;
        };
        let action = match tile.action {
            TileAction::Add(block_type) => PickerAction::Add(block_type),
            TileAction::Template(index) => PickerAction::Template(index),
        };
        act(id.get_untracked(), action);
    };
    view! {
        <unstyled::Button
            on_click={pick}
            content={move |handle: ButtonHandle| view! {
                <TileFace handle label glyph />
            }}
        />
    }
}

#[component]
fn TileFace(handle: ButtonHandle, label: Memo<String>, glyph: Memo<String>) -> NodeId {
    let theme = use_theme();
    let ButtonHandle {
        hovered, focused, ..
    } = handle;
    let fill = create_memo(clone!(theme hovered -> move || match hovered.get() {
        true => theme.hover.get(),
        false => theme.surface.get(),
    }));
    view! {
        <Frame
            width=TILE_WIDTH
            height=TILE_HEIGHT
            color={fill}
            outline={theme.border.clone()}
            outline_width=1.0
            outline_visible=true
            radius=5
            padding_horizontal=6.0
            padding_vertical=10.0
        >
            <List spacing=6.0 align=Align::Center>
                <Spacer @sizing=ItemSize::Percent(50.0) />
                <Icon glyph={glyph} text_size=40.0 />
                <Spacer @sizing=ItemSize::Percent(50.0) />
                <Text
                    string={label}
                    font_size=13.0
                    color={theme.text.clone()}
                    align=TextAlign::Center
                    wrap=true
                    underline={focused}
                />
            </List>
        </Frame>
    }
}

#[component]
fn LinkButton(id: Memo<Uuid>, link: Memo<Option<LinkRow>>) -> NodeId {
    let label =
        create_memo(clone!(link -> move || link.get().map(|link| link.name).unwrap_or_default()));
    let glyph =
        create_memo(clone!(link -> move || link.get().map(|link| link.icon).unwrap_or_default()));
    view! {
        <Button
            label={label}
            glyph={glyph}
            variant=ButtonVariant::Ghost
            on_click={move || {
                if let Some(link) = link.get_untracked() {
                    act(id.get_untracked(), PickerAction::Link(link.id));
                }
            }}
        />
    }
}

#[component]
fn CreateDialog(id: Memo<Uuid>, create: Memo<Option<CreateView>>, surface: SurfaceId) -> NodeId {
    let open = create_memo(clone!(create -> move || create.get().is_some()));
    let title = create_memo(clone!(create -> move || {
        format!(
            "New {}",
            create.get().map(|create| create.title).unwrap_or_default()
        )
    }));
    let working =
        create_memo(clone!(create -> move || create.get().is_some_and(|create| create.working)));
    let waiting_label = create_memo(clone!(create -> move || {
        format!(
            "Creating {}...",
            create.get().map(|create| create.title).unwrap_or_default()
        )
    }));
    let options = create_memo(clone!(working -> move || !working.get()));
    let not_ready =
        create_memo(clone!(create -> move || !create.get().is_some_and(|create| create.ready)));
    let dialog = create_memo(move || create.get().is_some_and(|create| create.dialog));
    let height = surfaces::handle(surface).height();
    let (create_id, cancel_id) = (id.clone(), id.clone());
    view! {
        <Dialog
            open={open}
            title={title}
            width=360.0
            on_dismiss={move || act(id.get_untracked(), PickerAction::CancelCreation)}
        >
            <List spacing=10.0>
                <Show condition={dialog}>
                    <Frame height={height}>
                        <HostSurface id=surface />
                    </Frame>
                </Show>
                <Separator />
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Show condition={options}>
                        <Button
                            label="Create"
                            variant=ButtonVariant::Primary
                            disabled={not_ready}
                            on_click={move || act(create_id.get_untracked(), PickerAction::Create)}
                        />
                    </Show>
                    <Show condition={working}>
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Spinner />
                            <Caption content={waiting_label} />
                        </List>
                    </Show>
                    <Button
                        label="Cancel"
                        variant=ButtonVariant::Secondary
                        on_click={move || act(cancel_id.get_untracked(), PickerAction::CancelCreation)}
                    />
                </List>
            </List>
        </Dialog>
    }
}

#[component]
fn PickerError(id: Memo<Uuid>, error: Memo<Option<String>>) -> NodeId {
    let open = create_memo(clone!(error -> move || error.get().is_some()));
    let dismiss = id.clone();
    view! {
        <Dialog
            open={open}
            title="Block picker error"
            width=320.0
            on_dismiss={move || act(dismiss.get_untracked(), PickerAction::DismissError)}
        >
            <List spacing=10.0>
                <ErrorText text={error} />
                <Button
                    label="Dismiss"
                    variant=ButtonVariant::Secondary
                    on_click={move || act(id.get_untracked(), PickerAction::DismissError)}
                />
            </List>
        </Dialog>
    }
}
