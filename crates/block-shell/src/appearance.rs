use block_editor_beui::be_block::metadata::{ICON, ICON_COLOR, MATERIAL_ICON};
use block_editor_beui::beui::icons::{ALL, ICON_SEARCH, ICON_X, named};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::beui::unstyled::{
    self, ButtonHandle, DockDialog, DockDialogPresentation, DockNode, TabId,
};
use block_editor_beui::beui::{Color32, NodeId, TextAlign, Vec2};
use block_editor_beui::block_ui::BlockTypes;
use uuid::Uuid;

use super::picker::PickAnchor;
use super::workspace::Workspace;

pub(crate) const APPEARANCE: TabId = TabId::new((1 << 39) + 2);
const SIZE: Vec2 = Vec2::new(392.0, 560.0);
const PADDING: f32 = 16.0;
const SPACING: f32 = 10.0;
const COLUMNS: usize = 8;
const TILE: f32 = 40.0;
const TILE_SPACING: f32 = 4.0;
const SWATCH: f32 = 22.0;
const SWATCH_RING: f32 = 2.0;
const SWATCH_SPACING: f32 = 6.0;
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
pub(crate) struct Appearing {
    pub(crate) block: Uuid,
    pub(crate) anchor: PickAnchor,
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
    type_icon: Option<&'static str>,
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
pub fn AppearanceDialog(workspace: Workspace, block: Uuid, tab: TabId) -> DockNode<TabId> {
    let anchor = workspace.appearance_anchor().and_then(|anchor| anchor.rect);
    view! {
        <DockDialog
            id=APPEARANCE
            parent={tab}
            title="Appearance"
            size=SIZE
            presentation=DockDialogPresentation::Card
            anchor
            dismiss_on_outside_press=true
            on_close={move || workspace.close_appearance()}
        >
            <AppearanceBody workspace block />
        </DockDialog>
    }
}

#[component]
fn AppearanceBody(workspace: Workspace, block: Uuid) -> NodeId {
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
    let field = NodeRef::new();
    let built = view! {
        <List spacing=0.0>
            <Frame padding={Sides::new(PADDING, PADDING, PADDING, 0.0)}>
                <List spacing=SPACING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Text @sizing=ItemSize::Percent(100.0) style=TextStyle::TITLE>
                            "Appearance"
                        </Text>
                        <IconButton
                            @test_id={"appearance.close"}
                            glyph={ICON_X.to_owned()}
                            label="Close"
                            variant=ButtonVariant::Ghost
                            on_click={move || workspace.close_appearance()}
                        />
                    </List>
                    <Text style=TextStyle::CAPTION>"Color"</Text>
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=SWATCH_SPACING
                    >
                        <Swatch
                            id={"default".to_owned()}
                            label="Block type color"
                            color={create_memo(move || look.get().type_color)}
                            chosen={create_memo(move || look.get().color.is_none())}
                            on_pick={move || set_color.call(None)}
                        />
                        <ForEach keys={COLORS.to_vec()}>|(name, color): (&'static str, [u8; 3])|
                            <Swatch
                                id={name.to_lowercase()}
                                label={name}
                                color={create_memo(move || Some(color))}
                                chosen={create_memo(move || look.get().color == Some(color))}
                                on_pick={move || set_color.call(Some(color))}
                            />
                        </ForEach>
                    </List>
                    <ColorInput
                        @test_id={"appearance.custom"}
                        value={custom}
                        label="Custom color"
                        alpha=false
                        on_preview={move |preview: Option<Color32>| previewed.set(preview)}
                    />
                    <Text style=TextStyle::CAPTION>"Icon"</Text>
                    <TextInput
                        @node_ref={&field}
                        value={search}
                        placeholder="Search icons"
                        label="Search icons"
                        glyph={ICON_SEARCH.to_owned()}
                        clearable=true
                        @test_id={"appearance.search"}
                    />
                </List>
            </Frame>
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <Frame padding=PADDING>
                    <List spacing=TILE_SPACING>
                        <Show condition={empty}>
                            <Text style=TextStyle::CAPTION>"No icons match."</Text>
                        </Show>
                        <VirtualList
                            keys={rows}
                            item_size={TILE + TILE_SPACING}
                        >|row: ReadSignal<Vec<Tile>>|
                            <List direction=Direction::Horizontal spacing=TILE_SPACING>
                                <ForEach keys={create_memo(move || row.get())}>|tile: Tile|
                                    <IconTile tile look shown set_icon />
                                </ForEach>
                            </List>
                        </VirtualList>
                    </List>
                </Frame>
            </Scroll>
        </List>
    };
    if let Some(node) = field.try_get() {
        with_document(|document| document.focus_into(node));
    }
    built
}

#[component]
fn Swatch<F: Fn() + 'static>(
    id: String,
    label: &'static str,
    color: Memo<Option<[u8; 3]>>,
    chosen: Memo<bool>,
    on_pick: F,
) -> NodeId {
    let fill = create_memo(move || match color.get() {
        Some(color) => ColorRole::Custom(rgb(color)),
        None => ColorRole::TextMuted,
    });
    view! {
        <unstyled::Button
            @test_id={format!("appearance.color.{id}")}
            label
            on_click={on_pick}
        >|handle: ButtonHandle| {
            let ringed = create_memo(move || chosen.get() || handle.focused.get() || handle.hovered.get());
            let ring = create_memo(move || match chosen.get() {
                true => ColorRole::Accent,
                false => ColorRole::Border,
            });
            view! {
                <Tooltip label={handle.tooltip}>
                    <Frame
                        padding=SWATCH_RING
                        radius=Radius::Full
                        border={create_memo(move || ringed.get().then(|| Border::new(ring.get()).width(SWATCH_RING)))}
                    >
                        <Frame
                            width=SWATCH
                            height=SWATCH
                            radius=Radius::Full
                            fill
                            border={Border::DEFAULT}
                        />
                    </Frame>
                </Tooltip>
            }
        }</unstyled::Button>
    }
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
        Tile::Default => look.get().type_icon.unwrap_or_default().to_owned(),
        Tile::Icon(name) => named(name).unwrap_or_default().to_owned(),
    });
    let chosen = create_memo(move || match tile {
        Tile::Default => look.get().icon.is_none(),
        Tile::Icon(name) => look.get().icon == Some(name),
    });
    let color = create_memo(move || shown.get().map_or(TextColor::Muted, TextColor::Custom));
    view! {
        <unstyled::Button
            @test_id={format!("appearance.icon.{id}")}
            label
            on_click={move || set_icon.call(match tile {
                Tile::Default => None,
                Tile::Icon(name) => Some(name),
            })}
        >|handle: ButtonHandle| {
            let fill = create_memo(move || match (chosen.get(), handle.hovered.get()) {
                (true, _) => ColorRole::AccentSoft,
                (false, true) => ColorRole::Hover,
                (false, false) => ColorRole::None,
            });
            view! {
                <Tooltip label={handle.tooltip}>
                    <Frame
                        width=TILE
                        height=TILE
                        radius=Radius::Md
                        fill
                        border={focus_border(handle.focused, Border::FOCUS)}
                        align={Align2::CENTER}
                    >
                        <Text color size=TextSize::Xl align=TextAlign::Center>
                            <Icon glyph />
                        </Text>
                    </Frame>
                </Tooltip>
            }
        }</unstyled::Button>
    }
}
