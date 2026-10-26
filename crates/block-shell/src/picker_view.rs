use beui::Overflow;
use block_editor_beui::beui::dock::styled::CHROME_BORDER;
use block_editor_beui::beui::dock::styled::{DockDialog, DockDialogPresentation, DockNode, TabId};
use block_editor_beui::beui::icons::{
    ICON_ARROW_LEFT, ICON_BLOCKS, ICON_CHEVRON_RIGHT, ICON_FILE_UP, ICON_FOLDER, ICON_LINK,
    ICON_SEARCH,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;

use block_editor_beui::beui::{Color32, NodeId, OverlayAnchor, Placement, Pos2, TextAlign, Vec2};
use block_editor_beui::block_ui::TemplateEntry;
use block_editor_beui::{
    BlockParent, ChildMode, ChildState, Subregion, SubregionContent, label_glyph,
};

use super::picker::{
    Creating, Pick, PickAction, PickTree, Stage, TreeKey, TreeRow, TypeGroup, common, groups,
    link_rows, place_rows, slides, tile_key,
};
use super::workspace::Workspace;

const MENU_WIDTH: Size = Size::Custom(256.0);
const SURFACE_SIZE: Vec2 = Vec2::new(520.0, 560.0);
const DIALOG_HEIGHT: Size = Size::Custom(460.0);
const SIDE_BLEED: Bleed = Bleed::DIALOG.top(false).bottom(false);
const HEADER_SPACING: Space = Space::Md;
const ROW_SPACING: Space = Space::Md;
const GROUP_SPACING: Space = Space::Xs;
const CREATION_HEIGHT: f32 = 96.0;
const CREATION_INSET: Space = Space::Sm;
const PICKER_TABS: u64 = 1 << 42;

type Act = Callback<PickAction>;

pub(crate) fn picker_tab(pick: u64) -> TabId {
    TabId::new(PICKER_TABS + pick)
}

pub(crate) fn is_picker_tab(tab: TabId) -> bool {
    tab.value() >= PICKER_TABS
}

fn held_memo(workspace: Workspace, pick: u64) -> Memo<Option<Pick>> {
    create_memo(move || workspace.pick(pick))
}

fn act_on(workspace: Workspace, pick: u64) -> Act {
    Callback::new(move |action: PickAction| workspace.pick_action(pick, action))
}

#[component]
pub fn Pickers(workspace: Workspace) -> NodeId {
    let picks = workspace.picks;
    let ids = create_memo(move || {
        picks.with(|picks| picks.iter().map(|held| held.pick).collect::<Vec<_>>())
    });
    view! {
        <List spacing=Space::None>
            <ForEach keys={ids}>|pick: u64|
                <Picker workspace pick />
            </ForEach>
        </List>
    }
}

#[component]
pub fn PickerDialog(workspace: Workspace, pick: u64, tab: TabId) -> DockNode<TabId> {
    let choosing = create_memo(move || {
        workspace
            .pick(pick)
            .is_some_and(|held| held.creating.is_none())
    });
    let anchor = workspace.pick_anchor(pick).and_then(|anchor| anchor.rect);
    let size = create_memo(move || match choosing.get() {
        true => SURFACE_SIZE,
        false => Vec2::new(
            SURFACE_SIZE.x,
            creation_height(workspace.pick(pick).as_ref()) + 2.0 * CHROME_BORDER,
        ),
    });
    view! {
        <DockDialog
            id={picker_tab(pick)}
            parent={tab}
            title="Add block"
            size={size}
            presentation=DockDialogPresentation::Card
            anchor
            dismiss_on_outside_press={choosing}
            on_close={move || close_surface(workspace, pick)}
        >
            <PickerSurface workspace pick />
        </DockDialog>
    }
}

fn close_surface(workspace: Workspace, pick: u64) {
    let creating = untrack(|| workspace.pick(pick)).and_then(|held| held.creating);
    match creating {
        Some(Creating {
            child: Some(child), ..
        }) => workspace.host().dismiss_child(child),
        Some(_) => workspace.pick_action(pick, PickAction::CancelCreation),
        None => workspace.pick_action(pick, PickAction::Close),
    }
}

#[component]
fn Picker(workspace: Workspace, pick: u64) -> NodeId {
    let held = held_memo(workspace, pick);
    let act = act_on(workspace, pick);
    let error = create_memo(move || held.get().and_then(|held| held.error));
    let docked = held
        .get_untracked()
        .is_some_and(|held| held.anchor.tab.is_some());
    let centred = match docked {
        true => view! {
            <Frame />
        },
        false => view! {
            <CentredSurface workspace pick held />
        },
    };
    let templates = held
        .get_untracked()
        .is_some_and(|held| held.filter.templates);
    let placing = held
        .get_untracked()
        .is_some_and(|held| held.place.is_some());
    view! {
        <List spacing=Space::None>
            <PickerMenu workspace held act templates placing />
            {centred}
            <CreationChild workspace pick held />
            <PickerError error act />
        </List>
    }
}

fn menu_anchor(workspace: Workspace, held: &Pick) -> (OverlayAnchor, Placement) {
    if let Some(rect) = held.anchor.rect {
        return (OverlayAnchor::Rect(rect), Placement::BelowStart);
    }
    match held.anchor.tab.and_then(|tab| workspace.tab_node(tab)) {
        Some(node) => (OverlayAnchor::Node(node), Placement::InsideTop),
        None => (OverlayAnchor::Point(Pos2::ZERO), Placement::Center),
    }
}

#[component]
fn PickerMenu(
    workspace: Workspace,
    held: Memo<Option<Pick>>,
    act: Act,
    templates: bool,
    placing: bool,
) -> NodeId {
    let open = create_memo(move || held.with(|held| held.as_ref().is_some_and(Pick::menu_open)));
    let placed = create_memo(move || {
        held.with(|held| held.as_ref().map(|held| menu_anchor(workspace, held)))
            .unwrap_or((OverlayAnchor::Point(Pos2::ZERO), Placement::Center))
    });
    let anchor = create_memo(move || placed.get().0);
    let placement = create_memo(move || placed.get().1);
    let phone = workspace.phone;
    let touch = held.get_untracked().is_some_and(|held| held.touch);
    let sheet = create_memo(move || touch || phone.get());
    let items = match templates {
        true => view! {
            <SlideItems workspace held act />
        },
        false => view! {
            <TypeItems workspace held act placing />
        },
    };
    view! {
        <MenuPopup
            items
            anchor
            placement
            open
            sheet
            min_width=MENU_WIDTH
            on_dismiss={move || act.call(PickAction::Close)}
        />
    }
}

#[component]
fn TypeItems(
    workspace: Workspace,
    held: Memo<Option<Pick>>,
    act: Act,
    placing: bool,
) -> Children<MenuItem> {
    let common = create_memo(move || {
        let types = workspace.types();
        held.with(|held| {
            held.as_ref()
                .map(|held| common(&types, held))
                .unwrap_or_default()
        })
    });
    let keys = create_memo(move || common.get().iter().map(tile_key).collect::<Vec<_>>());
    let first = create_memo(move || keys.get().first().cloned());
    let row = move |key: String| {
        let entry = create_memo({
            let key = key.clone();
            move || {
                common
                    .get()
                    .into_iter()
                    .find(|entry| tile_key(entry) == key)
            }
        });
        let separated = create_memo({
            let key = key.clone();
            move || placing && first.get().as_ref() == Some(&key)
        });
        view! {
            <TypeItem act key entry separated />
        }
    };
    match placing {
        true => view! {
            <LocationItem workspace held act />
            <ForEach keys>|key: String| { row(key) }</ForEach>
            <MoreItem act label="More blocks…" />
            <LinkItem act />
            <ImportItem held act />
        },
        false => view! {
            <ForEach keys>|key: String| { row(key) }</ForEach>
            <MoreItem act label="More blocks…" />
            <LinkItem act />
            <ImportItem held act />
        },
    }
}

#[component]
fn MoreItem(act: Act, label: &'static str) -> MenuItem {
    view! {
        <MenuItem
            label
            glyph=ICON_BLOCKS
            separated=true
            then=MenuThen::Stay
            row_test_id="picker.more"
            on_click={move || act.call(PickAction::Stage(Stage::More))}
        />
    }
}

#[component]
fn SlideItems(workspace: Workspace, held: Memo<Option<Pick>>, act: Act) -> Children<MenuItem> {
    let slides = create_memo(move || {
        let types = workspace.types();
        held.with(|held| {
            held.as_ref()
                .map(|held| slides(&types, held))
                .unwrap_or_default()
        })
    });
    let keys = create_memo(move || slides.get().iter().map(tile_key).collect::<Vec<_>>());
    let caption = held.get_untracked().map_or_else(String::new, |held| {
        format!("New {}", held.filter.name.to_lowercase())
    });
    view! {
        <MenuCaption label={caption} />
        <MenuGrid columns=2 row_test_id="picker.slides">
            <ForEach keys>|key: String| {
                let entry = create_memo({ let key = key.clone(); move || {
                    slides.get().into_iter().find(|entry| tile_key(entry) == key)
                } });
                view! {
                    <TypeItem act key entry separated={create_memo(|| false)} />
                }
            }</ForEach>
        </MenuGrid>
        <MoreItem act label="Other blocks…" />
        <LinkItem act />
    }
}

#[component]
fn TypeItem(
    act: Act,
    key: String,
    entry: Memo<Option<TemplateEntry>>,
    separated: Memo<bool>,
) -> MenuItem {
    let glyph_color = create_memo(move || {
        entry
            .get()
            .and_then(|entry| entry.color)
            .map(|[r, g, b]| Color32::from_rgb(r, g, b))
    });
    let label = create_memo(move || entry.get().map(|entry| entry.name).unwrap_or_default());
    let glyph = create_memo(move || entry.get().and_then(|entry| entry.icon));
    view! {
        <MenuItem
            label
            glyph
            glyph_color
            separated
            then=MenuThen::Stay
            row_test_id={format!("picker.menu.{key}")}
            on_click={move || {
                if let Some(entry) = entry.get_untracked() {
                    act.call(PickAction::Make(entry));
                }
            }}
        />
    }
}

#[component]
fn LinkItem(act: Act) -> MenuItem {
    view! {
        <MenuItem
            label="Link existing…"
            glyph=ICON_LINK
            then=MenuThen::Stay
            row_test_id="picker.link-existing"
            on_click={move || act.call(PickAction::Stage(Stage::Link))}
        />
    }
}

#[component]
fn ImportItem(held: Memo<Option<Pick>>, act: Act) -> MenuItem {
    let importing = create_memo(move || {
        held.with(|held| held.as_ref().is_some_and(|held| held.importing.is_some()))
    });
    let label = create_memo(move || match importing.get() {
        true => "Importing…".to_owned(),
        false => "Import a .block file…".to_owned(),
    });
    view! {
        <MenuItem
            label
            glyph=ICON_FILE_UP
            disabled={importing}
            then=MenuThen::Stay
            row_test_id="picker.import"
            on_click={move || act.call(PickAction::Import)}
        />
    }
}

#[component]
fn LocationItem(workspace: Workspace, held: Memo<Option<Pick>>, act: Act) -> MenuItem {
    let place = create_memo(move || held.with(|held| held.as_ref().and_then(|held| held.place)));
    let label = create_memo(move || {
        let types = workspace.types();
        match place.get() {
            Some(BlockParent::Block(id)) => workspace
                .info(id)
                .map(|info| info.label(types.as_ref()).name)
                .unwrap_or_else(|| "Untitled".to_owned()),
            _ => "Top level".to_owned(),
        }
    });
    let glyph = create_memo(move || {
        let types = workspace.types();
        place
            .get()
            .and_then(BlockParent::block)
            .and_then(|id| workspace.info(id))
            .and_then(|info| label_glyph(&info.label(types.as_ref())))
            .unwrap_or(ICON_FOLDER)
            .to_owned()
    });
    let rows = create_memo(move || {
        let types = workspace.types();
        held.with(|held| {
            held.as_ref()
                .map(|held| {
                    workspace.tree(PickTree::Place, |graph| place_rows(&types, held, graph))
                })
                .unwrap_or_default()
        })
    });
    let keys = create_memo(move || {
        rows.get()
            .into_iter()
            .map(|row| row.key)
            .collect::<Vec<_>>()
    });
    view! {
        <MenuItem lead="New in" label glyph page=true row_test_id="picker.location">
            <ForEach keys>|key: TreeKey| {
                let row = create_memo({ let key = key.clone(); move || {
                    rows.get().into_iter().find(|row| row.key == key)
                } });
                view! {
                    <PlaceItem act place key row />
                }
            }</ForEach>
        </MenuItem>
    }
}

#[component]
fn PlaceItem(
    act: Act,
    place: Memo<Option<BlockParent>>,
    key: TreeKey,
    row: Memo<Option<TreeRow>>,
) -> MenuItem {
    let id = key.id();
    let label = create_memo(move || row.get().map(|row| row.name).unwrap_or_default());
    let glyph = create_memo(move || row.get().and_then(|row| row.icon));
    let depth = create_memo(move || row.get().map(|row| row.depth));
    let unfolded = key.clone();
    let expanded = Binding::new(
        Prop::dynamic(move || {
            row.get()
                .filter(|row| row.expandable)
                .map(|row| row.expanded)
        }),
        move |open: Option<bool>| {
            act.call(PickAction::UnfoldPlace(
                unfolded.clone(),
                open == Some(true),
            ))
        },
    );
    let checked =
        create_memo(move || Some(id.is_some() && place.get().and_then(BlockParent::block) == id));
    let named = id.map_or_else(String::new, |id| format!("picker.place.{id}"));
    view! {
        <MenuItem
            label
            glyph
            depth
            expanded
            checked
            then=MenuThen::Back
            row_test_id={named}
            on_click={move || {
                if let Some(id) = id {
                    act.call(PickAction::Place(BlockParent::Block(id)));
                }
            }}
        />
    }
}

#[component]
fn CentredSurface(workspace: Workspace, pick: u64, held: Memo<Option<Pick>>) -> NodeId {
    let open = create_memo(move || held.with(|held| held.as_ref().is_some_and(Pick::surface_open)));
    let close = move || close_surface(workspace, pick);
    view! {
        <DialogFrame
            open={open}
            width={Size::Custom(SURFACE_SIZE.x)}
            narrow={workspace.phone}
            on_dismiss={close}
        >|| {
            let layout = use_context::<DialogLayout>();
            let creating = create_memo(move || held.with(|held| held.as_ref().is_some_and(|held| held.creating.is_some())));
            let height = create_memo(move || match layout.is_some_and(|layout| layout.sheet.get()) || creating.get() {
                true => Length::INTRINSIC,
                false => Length::Size(DIALOG_HEIGHT),
            });
            view! {
                <Frame height>
                    <PickerSurface workspace pick />
                </Frame>
            }
        }</DialogFrame>
    }
}

#[component]
fn PickerSurface(workspace: Workspace, pick: u64) -> NodeId {
    let held = held_memo(workspace, pick);
    let act = act_on(workspace, pick);
    let shown = create_memo(move || {
        held.with(|held| match held.as_ref() {
            Some(held)
                if held
                    .creating
                    .as_ref()
                    .is_some_and(|creating| creating.template.dialog) =>
            {
                Some(None)
            }
            Some(held) => Some(Some(held.stage)),
            None => None,
        })
    });
    view! {
        <List spacing=Space::None>
            <Dynamic value={shown}>|shown: Option<Option<Stage>>| {
                match shown {
                    Some(None) => view! {
                        <CreateBody @sizing=ItemSize::Intrinsic workspace pick held />
                    },
                    Some(Some(Stage::Link)) => view! {
                        <LinkBody @sizing=ItemSize::Percent(100.0) workspace held act />
                    },
                    Some(Some(Stage::More | Stage::Menu)) | None => view! {
                        <MoreBody @sizing=ItemSize::Percent(100.0) workspace held act />
                    },
                }
            }</Dynamic>
        </List>
    }
}

fn target_name(workspace: Workspace, held: &Pick) -> Option<String> {
    let block = held
        .place
        .and_then(BlockParent::block)
        .or_else(|| held.parent.block())?;
    let types = workspace.types();
    workspace
        .info(block)
        .map(|info| info.label(types.as_ref()).name)
}

#[component]
fn SurfaceHeader(
    title: Memo<String>,
    search: Memo<String>,
    placeholder: &'static str,
    act: Act,
) -> NodeId {
    let field = NodeRef::new();
    let built = view! {
        <List spacing=HEADER_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=HEADER_SPACING>
                <Button
                    icon_only=true
                    @test_id={"picker.back"}
                    glyph=ICON_ARROW_LEFT
                    label="Back to the menu"
                    variant=ButtonVariant::Ghost
                    on_click={move || act.call(PickAction::Stage(Stage::Menu))}
                />
                <Text @sizing=ItemSize::Percent(100.0) style=TextStyle::TITLE>{title}</Text>
            </List>
            <TextInput
                @node_ref={&field}
                value={Binding::new(search, move |value: String| act.call(PickAction::Search(value)))}
                placeholder
                label="Search"
                glyph=ICON_SEARCH
                @test_id={"picker.search"}
            />
        </List>
    };
    if let Some(node) = field.try_get() {
        with_document(|document| document.focus_into(node));
    }
    built
}

fn search_of(held: Memo<Option<Pick>>) -> Memo<String> {
    create_memo(move || {
        held.with(|held| {
            held.as_ref()
                .map(|held| held.search.clone())
                .unwrap_or_default()
        })
    })
}

#[component]
fn MoreBody(workspace: Workspace, held: Memo<Option<Pick>>, act: Act) -> NodeId {
    let title = create_memo(move || {
        held.with(|held| held.as_ref().and_then(|held| target_name(workspace, held)))
            .map_or_else(
                || "Add a block".to_owned(),
                |name| format!("Add a block to {name}"),
            )
    });
    let groups = create_memo(move || {
        let types = workspace.types();
        held.with(|held| {
            held.as_ref()
                .map(|held| groups(&types, held))
                .unwrap_or_default()
        })
    });
    let titles = create_memo(move || {
        groups
            .get()
            .into_iter()
            .map(|group| group.title)
            .collect::<Vec<_>>()
    });
    let empty = create_memo(move || titles.get().is_empty());
    view! {
        <DialogBody
            id="picker"
            scrolls=false
            actions={vec![DialogAction::cancel("Cancel")]}
            on_dismiss={move || act.call(PickAction::Close)}
        >
            <SurfaceHeader title search={search_of(held)} placeholder="Search block types" act />
            <Scroll @sizing=ItemSize::Percent(100.0) bleed=SIDE_BLEED>
                <List spacing=ROW_SPACING>
                    <Show condition={empty}>
                        <Text style=TextStyle::CAPTION>"No block types match."</Text>
                    </Show>
                    <ForEach keys={titles}>|title: String| {
                        let group = create_memo({ let title = title.clone(); move || {
                            groups.get().into_iter().find(|group| group.title == title)
                        } });
                        view! {
                            <TypeGroupView act group />
                        }
                    }</ForEach>
                </List>
            </Scroll>
        </DialogBody>
    }
}

#[component]
fn TypeGroupView(act: Act, group: Memo<Option<TypeGroup>>) -> NodeId {
    let title = create_memo(move || group.get().map(|group| group.title).unwrap_or_default());
    let types = create_memo(move || group.get().map(|group| group.types).unwrap_or_default());
    let keys = create_memo(move || types.get().iter().map(tile_key).collect::<Vec<_>>());
    view! {
        <List spacing=GROUP_SPACING>
            <Text style=TextStyle::CAPTION>{title}</Text>
            <ForEach keys>|key: String| {
                let entry = create_memo({ let key = key.clone(); move || {
                    types.get().into_iter().find(|entry| tile_key(entry) == key)
                } });
                view! {
                    <TypeRow act key entry />
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn TypeRow(act: Act, key: String, entry: Memo<Option<TemplateEntry>>) -> NodeId {
    let glyph_color = create_memo(move || {
        let color = entry.get().and_then(|entry| entry.color);
        Some(color.map_or(TextColor::Muted, |[r, g, b]| {
            TextColor::Custom(Color32::from_rgb(r, g, b))
        }))
    });
    let label = create_memo(move || entry.get().map(|entry| entry.name).unwrap_or_default());
    let glyph = create_memo(move || entry.get().and_then(|entry| entry.icon));
    view! {
        <ListRow
            @test_id={format!("picker.more.{key}")}
            label
            glyph
            glyph_color
            size=RowSize::Regular
            on_click={move || {
                if let Some(entry) = entry.get_untracked() {
                    act.call(PickAction::Make(entry));
                }
            }}
        />
    }
}

#[component]
fn LinkBody(workspace: Workspace, held: Memo<Option<Pick>>, act: Act) -> NodeId {
    let title = create_memo(move || {
        held.with(|held| held.as_ref().and_then(|held| target_name(workspace, held)))
            .map_or_else(
                || "Link a block".to_owned(),
                |name| format!("Link a block into {name}"),
            )
    });
    let rows = create_memo(move || {
        let types = workspace.types();
        held.with(|held| {
            held.as_ref()
                .map(|held| workspace.tree(PickTree::Link, |graph| link_rows(&types, held, graph)))
                .unwrap_or_default()
        })
    });
    let keys = create_memo(move || {
        rows.get()
            .into_iter()
            .map(|row| row.key)
            .collect::<Vec<_>>()
    });
    let empty = create_memo(move || keys.get().is_empty());
    let selected = create_memo(move || {
        let key = held.with(|held| held.as_ref().and_then(|held| held.selected.clone()))?;
        rows.with(|rows| rows.iter().find(|row| row.key == key).cloned())
    });
    let chosen = create_memo(move || selected.get().filter(|row| row.enabled));
    let unready = create_memo(move || chosen.get().is_none());
    let above = create_memo(move || selected.get().map(|row| row.above).unwrap_or_default());
    let has_above = create_memo(move || !above.get().is_empty());
    let name = create_memo(move || selected.get().map(|row| row.name).unwrap_or_default());
    let link = move |row: TreeRow| {
        if let (true, Some(id)) = (row.enabled, row.key.id()) {
            act.call(PickAction::Link(id, row.block_type));
        }
    };
    let item = move |key: TreeKey| {
        rows.with(|rows| {
            rows.iter().find(|row| row.key == key).map(|row| TreeItem {
                label: row.name.clone(),
                depth: row.depth,
                expandable: row.expandable,
                expanded: row.expanded,
            })
        })
        .unwrap_or(TreeItem {
            label: String::new(),
            depth: 0,
            expandable: false,
            expanded: false,
        })
    };
    let selected_key =
        create_memo(move || held.with(|held| held.as_ref().and_then(|held| held.selected.clone())));
    let empty_text = create_memo(move || {
        match held.with(|held| {
            held.as_ref()
                .is_none_or(|held| held.search.trim().is_empty())
        }) {
            true => "No blocks are available to link.".to_owned(),
            false => "No matching blocks.".to_owned(),
        }
    });
    let named = create_memo(move || !name.get().is_empty());
    view! {
        <DialogBody
            id="picker"
            scrolls=false
            actions={vec![
                DialogAction::cancel("Cancel"),
                DialogAction::primary("Link", move || {
                    if let Some(row) = chosen.get_untracked() {
                        link(row);
                    }
                })
                .disabled(unready),
            ]}
            on_dismiss={move || act.call(PickAction::Close)}
        >
            <SurfaceHeader
                title
                search={search_of(held)}
                placeholder="Search by name or UUID"
                act
            />
            <Show condition={empty}>
                <Text style=TextStyle::CAPTION>{empty_text}</Text>
            </Show>
            <Tree
                @sizing=ItemSize::Percent(100.0)
                keys
                item
                selected={selected_key}
                spacing=Space::Xs2
                bleed=SIDE_BLEED
                row_test_id={move |key: TreeKey| match key.id() {
                    Some(id) => format!("picker.link.{id}"),
                    None => "picker.link.shared".to_owned(),
                }}
                on_select={move |key: TreeKey| act.call(PickAction::Select(key))}
                on_open={move |key: TreeKey| {
                    if let Some(row) = rows.with_untracked(|rows| rows.iter().find(|row| row.key == key).cloned()) {
                        link(row);
                    }
                }}
                on_expand={move |(key, open): (TreeKey, bool)| act.call(PickAction::Unfold(key, open))}
            >|face: TreeRowFace<TreeKey>| {
                let key = face.key.clone();
                let row = create_memo(move || rows.with(|rows| rows.iter().find(|row| row.key == key).cloned()));
                view! {
                    <LinkRowFace row />
                }
            }</Tree>
            <Show condition={named}>
                <List direction=Direction::Horizontal align=Align::Center spacing=HEADER_SPACING>
                    <Show condition={has_above}>
                        <Text size=TextSize::Sm color=TextColor::Muted overflow=Overflow::Ellipsis>
                            {above}
                        </Text>
                        <Text size=TextSize::Sm color=TextColor::Muted>
                            <Icon glyph=ICON_CHEVRON_RIGHT />
                        </Text>
                    </Show>
                    <Text size=TextSize::Sm overflow=Overflow::Ellipsis>{name}</Text>
                </List>
            </Show>
        </DialogBody>
    }
}

#[component]
fn LinkRowFace(row: Memo<Option<TreeRow>>) -> NodeId {
    let glyph = create_memo(move || row.get().and_then(|row| row.icon));
    let label = create_memo(move || row.get().map(|row| row.name).unwrap_or_default());
    let detail = create_memo(move || row.get().map(|row| row.detail).unwrap_or_default());
    let section = create_memo(move || row.get().is_some_and(|row| row.section));
    let color =
        create_memo(
            move || match row.get().is_some_and(|row| row.enabled || row.section) {
                true => TextColor::Default,
                false => TextColor::Muted,
            },
        );
    let weight = create_memo(move || match section.get() {
        true => Weight::Bold,
        false => Weight::Normal,
    });
    view! {
        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Sm>
            <Text color=TextColor::Muted size=TextSize::Sm align=TextAlign::Center>
                <Icon glyph={glyph} />
            </Text>
            <Text @sizing=ItemSize::Percent(100.0) color weight overflow=Overflow::Ellipsis>
                {label}
            </Text>
            <Text size=TextSize::Sm color=TextColor::Muted>{detail}</Text>
        </List>
    }
}

#[component]
fn CreationChild(workspace: Workspace, pick: u64, held: Memo<Option<Pick>>) -> NodeId {
    let placed = create_memo(move || {
        held.with(|held| {
            held.as_ref()
                .and_then(|held| held.creating.as_ref())
                .filter(|creating| !creating.template.dialog)
                .map(creation_content)
        })
    });
    let editor = workspace.editor();
    view! {
        <Frame width=Size::Custom(0.0) height=Size::Custom(0.0)>
            <Frame width=Size::Custom(1.0) height=Size::Custom(1.0)>
                <Subregion
                    editor={editor}
                    placed={placed}
                    mode=ChildMode::Live
                    punch=false
                    on_state={move |state: ChildState| workspace.creation_state(pick, &state)}
                />
            </Frame>
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
fn CreateBody(workspace: Workspace, pick: u64, held: Memo<Option<Pick>>) -> NodeId {
    let creating =
        create_memo(move || held.with(|held| held.as_ref().and_then(|held| held.creating.clone())));
    let height =
        create_memo(move || Length::Size(Size::Custom(creation_height(held.get().as_ref()))));
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
        <Frame padding=CREATION_INSET>
            <Frame height=height>
                <Subregion
                    editor={editor}
                    placed={placed}
                    mode=ChildMode::Live
                    dialog=true
                    @test_id={"picker.creation"}
                    on_state={move |next: ChildState| workspace.creation_state(pick, &next)}
                />
            </Frame>
        </Frame>
    }
}

fn creation_height(held: Option<&Pick>) -> f32 {
    held.and_then(|held| held.creating.as_ref())
        .and_then(|creating| creating.height)
        .unwrap_or(CREATION_HEIGHT)
}

#[component]
fn PickerError(error: Memo<Option<String>>, act: Act) -> NodeId {
    let open = create_memo(move || error.get().is_some());
    let text = create_memo(move || error.get().unwrap_or_default());
    view! {
        <Dialog
            open={open}
            title="Block picker error"
            width=Size::Popover
            kind=DialogKind::Alert
            actions={vec![DialogAction::cancel("Dismiss")]}
            on_dismiss={move || act.call(PickAction::DismissError)}
        >
            <Text style=TextStyle::CAPTION color=TextColor::Danger>{text}</Text>
        </Dialog>
    }
}
