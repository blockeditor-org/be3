use beui::Overflow;
use uuid::Uuid;

use be_sdk_beui::Editor;
use be_sdk_beui::block_menu::{DeleteBlock, ExportAs, Exporter, MoveTo, rename_block};
use be_sdk_beui::beui::dock::styled::{DockBarGrip, DockBarSurface};
use be_sdk_beui::beui::dock::{DockAlign, DockBarPlacement, DockEdge};
use be_sdk_beui::beui::dock::{DockBar, DockPane, DockTab, Docking, DockingLayout};
use be_sdk_beui::beui::icons::{ICON_ARROW_DOWN, ICON_ARROW_UP, ICON_FOLDER, ICON_PENCIL_LINE};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, TextAlign, Vec2};

use super::entries::{Entry, Folder, FolderSort, Listing};
use beui::{Align, Direction, Justify};

const PADDING: Space = Space::Lg;
const TILE_SPACING: Space = Space::Md;
const ROW_HEIGHT: f32 = 24.0;
const INTRINSIC_WIDTH: f32 = 400.0;
const LARGE_TILE: Vec2 = Vec2::new(150.0, 118.0);
const SMALL_TILE: Vec2 = Vec2::new(98.0, 78.0);
const LARGE_ICON: TextSize = TextSize::Xl4;
const SMALL_ICON: TextSize = TextSize::Xl;
const ROW_ICON: TextSize = TextSize::Sm;
const DROP_OUTLINE: Stroke = Stroke::Focus;
const STAGE: &str = "stage";
const TOOLBAR: &str = "toolbar";

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum FolderView {
    #[default]
    LargeGrid,
    SmallGrid,
    List,
}

impl FolderView {
    fn label(self) -> &'static str {
        match self {
            Self::LargeGrid => "Large grid",
            Self::SmallGrid => "Small grid",
            Self::List => "List",
        }
    }
}

#[derive(Clone, Copy)]
struct Cells {
    editor: Editor,
    folder: Folder,
    entries: Memo<Vec<Entry>>,
    selected: Signal<Option<Uuid>>,
    /// The entry whose name is being edited in place.
    renaming: Signal<Option<Uuid>>,
    exporter: Exporter,
}

impl Cells {
    fn keys(&self) -> Memo<Vec<Uuid>> {
        let entries = self.entries;
        create_memo(move || {
            entries.with(|entries| entries.iter().map(|entry| entry.reference).collect())
        })
    }

    fn entry(&self, key: Uuid) -> Memo<Option<Entry>> {
        self.folder.entry(key)
    }

    /// Edits entry `key`'s name in place, if it may be renamed.
    fn rename(&self, key: Uuid) {
        let editable = self
            .folder
            .menu_block_now(key)
            .is_some_and(|block| block.access.can_edit());
        if editable {
            self.renaming.set(Some(key));
        }
    }

    fn open(&self, entry: Memo<Option<Entry>>) -> impl Fn() + use<> {
        let cells = *self;
        move || {
            if let Some(entry) = entry.get_untracked() {
                cells.open_entry(&entry);
            }
        }
    }

    fn open_entry(&self, entry: &Entry) {
        if let (Some(id), Some(block_type)) = (entry.id, entry.block_type) {
            self.editor.host().open_block(id, block_type);
        }
    }
}

#[component]
pub fn FolderEditor(editor: Editor) -> NodeId {
    let index = Listing::of(editor);
    let mode = create_signal(FolderView::default());
    let sort = create_signal(FolderSort::default());
    let descending = create_signal(false);
    let selected = create_signal(None::<Uuid>);

    let folder = Folder::watch(editor, index, sort.read_only(), descending.read_only());
    let entries = folder.entries();
    let drop = super::drop::watch(editor, folder);

    create_effect(move || {
        let rows = entries.with(Vec::len).max(1);
        let height = match mode.get() {
            FolderView::LargeGrid => LARGE_TILE.y * rows.div_ceil(3) as f32,
            FolderView::SmallGrid => SMALL_TILE.y * rows.div_ceil(5) as f32,
            FolderView::List => ROW_HEIGHT * rows as f32,
        };
        editor.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    });

    let empty = create_memo(move || entries.with(Vec::is_empty));
    let shown_mode = create_memo(move || Some(mode.get()));
    let shown_sort = create_memo(move || Some(sort.get()));
    let flip = move || {
        descending.set(!descending.get_untracked());
    };
    let cells = Cells {
        editor,
        folder,
        entries,
        selected,
        renaming: create_signal(None),
        exporter: Exporter::new(editor),
    };

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let layout = DockingLayout::<&'static str>::new();
    view! {
        <Docking layout focus={Some(STAGE)}>
            <DockPane id="stage" locked=true bare=true>
                <DockTab id=STAGE title="Folder">
                    <Frame
                        @node_ref={&content}
                        border={move || drop.get().map(|welcome| {
                            let outline = match welcome {
                                true => ColorRole::Accent,
                                false => ColorRole::Danger,
                            };
                            Border::new(outline).width(DROP_OUTLINE)
                        })}
                    >
                        <List spacing=Space::None>
                            <Show value={empty}>
                                <Frame padding=PADDING>
                                    <Text
                                        style=TextStyle::CAPTION
                                        align=TextAlign::Center
                                        @test_id={"folder.empty"}
                                    >
                                        "This folder is empty."
                                    </Text>
                                </Frame>
                            </Show>
                            <ListItem grow=1.0>
                                <Dynamic value={mode}>|shown: FolderView| {
                                    match shown {
                                        FolderView::List => view! {
                                            <EntryRows cells={cells} />
                                        },
                                        FolderView::LargeGrid => view! {
                                            <Scroll>
                                                <Frame padding=PADDING>
                                                    <EntryTiles
                                                        cells={cells}
                                                        tile=LARGE_TILE
                                                        icon_size=LARGE_ICON
                                                    />
                                                </Frame>
                                            </Scroll>
                                        },
                                        FolderView::SmallGrid => view! {
                                            <Scroll>
                                                <Frame padding=PADDING>
                                                    <EntryTiles
                                                        cells={cells}
                                                        tile=SMALL_TILE
                                                        icon_size=SMALL_ICON
                                                    />
                                                </Frame>
                                            </Scroll>
                                        },
                                    }
                                }</Dynamic>
                            </ListItem>
                        </List>
                    </Frame>
                </DockTab>
            </DockPane>
            <Show value={chrome}>
                <DockBar
                    id=TOOLBAR
                    parent=STAGE
                    placement={DockBarPlacement::Beside(DockEdge::Top, DockAlign::Start)}
                >
                    <DockBarSurface spacing=Space::Md @test_id={"folder.toolbar"}>
                        <DockBarGrip />
                        <ListItem shrink=1.0>
                            <Select
                                options={view! {
                                    <ChoiceOption
                                        value=FolderView::LargeGrid
                                        label={FolderView::LargeGrid.label()}
                                    />
                                    <ChoiceOption
                                        value=FolderView::SmallGrid
                                        label={FolderView::SmallGrid.label()}
                                    />
                                    <ChoiceOption
                                        value=FolderView::List
                                        label={FolderView::List.label()}
                                    />
                                }}
                                selected={Binding::new(shown_mode, move |chosen: Option<FolderView>| {
                                    if let Some(shown) = chosen {
                                        mode.set(shown);
                                    }
                                })}
                                label="Layout"
                                @test_id={"folder.view"}
                            />
                        </ListItem>
                        <ListItem shrink=1.0>
                            <Select
                                options={view! {
                                    <ChoiceOption
                                        value=FolderSort::Intrinsic
                                        label={FolderSort::Intrinsic.label()}
                                    />
                                    <ChoiceOption
                                        value=FolderSort::Name
                                        label={FolderSort::Name.label()}
                                    />
                                    <ChoiceOption
                                        value=FolderSort::Type
                                        label={FolderSort::Type.label()}
                                    />
                                }}
                                selected={Binding::new(shown_sort, move |chosen: Option<FolderSort>| {
                                    if let Some(order) = chosen {
                                        sort.set(order);
                                    }
                                })}
                                label="Sort"
                                @test_id={"folder.sort"}
                            />
                        </ListItem>
                        <Button
                            icon_only=true
                            glyph={move || match descending.get() {
                                true => ICON_ARROW_DOWN,
                                false => ICON_ARROW_UP,
                            }}
                            label={move || match descending.get() {
                                true => "Descending".to_owned(),
                                false => "Ascending".to_owned(),
                            }}
                            disabled={move || sort.get() == FolderSort::Intrinsic}
                            @test_id={"folder.direction"}
                            on_click={flip}
                        />
                    </DockBarSurface>
                </DockBar>
            </Show>
        </Docking>
    }
}

/// The entries as rows, each edited in place and with the menu every block row has (Rename,
/// Move to, Delete, Export as; be_sdk_beui's `block_menu`).
#[component]
fn EntryRows(cells: Cells) -> NodeId {
    let keys = cells.keys();
    let Cells {
        editor,
        selected,
        renaming,
        ..
    } = cells;
    let item = move |key: Uuid| {
        cells
            .folder
            .entry_tracked(key)
            .map(|entry| TreeItem {
                label: entry.name,
                ..TreeItem::default()
            })
            .unwrap_or_default()
    };
    let open = move |key: Uuid| {
        if let Some(entry) = cells.folder.entry_now(key) {
            cells.open_entry(&entry);
        }
    };
    let rename = move |(key, name): (Uuid, String)| {
        if let Some(block) = cells.folder.menu_block_now(key) {
            rename_block(editor, block.id, &name);
        }
        renaming.set(None);
    };
    view! {
        <Tree
            keys={keys}
            item={item}
            selected={selected}
            spacing=Space::Xs2
            padding=PADDING
            row_test_id={entry_test_id}
            menu={move |key: Uuid| Some(view! {
                <EntryMenu cells key />
            })}
            editing={renaming}
            on_select={move |key: Uuid| selected.set(Some(key))}
            on_open={open}
            on_edit={move |key: Uuid| cells.rename(key)}
            on_rename={rename}
            on_cancel_edit={move |_: Uuid| renaming.set(None)}
            leading={move |face: TreeRowFace<Uuid>| {
                Children::from([into_segment(view! {
                    <EntryGlyph entry={cells.entry(face.key)} size=ROW_ICON />
                })])
            }}
        >|face: TreeRowFace<Uuid>| {
            let entry = cells.entry(face.key);
            view! {
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <ListItem grow=1.0>
                        <Text overflow=Overflow::Ellipsis color={name_color(entry)}>
                            {name_of(entry)}
                        </Text>
                    </ListItem>
                    <Text style=TextStyle::CAPTION>{type_of(entry)}</Text>
                </List>
            }
        }</Tree>
    }
}

#[component]
fn EntryMenu(cells: Cells, key: Uuid) -> TreeMenu {
    let Cells {
        editor,
        folder,
        exporter,
        ..
    } = cells;
    let block = folder.menu_block(key);
    let destinations = folder.destinations();
    let named = entry_test_id(key);
    let uneditable = move || !block.with(|block| block.as_ref().is_some_and(|block| block.access.can_edit()));
    TreeMenu {
        items: view! {
            <MenuItem
                label="Rename"
                glyph=ICON_PENCIL_LINE
                disabled={uneditable}
                on_click={move || cells.rename(key)}
            />
            <MoveTo editor block destinations named={format!("{named}.move-to")} />
            <DeleteBlock editor block />
            <ExportAs exporter block named={format!("{named}.export")} />
        },
    }
}

#[component]
fn EntryTiles(cells: Cells, tile: Vec2, icon_size: TextSize) -> NodeId {
    let keys = cells.keys();
    let selected = cells.selected;
    let chosen = create_selector(move || selected.get());
    view! {
        <List direction=Direction::Horizontal wrap=true align=Align::Start spacing=TILE_SPACING>
            <ForEach value={keys}>|key: Uuid| {
                let entry = cells.entry(key);
                let open = cells.open(entry);
                let here = chosen.memo(Some(key));
                view! {
                    <ListItem length={tile.x}>
                        <ListRow
                            selected={here}
                            @test_id={entry_test_id(key)}
                            on_click={move || selected.set(Some(key))}
                            on_activate={open}
                        >
                            <Frame height={Size::Custom(tile.y)}>
                                <List align=Align::Center justify=Justify::Center spacing=Space::Xs>
                                    <EntryGlyph entry={entry} size={icon_size} />
                                    <Text color={name_color(entry)} align=TextAlign::Center>
                                        {name_of(entry)}
                                    </Text>
                                    <Text style=TextStyle::CAPTION align=TextAlign::Center>
                                        {type_of(entry)}
                                    </Text>
                                </List>
                            </Frame>
                        </ListRow>
                    </ListItem>
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn EntryGlyph(entry: Memo<Option<Entry>>, size: TextSize) -> NodeId {
    let glyph = create_memo(move || {
        entry
            .get()
            .and_then(|entry| entry.glyph)
            .unwrap_or(ICON_FOLDER)
    });
    view! {
        <Text size={size} align=TextAlign::Center>
            <Icon glyph={glyph} />
        </Text>
    }
}

fn entry_test_id(key: Uuid) -> String {
    format!("folder.entry.{key}")
}

fn name_of(entry: Memo<Option<Entry>>) -> Memo<String> {
    create_memo(move || entry.get().map(|entry| entry.name).unwrap_or_default())
}

fn type_of(entry: Memo<Option<Entry>>) -> Memo<String> {
    create_memo(move || entry.get().map(|entry| entry.type_name).unwrap_or_default())
}

fn name_color(entry: Memo<Option<Entry>>) -> Memo<TextColor> {
    create_memo(move || {
        let automatic = entry.get().is_none_or(|entry| entry.automatic);
        match automatic {
            true => TextColor::Muted,
            false => TextColor::Default,
        }
    })
}
