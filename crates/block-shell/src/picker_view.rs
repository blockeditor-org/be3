use block_editor_beui::beui::icons::ICON_UPLOAD_FILE;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
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

type Act = Callback<PickAction>;

#[component]
pub fn PickerDialogs(workspace: Workspace) -> NodeId {
    let picks = workspace.picks;
    let ids = create_memo(move || {
        picks.with(|picks| picks.iter().map(|held| held.pick).collect::<Vec<_>>())
    });
    view! {
        <List spacing=0.0>
            <ForEach keys={ids}>|pick: u64|
                <PickerDialog workspace pick />
            </ForEach>
        </List>
    }
}

#[component]
fn PickerDialog(workspace: Workspace, pick: u64) -> NodeId {
    let held = create_memo(move || workspace.pick(pick));
    let act: Act = Callback::new(move |action: PickAction| workspace.pick_action(pick, action));
    let phone = workspace.phone;
    let phone = create_memo(move || phone.get());
    let error = create_memo(move || held.get().and_then(|held| held.error));
    view! {
        <List spacing=0.0>
            <ChooseDialog workspace={workspace} held={held} act={act} phone={phone} />
            <CreateDialog workspace pick held act={act} phone />
            <PickerError error act />
        </List>
    }
}

#[component]
fn ChooseDialog(
    workspace: Workspace,
    held: Memo<Option<Pick>>,
    act: Act,
    phone: Memo<bool>,
) -> NodeId {
    let choosing = create_memo(move || held.with(|held| held.as_ref().is_some_and(Pick::choosing)));
    let wide = create_memo(move || choosing.get() && !phone.get());
    let narrow = create_memo(move || choosing.get() && phone.get());
    let placing = create_memo(move || {
        held.with(|held| held.as_ref().is_some_and(|held| held.place.is_some()))
    });
    let sheet_title = create_memo(move || match placing.get() {
        true => "New file".to_owned(),
        false => "Add block".to_owned(),
    });
    view! {
        <List spacing=0.0>
            <Dialog
                open={wide}
                title="Add block"
                width=PICKER_WIDTH
                scrolls=false
                on_dismiss={move || act.call(PickAction::Close)}
            >
                <List spacing=10.0>
                    <ChooseBody
                        @sizing={ItemSize::Intrinsic.shrink(1.0)}
                        workspace={workspace}
                        held={held}
                        act={act}
                        phone=false
                    />
                    <Separator />
                    <List direction=Direction::Horizontal justify=Justify::End spacing=8.0>
                        <Button
                            label="Close"
                            variant=ButtonVariant::Secondary
                            @test_id={"picker.close"}
                            on_click={move || act.call(PickAction::Close)}
                        />
                    </List>
                </List>
            </Dialog>
            <ModalSheet
                open={narrow}
                rest={SHEET_STOPS[2]}
                on_close={move || act.call(PickAction::Close)}
            >
                <Frame padding={Sides::symmetric(SHEET_PADDING, 4.0)}>
                    <List spacing=10.0>
                        <Text style=TextStyle::TITLE string={sheet_title} />
                        <ChooseBody
                            @sizing=ItemSize::Percent(100.0)
                            workspace={workspace}
                            held={held}
                            act={act}
                            phone=true
                        />
                    </List>
                </Frame>
            </ModalSheet>
        </List>
    }
}

#[component]
fn ChooseBody(workspace: Workspace, held: Memo<Option<Pick>>, act: Act, phone: bool) -> NodeId {
    let tab = create_memo(move || {
        held.with(|held| held.as_ref().map_or(PickerTab::Add, |held| held.tab))
    });
    let tab_index = create_memo(move || match tab.get() {
        PickerTab::Add => 0,
        PickerTab::Templates => 1,
        PickerTab::LinkExisting => 2,
        PickerTab::Import => 3,
    });
    let linking = create_memo(move || tab.get() == PickerTab::LinkExisting);
    let importing = create_memo(move || tab.get() == PickerTab::Import);
    let tiling = create_memo(move || !linking.get() && !importing.get());
    let sections = create_memo(move || {
        let types = workspace.types();
        held.with(|held| {
            held.as_ref()
                .map(|held| sections(&types, held))
                .unwrap_or_default()
        })
    });
    let section_keys = create_memo(move || {
        sections
            .get()
            .into_iter()
            .map(|section| section.key)
            .collect::<Vec<_>>()
    });
    let no_sections = create_memo(move || section_keys.get().is_empty());
    let tab_empty = create_memo(move || match tab.get() {
        PickerTab::Templates => "No templates are available here.".to_owned(),
        _ => "No blocks are available here.".to_owned(),
    });
    let links = create_memo(move || {
        if !linking.get() {
            return Vec::new();
        }
        let types = workspace.types();
        let blocks = workspace.every_block();
        held.with(|held| {
            held.as_ref()
                .map(|held| links(&types, held, &blocks))
                .unwrap_or_default()
        })
    });
    let link_keys = create_memo(move || {
        links
            .get()
            .into_iter()
            .map(|link| link.id)
            .collect::<Vec<_>>()
    });
    let no_links = create_memo(move || link_keys.get().is_empty());
    let empty = create_memo(move || {
        match held.with(|held| {
            held.as_ref()
                .is_none_or(|held| held.search.trim().is_empty())
        }) {
            true => "No blocks are available to link.".to_owned(),
            false => "No matching blocks.".to_owned(),
        }
    });
    let search = create_memo(move || {
        held.with(|held| {
            held.as_ref()
                .map(|held| held.search.clone())
                .unwrap_or_default()
        })
    });
    let height = match phone {
        true => Extent::INTRINSIC,
        false => Extent::at_most(PICKER_HEIGHT),
    };
    let (tab_act, search_act, tile_act, link_act, import_act) = (act, act, act, act, act);
    view! {
        <List spacing=10.0>
            <PlacingFields workspace held act />
            <Tabs
                selected={tab_index}
                on_change={move |index: usize| {
                    let tab = match index {
                        0 => PickerTab::Add,
                        1 => PickerTab::Templates,
                        2 => PickerTab::LinkExisting,
                        _ => PickerTab::Import,
                    };
                    tab_act.call(PickAction::Tab(tab));
                }}
                options={view! {
                    <ChoiceOption label="Add" />
                    <ChoiceOption label="Templates" />
                    <ChoiceOption label="Link existing" />
                    <ChoiceOption label="Import" />
                }}
            />
            <Frame @sizing={ItemSize::Intrinsic.shrink(1.0)} height=height>
                <List spacing=8.0>
                    <Show condition={tiling}>
                        <PickerArea @sizing=ItemSize::Percent(100.0) scrolls={!phone}>
                            <List spacing=16.0>
                                <Show condition={no_sections}>
                                    <Text style=TextStyle::CAPTION string={tab_empty} />
                                </Show>
                                <ForEach keys={section_keys}>|key: String| {
                                    let section = create_memo(move || {
                                        sections.get().into_iter().find(|section| section.key == key)
                                    });
                                    view! {
                                        <TileSectionView act={tile_act} section />
                                    }
                                }</ForEach>
                            </List>
                        </PickerArea>
                    </Show>
                    <Show condition={linking}>
                        <TextInput
                            value={search}
                            placeholder="Search by name or UUID"
                            label="Search"
                            @test_id={"picker.search"}
                            on_change={move |value: String| search_act.call(PickAction::Search(value))}
                        />
                    </Show>
                    <Show condition={importing}>
                        <ImportArea held act={import_act} />
                    </Show>
                    <Show condition={linking}>
                        <PickerArea @sizing=ItemSize::Percent(100.0) scrolls={!phone}>
                            <List spacing=0.0>
                                <Show condition={no_links}>
                                    <Text style=TextStyle::CAPTION string={empty} />
                                </Show>
                                <ForEach keys={link_keys}>|block: Uuid| {
                                    let link = create_memo(move || {
                                        links.get().into_iter().find(|link| link.id == block)
                                    });
                                    view! {
                                        <LinkButton act={link_act} link />
                                    }
                                }</ForEach>
                            </List>
                        </PickerArea>
                    </Show>
                </List>
            </Frame>
        </List>
    }
}

#[component]
fn ImportArea(held: Memo<Option<Pick>>, act: Act) -> NodeId {
    let waiting = create_memo(move || {
        held.with(|held| held.as_ref().is_some_and(|held| held.importing.is_some()))
    });
    let ready = create_memo(move || !waiting.get());
    view! {
        <List spacing=10.0>
            <Text
                style=TextStyle::CAPTION
                wrap=true
                string="Add a block and everything inside it from a .block file. The files pane saves one with Export as, then Block file."
            />
            <Show condition={ready}>
                <List direction=Direction::Horizontal spacing=8.0>
                    <Button
                        label="Choose a block file"
                        glyph={ICON_UPLOAD_FILE.to_owned()}
                        variant=ButtonVariant::Primary
                        @test_id={"picker.import"}
                        on_click={move || act.call(PickAction::Import)}
                    />
                </List>
            </Show>
            <Show condition={waiting}>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Spinner />
                    <Text style=TextStyle::CAPTION string="Importing..." />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn PickerArea(scrolls: bool, children: Child) -> NodeId {
    let scrolls = create_memo(move || scrolls);
    view! {
        <List spacing=0.0>
            <Dynamic value={scrolls}>|scrolls: bool| {
                match scrolls {
                    true => view! {
                        <Scroll @sizing=ItemSize::Percent(100.0)>{children}</Scroll>
                    },
                    false => view! {
                        <List @sizing=ItemSize::Intrinsic spacing=0.0>{children}</List>
                    },
                }
            }</Dynamic>
        </List>
    }
}

#[component]
fn PlacingFields(workspace: Workspace, held: Memo<Option<Pick>>, act: Act) -> NodeId {
    let shown = create_memo(move || {
        held.with(|held| held.as_ref().is_some_and(|held| held.place.is_some()))
    });
    let name = create_memo(move || {
        held.with(|held| {
            held.as_ref()
                .map(|held| held.name.clone())
                .unwrap_or_default()
        })
    });
    let places = create_memo(move || {
        let types = workspace.types();
        let blocks = workspace.every_block();
        held.with(|held| {
            held.as_ref()
                .map(|held| places(&types, held, &blocks))
                .unwrap_or_default()
        })
    });
    let keys = create_memo(move || {
        places
            .get()
            .into_iter()
            .map(|place| place.parent)
            .collect::<Vec<BlockParent>>()
    });
    let chosen = create_memo(move || held.with(|held| held.as_ref().and_then(|held| held.place)));
    let importing = create_memo(move || {
        held.with(|held| held.as_ref().is_some_and(|held| held.importing.is_some()))
    });
    view! {
        <List spacing=0.0>
            <Show condition={shown}>
                <List spacing=8.0>
                    <Text style=TextStyle::CAPTION string="Name" />
                    <TextInput
                        @test_id={"picker.name"}
                        value={name}
                        placeholder="Untitled"
                        label="Name"
                        on_change={move |value: String| act.call(PickAction::Name(value))}
                    />
                    <Text style=TextStyle::CAPTION string="Location" />
                    <Scroll direction=Direction::Horizontal>
                        <ForEach keys={keys}>|parent: BlockParent| {
                            let place = create_memo(move || {
                                places.get().into_iter().find(|place| place.parent == parent)
                            });
                            view! {
                                <PlaceButton act={act} chosen={chosen} importing parent place />
                            }
                        }</ForEach>
                    </Scroll>
                    <Text style=TextStyle::CAPTION string="Choose a type to create it" />
                </List>
            </Show>
        </List>
    }
}

#[component]
fn PlaceButton(
    act: Act,
    chosen: Memo<Option<BlockParent>>,
    importing: Memo<bool>,
    parent: BlockParent,
    place: Memo<Option<Place>>,
) -> NodeId {
    let label = create_memo(move || place.get().map(|place| place.name).unwrap_or_default());
    let glyph = create_memo(move || place.get().map(|place| place.icon).unwrap_or_default());
    let pressed = create_memo(move || chosen.get() == Some(parent));
    let named = match parent {
        BlockParent::Block(block) => format!("picker.place.{block}"),
        BlockParent::Root | BlockParent::Detached => "picker.place.root".to_owned(),
    };
    view! {
        <Frame padding={Sides::horizontal(3.0)}>
            <ToggleButton
                @test_id={named}
                label
                glyph
                pressed
                disabled={importing}
                on_change={move |_: bool| act.call(PickAction::Place(parent))}
            />
        </Frame>
    }
}

#[component]
fn TileSectionView(act: Act, section: Memo<Option<TileSection>>) -> NodeId {
    let title = create_memo(move || {
        section
            .get()
            .map(|section| section.title)
            .unwrap_or_default()
    });
    let glyph = create_memo(move || {
        section
            .get()
            .and_then(|section| section.icon)
            .unwrap_or_default()
    });
    let has_glyph = create_memo(move || !glyph.get().is_empty());
    let tiles = create_memo(move || {
        section
            .get()
            .map(|section| section.tiles)
            .unwrap_or_default()
    });
    let keys = create_memo(move || tiles.get().iter().map(tile_key).collect::<Vec<_>>());
    view! {
        <List spacing=8.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                <Show condition={has_glyph}>
                    <Text align=TextAlign::Center>
                        <Icon glyph={glyph} />
                    </Text>
                </Show>
                <Text style=TextStyle::HEADING string={title} />
            </List>
            <List direction=Direction::Horizontal spacing=8.0 wrap=true>
                <ForEach keys={keys}>|key: String| {
                    let tile = create_memo({ let key = key.clone(); move || {
                        tiles.get().into_iter().find(|tile| tile_key(tile) == key)
                    } });
                    view! {
                        <TileButton act={act} key tile />
                    }
                }</ForEach>
            </List>
        </List>
    }
}

#[component]
fn TileButton(act: Act, key: String, tile: Memo<Option<TemplateEntry>>) -> NodeId {
    let label = create_memo(move || tile.get().map(|tile| tile.name).unwrap_or_default());
    let glyph = create_memo(move || {
        tile.get()
            .and_then(|tile| tile.icon)
            .unwrap_or_default()
            .to_owned()
    });
    let pick = move || {
        if let Some(tile) = tile.get_untracked() {
            act.call(PickAction::Make(tile));
        }
    };
    view! {
        <unstyled::Button
            @test_id={format!("picker.tile.{key}")}
            on_click={pick}
        >|handle: ButtonHandle|
            <TileFace handle label glyph />
        </unstyled::Button>
    }
}

#[component]
fn TileFace(handle: ButtonHandle, label: Memo<String>, glyph: Memo<String>) -> NodeId {
    let theme = use_theme();
    let ButtonHandle {
        hovered, focused, ..
    } = handle;
    let fill = create_memo(move || match hovered.get() {
        true => theme.hover.get(),
        false => theme.surface.get(),
    });
    view! {
        <Frame
            width=TILE_WIDTH
            height=TILE_HEIGHT
            fill={ColorRole::custom(fill)}
            border={Border::new(ColorRole::Border).width(1.0)}
            radius={Radius::custom(5.0)}
            padding={Sides::symmetric(6.0, 10.0)}
        >
            <List spacing=6.0 align=Align::Center>
                <Spacer @sizing=ItemSize::Percent(50.0) />
                <Text size=TextSize::Xl3 align=TextAlign::Center>
                    <Icon glyph={glyph} />
                </Text>
                <Spacer @sizing=ItemSize::Percent(50.0) />
                <Text
                    string={label}
                    size=TextSize::Sm
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
    let label = create_memo(move || link.get().map(|link| link.name).unwrap_or_default());
    let glyph = create_memo(move || link.get().map(|link| link.icon).unwrap_or_default());
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
                    act.call(PickAction::Link(link.id, link.block_type));
                }
            }}
        />
    }
}

#[component]
fn CreateDialog(
    workspace: Workspace,
    pick: u64,
    held: Memo<Option<Pick>>,
    act: Act,
    phone: Memo<bool>,
) -> NodeId {
    let creating =
        create_memo(move || held.with(|held| held.as_ref().and_then(|held| held.creating.clone())));
    let open = create_memo(move || {
        creating.with(|creating| {
            creating
                .as_ref()
                .is_some_and(|creating| creating.template.dialog)
        })
    });
    let title = create_memo(move || {
        format!(
            "New {}",
            creating
                .get()
                .map(|creating| creating.template.name)
                .unwrap_or_default()
        )
    });
    view! {
        <List spacing=0.0>
            <CreationChild workspace pick creating={creating} />
            <CreateFrames cancel={act} open phone title>
                <CreateBody workspace={workspace} pick act creating />
            </CreateFrames>
        </List>
    }
}

#[component]
fn CreationChild(workspace: Workspace, pick: u64, creating: Memo<Option<Creating>>) -> NodeId {
    let placed = create_memo(move || {
        creating.with(|creating| {
            creating
                .as_ref()
                .filter(|creating| !creating.template.dialog)
                .map(creation_content)
        })
    });
    let editor = workspace.editor();
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
    let wide = create_memo(move || open.get() && !phone.get());
    let narrow = create_memo(move || open.get() && phone.get());
    let in_dialog = create_memo(move || wide.get().then_some(children));
    let in_sheet = create_memo(move || narrow.get().then_some(children));
    view! {
        <List spacing=0.0>
            <Dialog
                open={wide}
                title={title}
                width=360.0
                on_dismiss={move || cancel.call(PickAction::CancelCreation)}
            >
                <Portal node={in_dialog} />
            </Dialog>
            <ModalSheet
                open={narrow}
                fit=true
                on_close={move || cancel.call(PickAction::CancelCreation)}
            >
                <Frame padding=SHEET_PADDING>
                    <List spacing=10.0>
                        <Text style=TextStyle::TITLE string={title} />
                        <Portal node={in_sheet} />
                    </List>
                </Frame>
            </ModalSheet>
        </List>
    }
}

#[component]
fn CreateBody(
    workspace: Workspace,
    pick: u64,
    act: Act,
    creating: Memo<Option<Creating>>,
) -> NodeId {
    let (state, set_state) = create_signal(ChildState::default());
    let working = create_memo(move || {
        creating.with(|creating| creating.as_ref().is_some_and(|creating| creating.committed))
    });
    let waiting_label = create_memo(move || {
        format!(
            "Creating {}...",
            creating
                .get()
                .map(|creating| creating.template.name)
                .unwrap_or_default()
        )
    });
    let options = create_memo(move || !working.get());
    let not_ready = create_memo(move || {
        !matches!(
            state.with(|state| state.creation.clone()),
            Some(CreationProgress::Options { ready: true })
        )
    });
    let height = create_memo(move || {
        state.with(|state| {
            state
                .intrinsic_size
                .map_or(CREATION_HEIGHT, |size| size.y.max(1.0))
        })
    });
    let placed = create_memo(move || {
        creating.with(|creating| {
            creating
                .as_ref()
                .filter(|creating| creating.template.dialog)
                .map(creation_content)
        })
    });
    let editor = workspace.editor();
    view! {
        <List spacing=10.0>
            <Frame height=height>
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
                    <Button
                        label="Create"
                        variant=ButtonVariant::Primary
                        disabled={not_ready}
                        @test_id={"picker.create"}
                        on_click={move || act.call(PickAction::Create)}
                    />
                </Show>
                <Show condition={working}>
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Spinner />
                        <Text style=TextStyle::CAPTION string={waiting_label} />
                    </List>
                </Show>
                <Button
                    label="Cancel"
                    variant=ButtonVariant::Secondary
                    on_click={move || act.call(PickAction::CancelCreation)}
                />
            </List>
        </List>
    }
}

#[component]
fn PickerError(error: Memo<Option<String>>, act: Act) -> NodeId {
    let open = create_memo(move || error.get().is_some());
    let text = create_memo(move || error.get().unwrap_or_default());
    view! {
        <Dialog
            open={open}
            title="Block picker error"
            width=320.0
            on_dismiss={move || act.call(PickAction::DismissError)}
        >
            <List spacing=10.0>
                <Text style=TextStyle::CAPTION string={text} color=TextColor::Danger />
                <Button
                    label="Dismiss"
                    variant=ButtonVariant::Secondary
                    on_click={move || act.call(PickAction::DismissError)}
                />
            </List>
        </Dialog>
    }
}
