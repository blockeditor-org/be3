use std::rc::Rc;

use block_editor_beui::beui::reactive::{
    Align, Child, Direction, Dynamic, ForEach, Frame, ItemSize, Justify, List, Memo, Portal, Show,
    Spacer, Text, clone, component, create_memo, create_signal, view,
};
use block_editor_beui::beui::styled::{
    Button, ButtonVariant, Caption, Dialog, Heading, Icon, ModalSheet, SHEET_STOPS, Scroll,
    Separator, Spinner, Tabs, TextInput, Title, ToggleButton, use_theme,
};
use block_editor_beui::beui::unstyled::{self, ButtonHandle, ChoiceOption};
use block_editor_beui::beui::{NodeId, TextAlign};
use block_editor_beui::block_ui::TemplateEntry;
use block_editor_beui::{
    BlockParent, ChildMode, ChildState, CreationProgress, Subregion, SubregionContent,
};
use uuid::Uuid;

use super::picker::{
    Creating, LinkRow, Pick, PickAction, PickerTab, Place, TileSection, links, places, sections,
    tile_key,
};
use super::workspace::Workspace;

const TILE_WIDTH: f32 = 132.0;
const TILE_HEIGHT: f32 = 124.0;
const PICKER_WIDTH: f32 = 640.0;
const PICKER_HEIGHT: f32 = 420.0;
const SHEET_PADDING: f32 = 16.0;
const CREATION_HEIGHT: f32 = 96.0;

type Act = Rc<dyn Fn(PickAction)>;

#[component]
pub(crate) fn PickerDialogs(workspace: Rc<Workspace>) -> NodeId {
    let picks = workspace.picks.clone();
    let ids = create_memo(move || picks.with(|picks| picks.iter().map(|held| held.pick).collect::<Vec<_>>()));
    view! {
        <List spacing=0.0>
            <ForEach keys={ids}>
                {move |pick: u64| {
                    let workspace = Rc::clone(&workspace);
                    view! {
                        <PickerDialog workspace pick />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn PickerDialog(workspace: Rc<Workspace>, pick: u64) -> NodeId {
    let held = create_memo(clone!(workspace -> move || workspace.pick(pick)));
    let acting = Rc::clone(&workspace);
    let act: Act = Rc::new(move |action: PickAction| acting.pick_action(pick, action));
    let phone = workspace.phone.clone();
    let phone = create_memo(move || phone.get());
    let error = create_memo(clone!(held -> move || held.get().and_then(|held| held.error)));
    view! {
        <List spacing=0.0>
            <ChooseDialog workspace={Rc::clone(&workspace)} held={held.clone()} act={Rc::clone(&act)} phone={phone.clone()} />
            <CreateDialog workspace pick held act={Rc::clone(&act)} phone />
            <PickerError error act />
        </List>
    }
}

#[component]
fn ChooseDialog(
    workspace: Rc<Workspace>,
    held: Memo<Option<Pick>>,
    act: Act,
    phone: Memo<bool>,
) -> NodeId {
    let choosing = create_memo(clone!(held -> move || held.with(|held| held.as_ref().is_some_and(Pick::choosing))));
    let wide = create_memo(clone!(choosing phone -> move || choosing.get() && !phone.get()));
    let narrow = create_memo(clone!(choosing phone -> move || choosing.get() && phone.get()));
    let placing = create_memo(clone!(held -> move || {
        held.with(|held| held.as_ref().is_some_and(|held| held.place.is_some()))
    }));
    let sheet_title = create_memo(move || match placing.get() {
        true => "New file".to_owned(),
        false => "Add block".to_owned(),
    });
    let (dismiss, close, sheet_close) = (Rc::clone(&act), Rc::clone(&act), Rc::clone(&act));
    let sheet_workspace = Rc::clone(&workspace);
    let sheet_held = held.clone();
    let sheet_act = Rc::clone(&act);
    view! {
        <List spacing=0.0>
            <Dialog
                open={wide}
                title="Add block"
                width=PICKER_WIDTH
                on_dismiss={move || dismiss(PickAction::Close)}
            >
                <List spacing=10.0>
                    <ChooseBody workspace held act phone=false />
                    <Separator />
                    <List direction=Direction::Horizontal justify=Justify::End spacing=8.0>
                        <Button
                            label="Close"
                            variant=ButtonVariant::Secondary
                            @test_id={"picker.close"}
                            on_click={move || close(PickAction::Close)}
                        />
                    </List>
                </List>
            </Dialog>
            <ModalSheet
                open={narrow}
                rest={SHEET_STOPS[2]}
                on_close={move || sheet_close(PickAction::Close)}
            >
                <Frame padding_horizontal=SHEET_PADDING padding_vertical=4.0>
                    <List spacing=10.0>
                        <Title content={sheet_title} />
                        <ChooseBody
                            @sizing=ItemSize::Percent(100.0)
                            workspace={sheet_workspace}
                            held={sheet_held}
                            act={sheet_act}
                            phone=true
                        />
                    </List>
                </Frame>
            </ModalSheet>
        </List>
    }
}

#[component]
fn ChooseBody(workspace: Rc<Workspace>, held: Memo<Option<Pick>>, act: Act, phone: bool) -> NodeId {
    let tab = create_memo(clone!(held -> move || {
        held.with(|held| held.as_ref().map_or(PickerTab::Add, |held| held.tab))
    }));
    let tab_index = create_memo(clone!(tab -> move || match tab.get() {
        PickerTab::Add => 0,
        PickerTab::Templates => 1,
        PickerTab::LinkExisting => 2,
    }));
    let linking = create_memo(clone!(tab -> move || tab.get() == PickerTab::LinkExisting));
    let tiling = create_memo(clone!(linking -> move || !linking.get()));
    let sections = create_memo(clone!(workspace held -> move || {
        let types = workspace.types();
        held.with(|held| held.as_ref().map(|held| sections(&types, held)).unwrap_or_default())
    }));
    let section_keys = create_memo(clone!(sections -> move || {
        sections.get().into_iter().map(|section| section.key).collect::<Vec<_>>()
    }));
    let no_sections = create_memo(clone!(section_keys -> move || section_keys.get().is_empty()));
    let tab_empty = create_memo(clone!(tab -> move || match tab.get() {
        PickerTab::Templates => "No templates are available here.".to_owned(),
        _ => "No blocks are available here.".to_owned(),
    }));
    let links = create_memo(clone!(workspace held linking -> move || {
        if !linking.get() {
            return Vec::new();
        }
        let types = workspace.types();
        let blocks = workspace.every_block();
        held.with(|held| held.as_ref().map(|held| links(&types, held, &blocks)).unwrap_or_default())
    }));
    let link_keys = create_memo(clone!(links -> move || {
        links.get().into_iter().map(|link| link.id).collect::<Vec<_>>()
    }));
    let no_links = create_memo(clone!(link_keys -> move || link_keys.get().is_empty()));
    let empty = create_memo(clone!(held -> move || {
        match held.with(|held| held.as_ref().is_none_or(|held| held.search.trim().is_empty())) {
            true => "No blocks are available to link.".to_owned(),
            false => "No matching blocks.".to_owned(),
        }
    }));
    let search = create_memo(clone!(held -> move || {
        held.with(|held| held.as_ref().map(|held| held.search.clone()).unwrap_or_default())
    }));
    let height = match phone {
        true => None,
        false => Some(PICKER_HEIGHT),
    };
    let (tab_act, search_act, tile_act, link_act) =
        (Rc::clone(&act), Rc::clone(&act), Rc::clone(&act), Rc::clone(&act));
    view! {
        <List spacing=10.0>
            <PlacingFields workspace held act />
            <Tabs
                selected={tab_index}
                on_change={move |index: usize| {
                    let tab = match index {
                        0 => PickerTab::Add,
                        1 => PickerTab::Templates,
                        _ => PickerTab::LinkExisting,
                    };
                    tab_act(PickAction::Tab(tab));
                }}
                options={view! {
                    <ChoiceOption label="Add" />
                    <ChoiceOption label="Templates" />
                    <ChoiceOption label="Link existing" />
                }}
            />
            <Frame height={height}>
                <List spacing=8.0>
                    <Show condition={tiling}>
                        {move || clone!(no_sections section_keys sections tab_empty tile_act -> view! {
                            <PickerArea @sizing=ItemSize::Percent(100.0) scrolls={!phone}>
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
                                                <TileSectionView act={Rc::clone(&tile_act)} section />
                                            }
                                        }}
                                    </ForEach>
                                </List>
                            </PickerArea>
                        })}
                    </Show>
                    <Show condition={linking.clone()}>
                        {move || clone!(search_act -> view! {
                            <TextInput
                                value={search.clone()}
                                placeholder="Search by name or UUID"
                                label="Search"
                                @test_id={"picker.search"}
                                on_change={move |value: String| search_act(PickAction::Search(value))}
                            />
                        })}
                    </Show>
                    <Show condition={linking}>
                        {move || clone!(empty link_act link_keys links no_links -> view! {
                            <PickerArea @sizing=ItemSize::Percent(100.0) scrolls={!phone}>
                                <List spacing=0.0>
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
                                                <LinkButton act={Rc::clone(&link_act)} link />
                                            }
                                        }}
                                    </ForEach>
                                </List>
                            </PickerArea>
                        })}
                    </Show>
                </List>
            </Frame>
        </List>
    }
}

#[component]
fn PickerArea(scrolls: bool, children: Child) -> NodeId {
    let scrolls = create_memo(move || scrolls);
    view! {
        <List spacing=0.0>
            <Dynamic value={scrolls}>
                {move |scrolls: bool| match scrolls {
                    true => view! {
                        <Scroll @sizing=ItemSize::Percent(100.0)>{children}</Scroll>
                    },
                    false => view! {
                        <List @sizing=ItemSize::Intrinsic spacing=0.0>{children}</List>
                    },
                }}
            </Dynamic>
        </List>
    }
}

#[component]
fn PlacingFields(workspace: Rc<Workspace>, held: Memo<Option<Pick>>, act: Act) -> NodeId {
    let shown = create_memo(clone!(held -> move || {
        held.with(|held| held.as_ref().is_some_and(|held| held.place.is_some()))
    }));
    let name = create_memo(clone!(held -> move || {
        held.with(|held| held.as_ref().map(|held| held.name.clone()).unwrap_or_default())
    }));
    let places = create_memo(clone!(held -> move || {
        let types = workspace.types();
        let blocks = workspace.every_block();
        held.with(|held| held.as_ref().map(|held| places(&types, held, &blocks)).unwrap_or_default())
    }));
    let keys = create_memo(clone!(places -> move || {
        places.get().into_iter().map(|place| place.parent).collect::<Vec<BlockParent>>()
    }));
    let chosen = create_memo(move || held.with(|held| held.as_ref().and_then(|held| held.place)));
    let naming = Rc::clone(&act);
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                {move || clone!(chosen act keys name naming places -> view! {
                    <List spacing=8.0>
                        <Caption content="Name" />
                        <TextInput
                            @test_id={"picker.name"}
                            value={name}
                            placeholder="Untitled"
                            label="Name"
                            on_change={move |value: String| naming(PickAction::Name(value))}
                        />
                        <Caption content="Location" />
                        <Scroll direction=Direction::Horizontal>
                            <ForEach keys={keys}>
                                {move |parent: BlockParent| {
                                    let place = create_memo(clone!(places -> move || {
                                        places.get().into_iter().find(|place| place.parent == parent)
                                    }));
                                    view! {
                                        <PlaceButton act={Rc::clone(&act)} chosen={chosen.clone()} parent place />
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
fn PlaceButton(
    act: Act,
    chosen: Memo<Option<BlockParent>>,
    parent: BlockParent,
    place: Memo<Option<Place>>,
) -> NodeId {
    let label = create_memo(clone!(place -> move || place.get().map(|place| place.name).unwrap_or_default()));
    let glyph = create_memo(move || place.get().map(|place| place.icon).unwrap_or_default());
    let pressed = create_memo(move || chosen.get() == Some(parent));
    let named = match parent {
        BlockParent::Block(block) => format!("picker.place.{block}"),
        BlockParent::Root | BlockParent::Detached => "picker.place.root".to_owned(),
    };
    view! {
        <Frame padding_horizontal=3.0>
            <ToggleButton
                @test_id={named}
                label
                glyph
                pressed
                on_change={move |_: bool| act(PickAction::Place(parent))}
            />
        </Frame>
    }
}

#[component]
fn TileSectionView(act: Act, section: Memo<Option<TileSection>>) -> NodeId {
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
    let keys = create_memo(clone!(tiles -> move || {
        tiles.get().iter().map(tile_key).collect::<Vec<_>>()
    }));
    view! {
        <List spacing=8.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Show condition={has_glyph}>
                    <Icon glyph={glyph.clone()} text_size=18.0 />
                </Show>
                <Heading content={title} />
            </List>
            <List direction=Direction::Horizontal spacing=8.0 wrap=true>
                <ForEach keys={keys}>
                    {move |key: String| {
                        let tiles = tiles.clone();
                        let tile = create_memo(clone!(key -> move || {
                            tiles.get().into_iter().find(|tile| tile_key(tile) == key)
                        }));
                        view! {
                            <TileButton act={Rc::clone(&act)} key tile />
                        }
                    }}
                </ForEach>
            </List>
        </List>
    }
}

#[component]
fn TileButton(act: Act, key: String, tile: Memo<Option<TemplateEntry>>) -> NodeId {
    let label =
        create_memo(clone!(tile -> move || tile.get().map(|tile| tile.name).unwrap_or_default()));
    let glyph = create_memo(clone!(tile -> move || {
        tile.get().and_then(|tile| tile.icon).unwrap_or_default().to_owned()
    }));
    let pick = move || {
        if let Some(tile) = tile.get_untracked() {
            act(PickAction::Make(tile));
        }
    };
    view! {
        <unstyled::Button
            @test_id={format!("picker.tile.{key}")}
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
fn LinkButton(act: Act, link: Memo<Option<LinkRow>>) -> NodeId {
    let label =
        create_memo(clone!(link -> move || link.get().map(|link| link.name).unwrap_or_default()));
    let glyph =
        create_memo(clone!(link -> move || link.get().map(|link| link.icon).unwrap_or_default()));
    let test_id = link
        .get_untracked()
        .map(|link| format!("picker.link.{}", link.id))
        .unwrap_or_default();
    view! {
        <Button
            label={label}
            glyph={glyph}
            variant=ButtonVariant::Ghost
            @test_id={test_id}
            on_click={move || {
                if let Some(link) = link.get_untracked() {
                    act(PickAction::Link(link.id, link.block_type));
                }
            }}
        />
    }
}

#[component]
fn CreateDialog(
    workspace: Rc<Workspace>,
    pick: u64,
    held: Memo<Option<Pick>>,
    act: Act,
    phone: Memo<bool>,
) -> NodeId {
    let creating = create_memo(clone!(held -> move || {
        held.with(|held| held.as_ref().and_then(|held| held.creating.clone()))
    }));
    let open = create_memo(clone!(creating -> move || {
        creating.with(|creating| creating.as_ref().is_some_and(|creating| creating.template.dialog))
    }));
    let title = create_memo(clone!(creating -> move || {
        format!(
            "New {}",
            creating.get().map(|creating| creating.template.name).unwrap_or_default()
        )
    }));
    let cancel = Rc::clone(&act);
    let body = Rc::clone(&workspace);
    view! {
        <List spacing=0.0>
            <CreationChild workspace pick creating={creating.clone()} />
            <CreateFrames cancel open phone title>
                <CreateBody workspace={body} pick act creating />
            </CreateFrames>
        </List>
    }
}

#[component]
fn CreationChild(
    workspace: Rc<Workspace>,
    pick: u64,
    creating: Memo<Option<Creating>>,
) -> NodeId {
    let placed = create_memo(move || {
        creating.with(|creating| {
            creating
                .as_ref()
                .filter(|creating| !creating.template.dialog)
                .map(creation_content)
        })
    });
    let editor = workspace.editor().clone();
    view! {
        <Frame width=1.0 height=1.0>
            <Subregion
                editor={editor}
                placed={placed}
                mode=ChildMode::Live
                punch=false
                on_state={move |state: ChildState| workspace.creation_state(pick, &state)}
            />
        </Frame>
    }
}

fn creation_content(creating: &Creating) -> SubregionContent {
    SubregionContent::Creation {
        editor: creating.template.editor,
        template: creating.template.template.clone(),
    }
}

#[component]
fn CreateFrames(
    cancel: Act,
    open: Memo<bool>,
    phone: Memo<bool>,
    title: Memo<String>,
    children: Child,
) -> NodeId {
    let wide = create_memo(clone!(open phone -> move || open.get() && !phone.get()));
    let narrow = create_memo(clone!(open phone -> move || open.get() && phone.get()));
    let in_dialog = create_memo(clone!(wide -> move || wide.get().then_some(children)));
    let in_sheet = create_memo(clone!(narrow -> move || narrow.get().then_some(children)));
    let dismiss = Rc::clone(&cancel);
    let sheet_title = title.clone();
    view! {
        <List spacing=0.0>
            <Dialog
                open={wide}
                title={title}
                width=360.0
                on_dismiss={move || dismiss(PickAction::CancelCreation)}
            >
                <Portal node={in_dialog} />
            </Dialog>
            <ModalSheet
                open={narrow}
                fit=true
                on_close={move || cancel(PickAction::CancelCreation)}
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
fn CreateBody(
    workspace: Rc<Workspace>,
    pick: u64,
    act: Act,
    creating: Memo<Option<Creating>>,
) -> NodeId {
    let (state, set_state) = create_signal(ChildState::default());
    let working = create_memo(clone!(creating -> move || {
        creating.with(|creating| creating.as_ref().is_some_and(|creating| creating.committed))
    }));
    let waiting_label = create_memo(clone!(creating -> move || {
        format!(
            "Creating {}...",
            creating.get().map(|creating| creating.template.name).unwrap_or_default()
        )
    }));
    let options = create_memo(clone!(working -> move || !working.get()));
    let not_ready = create_memo(clone!(state -> move || {
        !matches!(
            state.with(|state| state.creation.clone()),
            Some(CreationProgress::Options { ready: true })
        )
    }));
    let height = create_memo(move || {
        state.with(|state| state.intrinsic_size.map_or(CREATION_HEIGHT, |size| size.y.max(1.0)))
    });
    let placed = create_memo(move || {
        creating.with(|creating| {
            creating
                .as_ref()
                .filter(|creating| creating.template.dialog)
                .map(creation_content)
        })
    });
    let editor = workspace.editor().clone();
    let (create_act, cancel_act) = (Rc::clone(&act), act);
    view! {
        <List spacing=10.0>
            <Frame height={height}>
                <Subregion
                    editor={editor}
                    placed={placed}
                    mode=ChildMode::Live
                    @test_id={"picker.creation"}
                    on_state={move |next: ChildState| {
                        workspace.creation_state(pick, &next);
                        set_state.set(next);
                    }}
                />
            </Frame>
            <Separator />
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Show condition={options}>
                    {move || clone!(create_act not_ready -> view! {
                        <Button
                            label="Create"
                            variant=ButtonVariant::Primary
                            disabled={not_ready}
                            @test_id={"picker.create"}
                            on_click={move || create_act(PickAction::Create)}
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
                    on_click={move || cancel_act(PickAction::CancelCreation)}
                />
            </List>
        </List>
    }
}

#[component]
fn PickerError(error: Memo<Option<String>>, act: Act) -> NodeId {
    let theme = use_theme();
    let open = create_memo(clone!(error -> move || error.get().is_some()));
    let text = create_memo(move || error.get().unwrap_or_default());
    let dismiss = Rc::clone(&act);
    view! {
        <Dialog
            open={open}
            title="Block picker error"
            width=320.0
            on_dismiss={move || dismiss(PickAction::DismissError)}
        >
            <List spacing=10.0>
                <Caption content={text} color={theme.danger.clone()} />
                <Button
                    label="Dismiss"
                    variant=ButtonVariant::Secondary
                    on_click={move || act(PickAction::DismissError)}
                />
            </List>
        </Dialog>
    }
}
