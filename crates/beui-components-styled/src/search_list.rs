use std::rc::Rc;

use accesskit::{Node, Role};
use beui_components_unstyled as unstyled;
use beui_components_unstyled::{
    CommandRowHandle, PaletteItem, PaletteScore, SearchListApi, SearchRowHandle,
};
use beui_macros::{component, view};
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::{Align, Overflow, TextAlign};
use stable_hash::HashMap;

use crate::command_palette::{PaletteRow, PaletteSearch};
use crate::frame::{ColorRole, Frame, Radius};
use crate::list::List;
use crate::picture::Picture;
use crate::scroll::scrollbar_style;
use crate::text::{Icon, Text, TextColor, TextSize};
use crate::text_input::text_input_style;
use crate::tokens::{Padding, Size, Space, use_space};

const LIST_HEIGHT: f32 = 360.0;
const SPACING: Space = Space::Sm;
const TILE_PADDING: Space = Space::Sm;
const TILE_SPACING: Space = Space::Xs;
const TILE_ICON: Size = Size::Lg;

#[component]
pub fn SearchList(
    #[prop(default = ComponentRef::new())] api: ComponentRef<SearchListApi<String>>,
    items: Prop<Vec<PaletteItem>>,
    #[prop(default = PaletteItem::substring_score as PaletteScore)] score: PaletteScore,
    #[prop(default = create_signal(String::new()))] query: Signal<String>,
    #[prop(default = 1)] columns: Prop<usize>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = LIST_HEIGHT)] list_height: Prop<f32>,
    #[prop(default = "Search".to_owned())] placeholder: Prop<String>,
    #[prop(default = "Search".to_owned())] label: Prop<String>,
    #[prop(default = "search-list".to_owned())] id: String,
    on_choose: Callback<String>,
) -> NodeId {
    let listed = create_memo(move || items.get());
    let by_key = create_memo(move || {
        listed.with(|items| {
            Rc::new(
                items
                    .iter()
                    .enumerate()
                    .map(|(at, item)| (item.key.clone(), at))
                    .collect::<HashMap<_, _>>(),
            )
        })
    });
    let item = move |key: &str| {
        let at = by_key.with(|keys| keys.get(key).copied())?;
        listed.with(|items| items.get(at).cloned())
    };
    let ranked = move |query: String| {
        listed.with(|items| {
            unstyled::rank(items, &query, score)
                .into_iter()
                .map(|at| items[at].key.clone())
                .collect::<Vec<_>>()
        })
    };
    let enabled = move |key: String| item(&key).is_some_and(|item| !item.disabled);
    let search_accessibility = create_memo(move || {
        let mut node = Node::new(Role::SearchInput);
        node.set_label(label.get());
        node
    });
    let row_accessibility = move |key: String| {
        let mut node = Node::new(Role::ListBoxOption);
        if let Some(item) = item(&key) {
            node.set_label(item.title);
            if item.disabled {
                node.set_disabled();
            }
        }
        node
    };
    let columns = create_memo(move || columns.get().max(1));
    let tiled = create_memo(move || columns.get() > 1);
    let search_id = format!("{id}.search");
    let row = move |handle: SearchRowHandle<String>| {
        let key = handle.key.clone();
        let row_test_id = format!("{id}.item.{key}");
        let found = create_memo({
            let key = key.clone();
            move || item(&key)
        });
        let text = move |read: fn(&PaletteItem) -> String| {
            create_memo(move || found.with(|item| item.as_ref().map(read).unwrap_or_default()))
        };
        let enabled = handle.enabled;
        let row = CommandRowHandle {
            key,
            label: text(|item| item.title.clone()),
            detail: text(|item| item.detail.clone()),
            glyph: create_memo(move || found.with(|item| item.as_ref()?.glyph)),
            image: create_memo(move || found.with(|item| item.as_ref()?.image.clone())),
            shortcut: text(|item| item.shortcut.clone()),
            checked: create_memo(move || found.with(|item| item.as_ref()?.checked)),
            disabled: create_memo(move || !enabled.get()),
            highlighted: handle.highlighted,
            hovered: handle.hovered,
        };
        match tiled.get_untracked() {
            true => view! {
                <PaletteTile @test_id={row_test_id} handle={row} />
            },
            false => view! {
                <PaletteRow @test_id={row_test_id} handle={row} />
            },
        }
    };
    view! {
        <unstyled::SearchList
            api
            query
            highlighted={create_signal(None)}
            rank={ranked}
            enabled
            columns
            on_choose={move |key: String| on_choose.call(key)}
            focused
            placeholder
            style={text_input_style()}
            search_accessibility
            field={move |handle| view! {
                <PaletteSearch @test_id={search_id.clone()} handle />
            }}
            list_height
            spacing={use_space(SPACING).get_untracked()}
            scrollbar={scrollbar_style()}
            accessibility={row_accessibility}
            row
        />
    }
}

#[component]
fn PaletteTile(handle: CommandRowHandle) -> NodeId {
    let CommandRowHandle {
        label,
        glyph,
        image,
        disabled,
        highlighted,
        hovered,
        ..
    } = handle;
    let pictured = create_memo(move || image.with(Option::is_some));
    let plain = create_memo(move || !pictured.get());
    let fill = create_memo(move || match (highlighted.get(), hovered.get()) {
        (true, true) => ColorRole::AccentSoftHover,
        (true, false) => ColorRole::AccentSoft,
        (false, true) => ColorRole::Hover,
        (false, false) => ColorRole::None,
    });
    let color = create_memo(move || match disabled.get() {
        true => TextColor::Muted,
        false => TextColor::Default,
    });
    view! {
        <Frame fill radius=Radius::Md padding={Padding::all(TILE_PADDING)}>
            <List align=Align::Center spacing=TILE_SPACING>
                <Frame width=TILE_ICON height=TILE_ICON>
                    <List align=Align::Center spacing=Space::None>
                        <Show condition={pictured}>
                            <Picture image />
                        </Show>
                        <Show condition={plain}>
                            <Text size=TextSize::Xl color align=TextAlign::Center>
                                <Icon glyph />
                            </Text>
                        </Show>
                    </List>
                </Frame>
                <Text size=TextSize::Sm color align=TextAlign::Center overflow=Overflow::Ellipsis>
                    {label}
                </Text>
            </List>
        </Frame>
    }
}
