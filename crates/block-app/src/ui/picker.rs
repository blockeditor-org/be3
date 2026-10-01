use be_graph::BlockParent;
use beui::reactive::{
    Align, Child, Direction, ForEach, Frame, ItemSize, Justify, List, Memo, Portal, Show, Spacer,
    Text, clone, component, create_memo, view,
};
use beui::styled::theme::NARROW_WIDTH;
use beui::styled::{
    Button, ButtonVariant, Caption, Dialog, Heading, Icon, ModalSheet, SHEET_STOPS, Scroll,
    Separator, Spinner, Tabs, TextInput, Title, ToggleButton, use_theme,
};
use beui::unstyled::{self, ButtonHandle, ChoiceOption, narrower_than};
use beui::{NodeId, TextAlign};
use uuid::Uuid;

use super::onboarding::ErrorText;
use super::{AppViewStore, UiCommand, send};
use crate::block_picker::{
    ChooseView, CreateView, LinkRow, PickerAction, PickerCommand, PickerTab, PickerView, Placing,
    Tile, TileSection, creation_surface,
};
use crate::surfaces::{self, HostSurface, SurfaceId};

const TILE_WIDTH: f32 = 132.0;
const TILE_HEIGHT: f32 = 124.0;
const PICKER_WIDTH: f32 = 640.0;
const PICKER_HEIGHT: f32 = 420.0;
const SHEET_PADDING: f32 = 16.0;

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
    let phone = narrower_than(NARROW_WIDTH);
    view! {
        <List spacing=0.0>
            <ChooseDialog id={id.clone()} choose phone={phone.clone()} />
            <CreateDialog id={id.clone()} create surface phone />
            <PickerError id error />
        </List>
    }
}

#[component]
fn ChooseDialog(id: Memo<Uuid>, choose: Memo<Option<ChooseView>>, phone: Memo<bool>) -> NodeId {
    let wide = create_memo(clone!(choose phone -> move || choose.get().is_some() && !phone.get()));
    let narrow = create_memo(clone!(choose phone -> move || choose.get().is_some() && phone.get()));
    let placing = create_memo(clone!(choose -> move || {
        choose.get().is_some_and(|choose| choose.placing.is_some())
    }));
    let sheet_title = create_memo(move || match placing.get() {
        true => "New file".to_owned(),
        false => "Add block".to_owned(),
    });
    let (close_id, sheet_close, done_id) = (id.clone(), id.clone(), id.clone());
    let (dialog_id, sheet_id) = (id.clone(), id);
    let dialog_choose = choose.clone();
    view! {
        <List spacing=0.0>
            <Dialog
                open={wide}
                title="Add block"
                width=PICKER_WIDTH
                on_dismiss={move || act(close_id.get_untracked(), PickerAction::Close)}
            >
                <List spacing=10.0>
                    <ChooseBody id={dialog_id} choose={dialog_choose} phone=false />
                    <Separator />
                    <List direction=Direction::Horizontal justify=Justify::End spacing=8.0>
                        <Button
                            label="Close"
                            variant=ButtonVariant::Secondary
                            on_click={move || act(done_id.get_untracked(), PickerAction::Close)}
                        />
                    </List>
                </List>
            </Dialog>
            <ModalSheet
                open={narrow}
                rest={SHEET_STOPS[2]}
                on_close={move || act(sheet_close.get_untracked(), PickerAction::Close)}
            >
                <Frame padding_horizontal=SHEET_PADDING padding_vertical=4.0>
                    <List spacing=10.0>
                        <Title content={sheet_title} />
                        <ChooseBody
                            @sizing=ItemSize::Percent(100.0)
                            id={sheet_id}
                            choose
                            phone=true
                        />
                    </List>
                </Frame>
            </ModalSheet>
        </List>
    }
}

#[component]
fn ChooseBody(id: Memo<Uuid>, choose: Memo<Option<ChooseView>>, phone: bool) -> NodeId {
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
    let sections = create_memo(clone!(choose -> move || {
        choose.get().map(|choose| choose.sections).unwrap_or_default()
    }));
    let section_keys = create_memo(clone!(sections -> move || {
        sections.get().into_iter().map(|section| section.key).collect::<Vec<_>>()
    }));
    let no_sections = create_memo(clone!(section_keys -> move || section_keys.get().is_empty()));
    let tab_empty = create_memo(clone!(tab -> move || match tab.get() {
        PickerTab::Templates => "No templates are available here.".to_owned(),
        _ => "No blocks are available here.".to_owned(),
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
    let placing = create_memo(clone!(choose -> move || {
        choose.get().and_then(|choose| choose.placing)
    }));
    let search = create_memo(move || choose.get().map(|choose| choose.search).unwrap_or_default());
    let (tab_id, search_id, place_id) = (id.clone(), id.clone(), id.clone());
    let (tile_id, link_id) = (id.clone(), id);
    let height = match phone {
        true => None,
        false => Some(PICKER_HEIGHT),
    };
    let area = match phone {
        true => ItemSize::Percent(100.0),
        false => ItemSize::Intrinsic,
    };
    view! {
        <List spacing=10.0>
            <PlacingFields id={place_id} placing />
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
            <Frame @sizing={area} height={height}>
                <List spacing=8.0>
                    <Show condition={tiling}>
                        {move || clone!(no_sections section_keys sections tab_empty tile_id -> view! {
                            <Scroll @sizing=ItemSize::Percent(100.0)>
                                <List spacing=16.0>
                                    <Show condition={no_sections}>
                                        <Caption content={tab_empty.clone()} />
                                    </Show>
                                    <ForEach keys={section_keys}>
                                        {move |key: String| {
                                            let sections = sections.clone();
                                            let section = create_memo(move || {
                                                sections.get().into_iter().find(|section| section.key == key)
                                            });
                                            view! {
                                                <TileSectionView id={tile_id.clone()} section />
                                            }
                                        }}
                                    </ForEach>
                                </List>
                            </Scroll>
                        })}
                    </Show>
                    <Show condition={linking.clone()}>
                        {move || clone!(search_id -> view! {
                            <TextInput
                                value={search.clone()}
                                placeholder="Search by name or UUID"
                                label="Search"
                                on_change={move |value: String| {
                                    act(search_id.get_untracked(), PickerAction::Search(value));
                                }}
                            />
                        })}
                    </Show>
                    <Show condition={linking}>
                        {move || clone!(empty link_id link_keys links no_links -> view! {
                            <Scroll @sizing=ItemSize::Percent(100.0)>
                                <Show condition={no_links}>
                                    <Caption content={empty.clone()} />
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
                        })}
                    </Show>
                </List>
            </Frame>
        </List>
    }
}

#[component]
fn PlacingFields(id: Memo<Uuid>, placing: Memo<Option<Placing>>) -> NodeId {
    let shown = create_memo(clone!(placing -> move || placing.get().is_some()));
    let name = create_memo(clone!(placing -> move || {
        placing.get().map(|placing| placing.name).unwrap_or_default()
    }));
    let places = create_memo(clone!(placing -> move || {
        placing.get().map(|placing| placing.places).unwrap_or_default()
    }));
    let keys = create_memo(clone!(places -> move || {
        places.get().into_iter().map(|place| place.parent).collect::<Vec<BlockParent>>()
    }));
    let chosen = create_memo(clone!(placing -> move || placing.get().map(|placing| placing.place)));
    let naming = id.clone();
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                {move || clone!(id keys name places chosen naming -> view! {
                    <List spacing=8.0>
                        <Caption content="Name" />
                        <TextInput
                            @test_id={"picker.name"}
                            value={name}
                            placeholder="Untitled"
                            label="Name"
                            on_change={move |value: String| {
                                act(naming.get_untracked(), PickerAction::Name(value));
                            }}
                        />
                        <Caption content="Location" />
                        <Scroll direction=Direction::Horizontal>
                            <ForEach keys={keys}>
                                {move |parent: BlockParent| {
                                    let place = create_memo(clone!(places -> move || {
                                        places.get().into_iter().find(|place| place.parent == parent)
                                    }));
                                    let label = create_memo(clone!(place -> move || {
                                        place.get().map(|place| place.name).unwrap_or_default()
                                    }));
                                    let glyph = create_memo(clone!(place -> move || {
                                        place.get().map(|place| place.icon).unwrap_or_default()
                                    }));
                                    let pressed = create_memo(clone!(chosen -> move || {
                                        chosen.get() == Some(parent)
                                    }));
                                    let id = id.clone();
                                    let named = match parent {
                                        BlockParent::Block(block) => format!("picker.place.{block}"),
                                        BlockParent::Root | BlockParent::Detached => {
                                            "picker.place.root".to_owned()
                                        }
                                    };
                                    view! {
                                        <Frame padding_horizontal=3.0>
                                            <ToggleButton
                                                @test_id={named}
                                                label
                                                glyph
                                                pressed
                                                on_change={move |_: bool| {
                                                    act(id.get_untracked(), PickerAction::Place(parent));
                                                }}
                                            />
                                        </Frame>
                                    }
                                }}
                            </ForEach>
                        </Scroll>
                        <Caption content="Choose a type to create it" />
                    </List>
                })}
            </Show>
        </List>
    }
}

#[component]
fn TileSectionView(id: Memo<Uuid>, section: Memo<Option<TileSection>>) -> NodeId {
    let title = create_memo(clone!(section -> move || {
        section.get().map(|section| section.title).unwrap_or_default()
    }));
    let glyph = create_memo(clone!(section -> move || {
        section.get().and_then(|section| section.icon).unwrap_or_default()
    }));
    let has_glyph = create_memo(clone!(glyph -> move || !glyph.get().is_empty()));
    let tiles = create_memo(move || {
        section
            .get()
            .map(|section| section.tiles)
            .unwrap_or_default()
    });
    view! {
        <List spacing=8.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Show condition={has_glyph}>
                    <Icon glyph={glyph.clone()} text_size=18.0 />
                </Show>
                <Heading content={title} />
            </List>
            <TileGrid id tiles />
        </List>
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
        act(id.get_untracked(), PickerAction::Make(tile.action));
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
fn CreateDialog(
    id: Memo<Uuid>,
    create: Memo<Option<CreateView>>,
    surface: SurfaceId,
    phone: Memo<bool>,
) -> NodeId {
    let open = create_memo(clone!(create -> move || create.get().is_some()));
    let title = create_memo(clone!(create -> move || {
        format!(
            "New {}",
            create.get().map(|create| create.title).unwrap_or_default()
        )
    }));
    let cancel = id.clone();
    view! {
        <CreateFrames id={cancel} open phone title>
            <CreateBody id create surface />
        </CreateFrames>
    }
}

#[component]
fn CreateFrames(
    id: Memo<Uuid>,
    open: Memo<bool>,
    phone: Memo<bool>,
    title: Memo<String>,
    children: Child,
) -> NodeId {
    let wide = create_memo(clone!(open phone -> move || open.get() && !phone.get()));
    let narrow = create_memo(clone!(open phone -> move || open.get() && phone.get()));
    let in_dialog = create_memo(clone!(wide -> move || wide.get().then_some(children)));
    let in_sheet = create_memo(clone!(narrow -> move || narrow.get().then_some(children)));
    let (dismiss, closing) = (id.clone(), id);
    let sheet_title = title.clone();
    view! {
        <List spacing=0.0>
            <Dialog
                open={wide}
                title={title}
                width=360.0
                on_dismiss={move || act(dismiss.get_untracked(), PickerAction::CancelCreation)}
            >
                <Portal node={in_dialog} />
            </Dialog>
            <ModalSheet
                open={narrow}
                fit=true
                on_close={move || act(closing.get_untracked(), PickerAction::CancelCreation)}
            >
                <Frame padding_horizontal=SHEET_PADDING padding_vertical=SHEET_PADDING>
                    <List spacing=10.0>
                        <Title content={sheet_title} />
                        <Portal node={in_sheet} />
                    </List>
                </Frame>
            </ModalSheet>
        </List>
    }
}

#[component]
fn CreateBody(id: Memo<Uuid>, create: Memo<Option<CreateView>>, surface: SurfaceId) -> NodeId {
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
    let (create_id, cancel_id) = (id.clone(), id);
    view! {
        <List spacing=10.0>
            <Show condition={dialog}>
                <Frame height={height.clone()}>
                    <HostSurface id=surface />
                </Frame>
            </Show>
            <Separator />
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Show condition={options}>
                    {move || clone!(create_id -> view! {
                        <Button
                            label="Create"
                            variant=ButtonVariant::Primary
                            disabled={not_ready.clone()}
                            on_click={move || act(create_id.get_untracked(), PickerAction::Create)}
                        />
                    })}
                </Show>
                <Show condition={working}>
                    {move || clone!(waiting_label -> view! {
                        <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                            <Spinner />
                            <Caption content={waiting_label} />
                        </List>
                    })}
                </Show>
                <Button
                    label="Cancel"
                    variant=ButtonVariant::Secondary
                    on_click={move || act(cancel_id.get_untracked(), PickerAction::CancelCreation)}
                />
            </List>
        </List>
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
