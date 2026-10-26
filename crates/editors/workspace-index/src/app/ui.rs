use beui::Overflow;
use uuid::Uuid;

use block_editor_beui::beui::icons::{ICON_ARROW_DOWN, ICON_ARROW_UP, ICON_FOLDER};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::beui::{NodeId, TextAlign, Vec2};
use block_editor_beui::{Editor, Toolbar};

use super::entries::{Entry, Folder, FolderSort, Listing};

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

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum FolderView {
    #[default]
    LargeGrid,
    SmallGrid,
    List,
}

impl FolderView {
    const ALL: [Self; 3] = [Self::LargeGrid, Self::SmallGrid, Self::List];

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
    entries: Memo<Vec<Entry>>,
    selected: Signal<Option<Uuid>>,
}

impl Cells {
    fn keys(&self) -> Memo<Vec<Uuid>> {
        let entries = self.entries;
        create_memo(move || {
            entries.with(|entries| entries.iter().map(|entry| entry.reference).collect())
        })
    }

    fn entry(&self, key: Uuid) -> Memo<Option<Entry>> {
        let entries = self.entries;
        create_memo(move || {
            entries.with(|entries| entries.iter().find(|entry| entry.reference == key).cloned())
        })
    }

    fn open(&self, entry: Memo<Option<Entry>>) -> impl Fn() + use<> {
        let host = self.editor.host();
        move || {
            let Some(entry) = entry.get_untracked() else {
                return;
            };
            if let (Some(id), Some(block_type)) = (entry.id, entry.block_type) {
                host.open_block(id, block_type);
            }
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
    let unordered = create_memo(move || sort.get() == FolderSort::Intrinsic);
    let direction_glyph = create_memo(move || match descending.get() {
        true => ICON_ARROW_DOWN,
        false => ICON_ARROW_UP,
    });
    let direction_label = create_memo(move || match descending.get() {
        true => "Descending".to_owned(),
        false => "Ascending".to_owned(),
    });
    let mode_index = create_memo(move || {
        FolderView::ALL
            .iter()
            .position(|shown| *shown == mode.get())
    });
    let sort_index = create_memo(move || {
        FolderSort::ALL
            .iter()
            .position(|order| *order == sort.get())
    });
    let flip = move || {
        descending.set(!descending.get_untracked());
    };
    let cells = Cells {
        editor,
        entries,
        selected,
    };

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let hovering = create_memo(move || drop.get().is_some());
    let outline = create_memo(move || match drop.get() {
        Some(true) => ColorRole::Accent,
        _ => ColorRole::Danger,
    });
    view! {
        <List spacing=Space::None>
            <Toolbar shown={chrome}>
                <Select
                    options={view! {
                        <ChoiceOption label={FolderView::LargeGrid.label()} />
                        <ChoiceOption label={FolderView::SmallGrid.label()} />
                        <ChoiceOption label={FolderView::List.label()} />
                    }}
                    selected={Binding::new(mode_index, move |chosen: Option<usize>| {
                        if let Some(shown) = chosen.and_then(|index| FolderView::ALL.get(index))
                        {
                            mode.set(*shown);
                        }
                    })}
                    label="Layout"
                    @test_id={"folder.view"}
                />
                <Select
                    options={view! {
                        <ChoiceOption label={FolderSort::Intrinsic.label()} />
                        <ChoiceOption label={FolderSort::Name.label()} />
                        <ChoiceOption label={FolderSort::Type.label()} />
                    }}
                    selected={Binding::new(sort_index, move |chosen: Option<usize>| {
                        if let Some(order) = chosen.and_then(|index| FolderSort::ALL.get(index))
                        {
                            sort.set(*order);
                        }
                    })}
                    label="Sort"
                    @test_id={"folder.sort"}
                />
                <Button
                    icon_only=true
                    glyph={direction_glyph}
                    label={direction_label}
                    disabled={unordered}
                    @test_id={"folder.direction"}
                    on_click={flip}
                />
            </Toolbar>
            <Frame
                @sizing=ItemSize::Percent(100.0)
                @node_ref={&content}
                border={create_memo(move || hovering.get().then(|| Border::new(outline.get()).width(DROP_OUTLINE)))}
                padding=PADDING
            >
                <List spacing=Space::None>
                    <Show condition={empty}>
                        <Text
                            style=TextStyle::CAPTION
                            align=TextAlign::Center
                            @test_id={"folder.empty"}
                        >
                            "This folder is empty."
                        </Text>
                    </Show>
                    <Scroll @sizing=ItemSize::Percent(100.0)>
                        <Dynamic value={mode}>|shown: FolderView| {
                            match shown {
                                FolderView::List => view! {
                                    <EntryRows cells={cells} />
                                },
                                FolderView::LargeGrid => view! {
                                    <EntryTiles
                                        cells={cells}
                                        tile=LARGE_TILE
                                        icon_size=LARGE_ICON
                                    />
                                },
                                FolderView::SmallGrid => view! {
                                    <EntryTiles
                                        cells={cells}
                                        tile=SMALL_TILE
                                        icon_size=SMALL_ICON
                                    />
                                },
                            }
                        }</Dynamic>
                    </Scroll>
                </List>
            </Frame>
        </List>
    }
}

#[component]
fn EntryRows(cells: Cells) -> NodeId {
    let keys = cells.keys();
    let selected = cells.selected;
    let chosen = create_selector(move || selected.get());
    view! {
        <List spacing=Space::Xs2>
            <ForEach keys={keys}>|key: Uuid| {
                let entry = cells.entry(key);
                let open = cells.open(entry);
                let here = chosen.memo(Some(key));
                view! {
                    <ListRow
                        selected={here}
                        @test_id={entry_test_id(key)}
                        on_click={move || selected.set(Some(key))}
                        on_activate={open}
                    >
                        <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                            <EntryGlyph entry={entry} size=ROW_ICON />
                            <Text
                                overflow=Overflow::Ellipsis
                                @sizing=ItemSize::Percent(100.0)
                                color={name_color(entry)}
                            >
                                {name_of(entry)}
                            </Text>
                            <Text style=TextStyle::CAPTION>{type_of(entry)}</Text>
                        </List>
                    </ListRow>
                }
            }</ForEach>
        </List>
    }
}

#[component]
fn EntryTiles(cells: Cells, tile: Vec2, icon_size: TextSize) -> NodeId {
    let keys = cells.keys();
    let selected = cells.selected;
    let chosen = create_selector(move || selected.get());
    view! {
        <List direction=Direction::Horizontal wrap=true align=Align::Start spacing=TILE_SPACING>
            <ForEach keys={keys}>|key: Uuid| {
                let entry = cells.entry(key);
                let open = cells.open(entry);
                let here = chosen.memo(Some(key));
                view! {
                    <ListRow
                        @sizing=ItemSize::Fixed(tile.x)
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
