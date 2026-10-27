use block_editor_beui::be_block::metadata::{ICON, ICON_COLOR, MATERIAL_ICON, MAX_NAME_BYTES};
use block_editor_beui::beui::dock::styled::{DockDialog, DockDialogPresentation, DockNode, TabId};
use block_editor_beui::beui::icons::Glyph;
use block_editor_beui::beui::icons::{ALL, ICON_SEARCH, named};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::{Color32, NodeId, TextAlign, Vec2};
use block_editor_beui::block_ui::BlockTypes;
use uuid::Uuid;

use super::workspace::Workspace;
use beui::{Align, Align2, Direction, ItemSize};

pub(crate) const RENAME: TabId = TabId::new((1 << 39) + 2);
const SIZE: Vec2 = Vec2::new(392.0, 620.0);
const SPACING: Space = Space::Md;
const COLUMNS: usize = 8;
const TILE: Size = Size::Xl;
const TILE_SPACING: Space = Space::Xs;
const SWATCH_SPACING: Space = Space::Xs2;
const NEUTRAL: [u8; 3] = [135, 139, 148];

pub(crate) const COLORS: [(&str, [u8; 3]); 8] = [
    ("Blue", [65, 145, 210]),
    ("Green", [44, 158, 128]),
    ("Purple", [155, 112, 211]),
    ("Orange", [211, 126, 59]),
    ("Pink", [203, 91, 125]),
    ("Teal", [64, 155, 173]),
    ("Gold", [172, 145, 47]),
    ("Gray", NEUTRAL),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenameAnchor {
    pub rect: Option<block_editor_beui::beui::Rect>,
    pub tab: Option<TabId>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Renaming {
    pub(crate) block: Uuid,
    pub(crate) anchor: RenameAnchor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Tile {
    Default,
    Icon(&'static str),
}

#[derive(Clone, Copy, PartialEq)]
struct Look {
    icon: Option<&'static str>,
    color: Option<[u8; 3]>,
    type_icon: Option<Glyph>,
    type_color: Option<[u8; 3]>,
}

fn rgb([r, g, b]: [u8; 3]) -> Color32 {
    Color32::from_rgb(r, g, b)
}

pub(crate) fn matching(search: &str) -> Vec<&'static str> {
    let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
    let (mut named, mut tagged) = (Vec::new(), Vec::new());
    for icon in ALL {
        if words.iter().all(|word| icon.name.contains(word.as_str())) {
            named.push(icon.name);
        } else if words
            .iter()
            .all(|word| icon.name.contains(word.as_str()) || icon.tags.contains(word.as_str()))
        {
            tagged.push(icon.name);
        }
    }
    named.extend(tagged);
    named
}

#[component]
pub fn RenameDialog(workspace: Workspace, block: Uuid, tab: TabId) -> DockNode<TabId> {
    let anchor = workspace.rename_anchor().and_then(|anchor| anchor.rect);
    let initial = untrack(|| {
        workspace
            .info(block)
            .map(|info| workspace.label(info.id, info.block_type).name)
            .unwrap_or_default()
    });
    let name = create_signal(initial.clone());
    let invalid = create_memo(move || name.with(|name| name.len() > MAX_NAME_BYTES));
    let done = Callback::new(move |()| {
        let chosen = name.get_untracked();
        if chosen != initial && !invalid.get_untracked() {
            workspace.rename(block, &chosen);
        }
        workspace.close_rename();
    });
    view! {
        <DockDialog
            id=RENAME
            parent={tab}
            title="Rename"
            size=SIZE
            presentation=DockDialogPresentation::Card
            anchor
            dismiss_on_outside_press=true
            on_close={move || done.call(())}
        >
            <RenameBody workspace block name invalid done />
        </DockDialog>
    }
}

#[component]
fn RenameBody(
    workspace: Workspace,
    block: Uuid,
    name: Signal<String>,
    invalid: Memo<bool>,
    done: Callback<()>,
) -> NodeId {
    let look = create_memo(move || {
        let types = workspace.types();
        let info = workspace.info(block);
        let block_type = info.as_ref().map(|info| info.block_type);
        Look {
            icon: info
                .as_ref()
                .and_then(|info| info.metadata.icon())
                .and_then(|icon| ALL.iter().find(|named| named.name == icon))
                .map(|named| named.name),
            color: info
                .as_ref()
                .and_then(|info| info.metadata.get(&ICON_COLOR)),
            type_icon: block_type.and_then(|block_type| types.icon(block_type)),
            type_color: block_type.and_then(|block_type| types.color(block_type)),
        }
    });
    let previewed = create_signal(None::<Color32>);
    let shown = create_memo(move || {
        let look = look.get();
        previewed
            .get()
            .or_else(|| look.color.or(look.type_color).map(rgb))
    });
    let set_icon = Callback::new(move |icon: Option<&'static str>| {
        let blocks = workspace.blocks();
        blocks.define(block, ICON.definition(icon.map(str::to_owned).as_ref()));
        blocks.define(block, MATERIAL_ICON.definition(None));
    });
    let set_color = Callback::new(move |color: Option<[u8; 3]>| {
        workspace
            .blocks()
            .define(block, ICON_COLOR.definition(color.as_ref()));
    });
    let custom = Binding::new(
        create_memo(move || {
            let look = look.get();
            rgb(look.color.or(look.type_color).unwrap_or(NEUTRAL))
        }),
        move |color: Color32| {
            let [r, g, b, _] = color.to_array();
            set_color.call(Some([r, g, b]));
        },
    );
    let search = create_signal(String::new());
    let rows = create_memo(move || {
        let search = search.get();
        let mut tiles: Vec<Tile> = Vec::new();
        if search.trim().is_empty() {
            tiles.push(Tile::Default);
        }
        tiles.extend(matching(&search).into_iter().map(Tile::Icon));
        tiles
            .chunks(COLUMNS)
            .map(<[Tile]>::to_vec)
            .collect::<Vec<_>>()
    });
    let empty = create_memo(move || rows.with(Vec::is_empty));
    let error = create_memo(move || format!("Name must be at most {MAX_NAME_BYTES} UTF-8 bytes."));
    let pictured = create_memo(move || {
        let look = look.get();
        look.icon.and_then(named).or(look.type_icon)
    });
    let tinted = create_memo(move || {
        shown
            .get()
            .map_or(TextColor::Muted, |color| TextColor::Custom(color.into()))
    });
    let tile = use_size(TILE);
    let gap = use_space(TILE_SPACING);
    let row_size = create_memo(move || tile.get() + gap.get());
    let field = NodeRef::new();
    let built = view! {
        <DialogBody title="Rename" id="rename" scrolls=false on_dismiss={move || done.call(())}>
            <List spacing=SPACING>
                <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                    <Frame
                        width=TILE
                        height=TILE
                        radius=Radius::Md
                        fill=ColorRole::SurfaceRaised
                        align={Align2::CENTER}
                    >
                        <Text color={tinted} size=TextSize::Xl align=TextAlign::Center>
                            <Icon glyph={pictured} />
                        </Text>
                    </Frame>
                    <TextInput
                        @sizing=ItemSize::Percent(100.0)
                        @node_ref={&field}
                        @test_id={"rename.name"}
                        value={name}
                        label="Name"
                        select_on_focus=true
                        on_submit={move |_: String| done.call(())}
                    />
                </List>
                <Show condition={invalid}>
                    <Text style=TextStyle::CAPTION>{error}</Text>
                </Show>
                <Text style=TextStyle::CAPTION>"Color"</Text>
                <List direction=Direction::Horizontal align=Align::Center spacing=SWATCH_SPACING>
                    <SwatchButton
                        @test_id={"rename.color.default"}
                        label="Block type color"
                        color={create_memo(move || rgb(look.get().type_color.unwrap_or(NEUTRAL)))}
                        selected={create_memo(move || look.get().color.is_none())}
                        on_click={move || set_color.call(None)}
                    />
                    <ForEach keys={COLORS.to_vec()}>|(name, color): (&'static str, [u8; 3])|
                        <SwatchButton
                            @test_id={format!("rename.color.{}", name.to_lowercase())}
                            label={name}
                            color={rgb(color)}
                            selected={create_memo(move || look.get().color == Some(color))}
                            on_click={move || set_color.call(Some(color))}
                        />
                    </ForEach>
                </List>
                <ColorInput
                    @test_id={"rename.custom"}
                    value={custom}
                    label="Custom color"
                    alpha=false
                    on_preview={move |preview: Option<Color32>| previewed.set(preview)}
                />
                <Text style=TextStyle::CAPTION>"Icon"</Text>
                <TextInput
                    value={search}
                    placeholder="Search icons"
                    label="Search icons"
                    glyph=ICON_SEARCH
                    clearable=true
                    @test_id={"rename.search"}
                />
            </List>
            <Scroll @sizing=ItemSize::Percent(100.0) bleed={Bleed::DIALOG.top(false)}>
                <List spacing=TILE_SPACING>
                    <Show condition={empty}>
                        <Text style=TextStyle::CAPTION>"No icons match."</Text>
                    </Show>
                    <VirtualList keys={rows} item_size={row_size}>|row: ReadSignal<Vec<Tile>>|
                        <List direction=Direction::Horizontal spacing=TILE_SPACING>
                            <ForEach keys={create_memo(move || row.get())}>|tile: Tile|
                                <IconTile tile look shown set_icon />
                            </ForEach>
                        </List>
                    </VirtualList>
                </List>
            </Scroll>
        </DialogBody>
    };
    if let Some(node) = field.try_get() {
        node.focus_into();
    }
    built
}

#[component]
fn IconTile(
    tile: Tile,
    look: Memo<Look>,
    shown: Memo<Option<Color32>>,
    set_icon: Callback<Option<&'static str>>,
) -> NodeId {
    let (id, label) = match tile {
        Tile::Default => ("default".to_owned(), "Block type icon".to_owned()),
        Tile::Icon(name) => (name.to_owned(), name.replace('-', " ")),
    };
    let glyph = create_memo(move || match tile {
        Tile::Default => look.get().type_icon,
        Tile::Icon(name) => named(name),
    });
    let chosen = create_memo(move || match tile {
        Tile::Default => look.get().icon.is_none(),
        Tile::Icon(name) => look.get().icon == Some(name),
    });
    let color = create_memo(move || {
        shown
            .get()
            .map_or(TextColor::Muted, |color| TextColor::Custom(color.into()))
    });
    view! {
        <TileButton
            @test_id={format!("rename.icon.{id}")}
            label
            selected={chosen}
            size=TILE
            on_click={move || set_icon.call(match tile {
                Tile::Default => None,
                Tile::Icon(name) => Some(name),
            })}
        >
            <Text color size=TextSize::Xl align=TextAlign::Center>
                <Icon glyph />
            </Text>
        </TileButton>
    }
}
