use beui_components_unstyled as unstyled;
use beui_components_unstyled::{TreemapHandle, TreemapItem, TreemapTileHandle, TreemapTileKey};
use beui_macros::{component, view};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::icons::ICON_CORNER_LEFT_UP;
use beui_view::types::{Align, Direction, Overflow, TextAlign};
use std::hash::Hash;

use crate::button::{Button, ButtonSize, ButtonVariant};
use crate::frame::{Border, ColorRole, Frame, Radius};
use crate::list::List;
use crate::text::{Text, TextColor, TextSize};
use crate::tokens::{Length, Size, Space, Stroke};

const SPACING: Space = Space::Sm;
const GAP: Space = Space::Xs2;
const TILE_PADDING: Space = Space::Xs;
const LABEL_WIDTH: Size = Size::Xl;
const LABEL_HEIGHT: Size = Size::Md;
const DETAIL_HEIGHT: Size = Size::Xl;

/// A treemap with a bar above it: a button back out, the level's name, and the
/// name and size of the tile pointed at or focused. Tiles that open are tinted
/// apart from those that do not, and show their name, then their size, where
/// they fit. Give it its room: it fills what it is given.
#[component]
pub fn Treemap<K>(
    items: Prop<Vec<TreemapItem<K>>>,
    path: Binding<Vec<TreemapItem<K>>>,
    /// Says a size in words.
    format: Func<u64, String>,
    /// The name of the top level.
    #[prop(default = String::new())]
    label: Prop<String>,
    #[prop(default = String::from("treemap.tile"))] tile_test_id: String,
    #[prop(default = String::from("treemap.up"))] up_test_id: String,
) -> NodeId
where
    K: Clone + Eq + Hash + std::fmt::Display + 'static,
{
    let gap = GAP.px();
    let top = label.clone();
    view! {
        <unstyled::Treemap
            items
            path
            describe=format
            gap
            label
            tile_test_id
            tile={move |handle: TreemapTileHandle<K>| view! {
                <TreemapTileFace handle format />
            }}
        >|handle: TreemapHandle<K>| {
            let TreemapHandle { tiles, path, up, shown, total, .. } = handle;
            let top = top.clone();
            let at_top = create_memo(move || path.with(Vec::is_empty));
            let level = create_memo(move || {
                path.with(|path| path.last().map(|item| item.label.clone()))
                    .unwrap_or_else(|| top.get())
            });
            let caption = create_memo(move || match shown.get() {
                Some(tile) => format!("{} · {}", tile.label, format.call(tile.size)),
                None => format.call(total.get()),
            });
            view! {
                <List spacing=SPACING>
                    <List direction=Direction::Horizontal align=Align::Center spacing=SPACING>
                        <Button
                            @test_id={up_test_id.clone()}
                            label="Up"
                            glyph=ICON_CORNER_LEFT_UP
                            icon_only=true
                            variant=ButtonVariant::Ghost
                            size=ButtonSize::Small
                            disabled={at_top}
                            on_click={up}
                        />
                        <ListItem shrink=1.0>
                            <Text size=TextSize::Sm overflow=Overflow::Ellipsis>{level}</Text>
                        </ListItem>
                        <ListItem grow=1.0 shrink=1.0>
                            <Text
                                size=TextSize::Sm
                                color=TextColor::Muted
                                align={Some(TextAlign::End)}
                                overflow=Overflow::Ellipsis
                            >
                                {caption}
                            </Text>
                        </ListItem>
                    </List>
                    <ListItem grow=1.0>{tiles}</ListItem>
                </List>
            }
        }</unstyled::Treemap>
    }
}

#[component]
fn TreemapTileFace<K>(handle: TreemapTileHandle<K>, format: Func<u64, String>) -> NodeId
where
    K: Clone + Eq + Hash + 'static,
{
    let TreemapTileHandle {
        tile,
        size,
        focused,
        hovered,
        active,
    } = handle;
    let opens = create_memo(move || tile.with(|tile| tile.opens));
    let rest = create_memo(move || tile.with(|tile| tile.key == TreemapTileKey::Rest));
    let fill =
        create_memo(
            move || match (opens.get(), rest.get(), active.get() || hovered.get()) {
                (_, true, false) => ColorRole::Shade,
                (_, true, true) => ColorRole::Hover,
                (true, _, false) => ColorRole::AccentSoft,
                (true, _, true) => ColorRole::AccentSoftHover,
                (false, _, false) => ColorRole::Control,
                (false, _, true) => ColorRole::Hover,
            },
        );
    let border = Some(Border::new(ColorRole::Border).width(Stroke::Hairline));
    let (label_width, label_height, detail_height) =
        (LABEL_WIDTH.px(), LABEL_HEIGHT.px(), DETAIL_HEIGHT.px());
    let labelled = create_memo(move || {
        size.get()
            .is_some_and(|size| size.x >= label_width && size.y >= label_height)
    });
    let detailed = create_memo(move || {
        labelled.get() && size.get().is_some_and(|size| size.y >= detail_height)
    });
    let label = create_memo(move || tile.with(|tile| tile.label.clone()));
    let detail = create_memo(move || format.call(tile.with(|tile| tile.size)));
    view! {
        <Frame
            width=Length::FILL
            height=Length::FILL
            fill
            border
            focus={focused}
            radius=Radius::Sm
            padding=TILE_PADDING
        >
            <List spacing=Space::None>
                <Show condition={labelled}>
                    <Text size=TextSize::Xs overflow=Overflow::Ellipsis>{label}</Text>
                </Show>
                <Show condition={detailed}>
                    <Text size=TextSize::Xs color=TextColor::Muted overflow=Overflow::Ellipsis>
                        {detail}
                    </Text>
                </Show>
            </List>
        </Frame>
    }
}
