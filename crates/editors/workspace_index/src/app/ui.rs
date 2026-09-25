use std::rc::Rc;
use uuid::Uuid;

use block_editor_beui::be_block::FolderContent;
use block_editor_beui::beui::icons::{ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD, ICON_FOLDER};
use block_editor_beui::beui::reactive::{
    Align, Direction, Dynamic, ForEach, Frame, ItemSize, List, Memo, NodeRef, ReadSignal, Show,
    Spacer, WriteSignal, clone, component, create_effect, create_memo, create_selector,
    create_signal, view,
};
use block_editor_beui::beui::styled::{
    Body, Caption, IconButton, IconSized, ListRow, Scroll, Select, use_theme,
};
use block_editor_beui::beui::unstyled::ChoiceOption;
use block_editor_beui::beui::{Color32, NodeId, TextAlign, Vec2};
use block_editor_beui::{Editor, Toolbar};

use super::entries::{Entry, Folder, FolderSort};

const PADDING: f32 = 12.0;
const TILE_SPACING: f32 = 10.0;
const ROW_HEIGHT: f32 = 24.0;
const INTRINSIC_WIDTH: f32 = 400.0;
const LARGE_TILE: Vec2 = Vec2::new(150.0, 118.0);
const SMALL_TILE: Vec2 = Vec2::new(98.0, 78.0);
const LARGE_ICON: f32 = 44.0;
const SMALL_ICON: f32 = 28.0;
const ROW_ICON: f32 = 16.0;
const DROP_OUTLINE: f32 = 2.0;

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

#[derive(Clone)]
struct Cells {
    editor: Editor,
    entries: Memo<Vec<Entry>>,
    selected: ReadSignal<Option<Uuid>>,
    set_selected: WriteSignal<Option<Uuid>>,
}

impl Cells {
    fn keys(&self) -> Memo<Vec<Uuid>> {
        let entries = self.entries.clone();
        create_memo(move || {
            entries.with(|entries| entries.iter().map(|entry| entry.reference).collect())
        })
    }

    fn entry(&self, key: Uuid) -> Memo<Option<Entry>> {
        let entries = self.entries.clone();
        create_memo(move || {
            entries.with(|entries| entries.iter().find(|entry| entry.reference == key).cloned())
        })
    }

    fn open(&self, entry: Memo<Option<Entry>>) -> impl Fn() + use<> {
        let host = self.editor.host().clone();
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
    let index = editor.block_content::<FolderContent>();
    let (mode, set_mode) = create_signal(FolderView::default());
    let (sort, set_sort) = create_signal(FolderSort::default());
    let (descending, set_descending) = create_signal(false);
    let (selected, set_selected) = create_signal(None::<Uuid>);

    let folder = Rc::new(Folder::watch(
        &editor,
        &index,
        sort.clone(),
        descending.clone(),
    ));
    let entries = folder.entries();
    let drop = super::drop::watch(&editor, &folder);

    let adds = Rc::clone(&folder);
    let pumped = index.clone();
    editor.each_frame(move || adds.poll_adds(&pumped));

    let sized = editor.clone();
    create_effect(clone!(entries mode -> move || {
        let rows = entries.with(Vec::len).max(1);
        let height = match mode.get() {
            FolderView::LargeGrid => LARGE_TILE.y * rows.div_ceil(3) as f32,
            FolderView::SmallGrid => SMALL_TILE.y * rows.div_ceil(5) as f32,
            FolderView::List => ROW_HEIGHT * rows as f32,
        };
        sized.set_intrinsic_size(Some(Vec2::new(INTRINSIC_WIDTH, height)));
    }));

    let empty = create_memo(clone!(entries -> move || entries.with(Vec::is_empty)));
    let unordered = create_memo(clone!(sort -> move || sort.get() == FolderSort::Intrinsic));
    let direction_glyph = create_memo(clone!(descending -> move || match descending.get() {
        true => ICON_ARROW_DOWNWARD.to_owned(),
        false => ICON_ARROW_UPWARD.to_owned(),
    }));
    let direction_label = create_memo(clone!(descending -> move || match descending.get() {
        true => "Descending".to_owned(),
        false => "Ascending".to_owned(),
    }));
    let mode_index = create_memo(clone!(mode -> move || {
        FolderView::ALL.iter().position(|shown| *shown == mode.get())
    }));
    let sort_index = create_memo(clone!(sort -> move || {
        FolderSort::ALL.iter().position(|order| *order == sort.get())
    }));
    let flip = clone!(descending set_descending -> move || {
        set_descending.set(!descending.get_untracked());
    });
    let cells = Cells {
        editor: editor.clone(),
        entries,
        selected,
        set_selected,
    };

    let chrome = editor.chrome_shown();
    let content = NodeRef::new();
    editor.content(&content);
    let theme = use_theme();
    let hovering = create_memo(clone!(drop -> move || drop.get().is_some()));
    let outline = create_memo(clone!(drop theme -> move || match drop.get() {
        Some(true) => theme.accent.get(),
        _ => theme.danger.get(),
    }));
    view! {
        <Frame color={theme.background.clone()}>
            <List spacing=0.0>
                <Toolbar shown={chrome}>
                    <Select
                        options={view! {
                            <ChoiceOption label={FolderView::LargeGrid.label()} />
                            <ChoiceOption label={FolderView::SmallGrid.label()} />
                            <ChoiceOption label={FolderView::List.label()} />
                        }}
                        selected={mode_index}
                        label="Layout"
                        @test_id={"folder.view"}
                        on_change={move |chosen: Option<usize>| {
                            if let Some(shown) = chosen.and_then(|index| FolderView::ALL.get(index))
                            {
                                set_mode.set(*shown);
                            }
                        }}
                    />
                    <Select
                        options={view! {
                            <ChoiceOption label={FolderSort::Intrinsic.label()} />
                            <ChoiceOption label={FolderSort::Name.label()} />
                            <ChoiceOption label={FolderSort::Type.label()} />
                        }}
                        selected={sort_index}
                        label="Sort"
                        @test_id={"folder.sort"}
                        on_change={move |chosen: Option<usize>| {
                            if let Some(order) = chosen.and_then(|index| FolderSort::ALL.get(index))
                            {
                                set_sort.set(*order);
                            }
                        }}
                    />
                    <IconButton
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
                    outline={outline}
                    outline_width=DROP_OUTLINE
                    outline_visible={hovering}
                    padding_horizontal=PADDING
                    padding_vertical=PADDING
                >
                    <List spacing=0.0>
                        <Show condition={empty}>
                            <Caption
                                content="This folder is empty."
                                align=TextAlign::Center
                                @test_id={"folder.empty"}
                            />
                        </Show>
                        <Scroll @sizing=ItemSize::Percent(100.0)>
                            <Dynamic value={mode}>
                                {move |shown: FolderView| {
                                    let cells = cells.clone();
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
                                }}
                            </Dynamic>
                        </Scroll>
                    </List>
                </Frame>
            </List>
        </Frame>
    }
}

#[component]
fn EntryRows(cells: Cells) -> NodeId {
    let keys = cells.keys();
    let selected = cells.selected.clone();
    let chosen = create_selector(move || selected.get());
    view! {
        <List spacing=2.0>
            <ForEach keys={keys}>
                {move |key: Uuid| {
                    let entry = cells.entry(key);
                    let open = cells.open(entry.clone());
                    let set_selected = cells.set_selected.clone();
                    let here = chosen.memo(Some(key));
                    view! {
                        <ListRow
                            selected={here}
                            @test_id={entry_test_id(key)}
                            on_click={move || set_selected.set(Some(key))}
                            on_activate={open}
                        >
                            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                                <EntryGlyph entry={entry.clone()} size=ROW_ICON />
                                <Body
                                    @sizing=ItemSize::Percent(100.0)
                                    content={name_of(entry.clone())}
                                    color={name_color(entry.clone())}
                                />
                                <Caption content={type_of(entry)} />
                            </List>
                        </ListRow>
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn EntryTiles(cells: Cells, tile: Vec2, icon_size: f32) -> NodeId {
    let keys = cells.keys();
    let selected = cells.selected.clone();
    let chosen = create_selector(move || selected.get());
    view! {
        <List direction=Direction::Horizontal wrap=true align=Align::Start spacing=TILE_SPACING>
            <ForEach keys={keys}>
                {move |key: Uuid| {
                    let entry = cells.entry(key);
                    let open = cells.open(entry.clone());
                    let set_selected = cells.set_selected.clone();
                    let here = chosen.memo(Some(key));
                    view! {
                        <ListRow
                            @sizing=ItemSize::Fixed(tile.x)
                            selected={here}
                            @test_id={entry_test_id(key)}
                            on_click={move || set_selected.set(Some(key))}
                            on_activate={open}
                        >
                            <Frame height={tile.y}>
                                <List align=Align::Center spacing=4.0>
                                    <Spacer @sizing=ItemSize::Percent(100.0) />
                                    <EntryGlyph entry={entry.clone()} size={icon_size} />
                                    <Body
                                        content={name_of(entry.clone())}
                                        color={name_color(entry.clone())}
                                        align=TextAlign::Center
                                    />
                                    <Caption content={type_of(entry)} align=TextAlign::Center />
                                    <Spacer @sizing=ItemSize::Percent(100.0) />
                                </List>
                            </Frame>
                        </ListRow>
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn EntryGlyph(entry: Memo<Option<Entry>>, size: f32) -> NodeId {
    let theme = use_theme();
    let glyph = create_memo(move || {
        let glyph = entry.get().map(|entry| entry.glyph).unwrap_or_default();
        match glyph.is_empty() {
            true => ICON_FOLDER.to_owned(),
            false => glyph,
        }
    });
    view! {
        <IconSized glyph={glyph} font_size={size} color={theme.text.clone()} />
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

fn name_color(entry: Memo<Option<Entry>>) -> Memo<Color32> {
    let theme = use_theme();
    create_memo(move || {
        let automatic = entry.get().is_none_or(|entry| entry.automatic);
        match automatic {
            true => theme.text_muted.get(),
            false => theme.text.get(),
        }
    })
}
